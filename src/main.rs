// https://docs.gitlab.com/ee/api/projects.html
// https://docs.gitlab.com/ee/api/project_access_tokens.html

use axum::{extract::State, http::StatusCode, routing::get, Router};
use core::fmt::Write as _; // import without risk of name clashing
use core::{future::IntoFuture, time::Duration};
use dotenv::dotenv;
use serde::Deserialize;
use serde_repr::Deserialize_repr;
use std::{env, error::Error};
use tokio::{
    net::TcpListener,
    select,
    signal::unix::{signal, SignalKind},
    sync::{mpsc, oneshot},
    time,
};

const DATA_REFRESH_HOURS_DEFAULT: u8 = 6;

#[derive(Debug)]
enum ActorMessage {
    GetResponse { respond_to: oneshot::Sender<String> },
}

#[derive(Clone)]
struct AppState {
    sender: mpsc::Sender<ActorMessage>,
}

#[derive(Debug, Deserialize)]
struct Project {
    id: usize,
    path_with_namespace: String,
}

#[derive(Debug, Deserialize_repr)]
#[repr(u8)]
enum AccessLevel {
    Guest = 10,
    Reporter = 20,
    Developer = 30,
    Maintainer = 40,
    Owner = 50,
}

#[derive(Debug, Deserialize)]
struct AccessToken {
    scopes: Vec<String>,
    name: String,
    expires_at: chrono::NaiveDate,
    active: bool,
    revoked: bool,
    access_level: AccessLevel,
}

async fn get_all_projects(
    http_client: &reqwest::Client,
    gitlab_baseurl: &String,
    gitlab_token: &String,
) -> Result<Vec<Project>, Box<dyn Error>> {
    let mut result: Vec<Project> = Vec::new();
    let mut next_url: Option<String> = Some(format!(
        "https://{gitlab_baseurl}/api/v4/projects?per_page=100"
    ));

    while let Some(url) = next_url {
        let resp = http_client
            .get(url)
            .header("PRIVATE-TOKEN", gitlab_token)
            .send()
            .await?;

        next_url = resp
            .headers()
            .get("link")
            .and_then(|header_value| header_value.to_str().ok())
            .and_then(|header_value_str| parse_link_header::parse_with_rel(header_value_str).ok())
            .and_then(|links| links.get("next").map(|link| link.raw_uri.clone()));

        let mut projects: Vec<Project> = resp.json().await?;
        result.append(&mut projects);
    }

    Ok(result)
}

async fn get_project_access_tokens(
    req_client: &reqwest::Client,
    gitlab_baseurl: &String,
    gitlab_token: &String,
    project: &Project,
) -> Result<Vec<AccessToken>, Box<dyn Error>> {
    let mut result: Vec<AccessToken> = Vec::new();
    let mut next_url: Option<String> = Some(format!(
        "https://{gitlab_baseurl}/api/v4/projects/{}/access_tokens?per_page=100",
        project.id
    ));

    while let Some(url) = next_url {
        let resp = req_client
            .get(url)
            .header("PRIVATE-TOKEN", gitlab_token)
            .send()
            .await?;

        next_url = resp
            .headers()
            .get("link")
            .and_then(|header_value| header_value.to_str().ok())
            .and_then(|header_value_str| parse_link_header::parse_with_rel(header_value_str).ok())
            .and_then(|links| links.get("next").map(|link| link.raw_uri.clone()));

        let mut access_tokens: Vec<AccessToken> = resp.json().await?;
        result.append(&mut access_tokens);
    }

    Ok(result)
}

#[allow(clippy::arithmetic_side_effects)]
fn build_metric(project: &Project, access_token: &AccessToken) -> String {
    let mut res = String::new();
    let date_now = chrono::Utc::now().date_naive();

    let metric_name = format!(
        "gitlab_token_{}_{}",
        project.path_with_namespace, access_token.name
    )
    .replace(['-', '/', ' '], "_");

    writeln!(res, "# HELP {metric_name} Gitlab token").unwrap();
    writeln!(res, "# TYPE {metric_name} gauge").unwrap();
    let access_level = format!("{:?}", access_token.access_level).replace('"', "");
    let scopes = format!("{:?}", access_token.scopes).replace('"', "");
    writeln!(res, "{metric_name}{{project=\"{}\",token_name=\"{}\",active=\"{}\",revoked=\"{}\",access_level=\"{access_level}\",scopes=\"{scopes}\",expires_at=\"{}\"}} {}",
        project.path_with_namespace,
        access_token.name,
        access_token.active,
        access_token.revoked,
        access_token.expires_at,
        (access_token.expires_at - date_now).num_days()
    ).unwrap();

    res
}

#[allow(clippy::integer_division_remainder_used)] // Because clippy is not happy with the tokio::select macro
#[allow(clippy::redundant_pub_crate)] // Because clippy is not happy with the tokio::select macro
async fn gitlab_tokens_actor(mut receiver: mpsc::Receiver<ActorMessage>) -> String {
    let mut response = String::new(); // The is the state this actor is handling

    dotenv().ok();

    let Ok(gitlab_token) = env::var("GITLAB_TOKEN") else {
        return "env variable GITLAB_TOKEN is not defined".to_owned();
    };
    let Ok(gitlab_baseurl) = env::var("GITLAB_BASEURL") else {
        return "env variable GITLAB_BASEURL is not defined".to_owned();
    };
    let data_refresh_hours =
        env::var("DATA_REFRESH_HOURS").map_or(DATA_REFRESH_HOURS_DEFAULT, |value| {
            value.parse().map_or(DATA_REFRESH_HOURS_DEFAULT, |value| {
                if value > 0 && value <= 24 {
                    value
                } else {
                    DATA_REFRESH_HOURS_DEFAULT
                }
            })
        });

    // State (response) initialization is done below (the first call to timer.tick() returns immediately)

    let mut timer = time::interval(Duration::from_secs(
        u64::from(data_refresh_hours).saturating_mul(3600),
    ));

    let (update_sender, mut update_receiver) = mpsc::unbounded_channel();

    // We now wait for some messages (or for the timer to tick)
    loop {
        select! {
            msg = receiver.recv() => match msg {
                Some(msg_value) => match msg_value {
                    ActorMessage::GetResponse { respond_to } => respond_to.send(response.clone()).unwrap_or_else(|_| println!("Failed to send reponse : oneshot channel was closed"))
                },
                None =>
                    break "recv failed".to_owned()
            },
            _ = timer.tick() => {

                // We are going to create a async task to get an update of our data because it can take quite a long time
                // The response is sent to the 'update_sender_clone' channel
                // We have to clone() multiple variables to make the borrow checker happy ;)
                let update_sender_clone = update_sender.clone();
                let gitlab_baseurl_clone = gitlab_baseurl.clone();
                let gitlab_token_clone = gitlab_token.clone();

                println!("Updating tokens data...");

                tokio::spawn(async move {
                    let mut res = String::new();

                    // Create an HTTP client
                    let http_client = reqwest::Client::new();

                    let projects = get_all_projects(&http_client, &gitlab_baseurl_clone, &gitlab_token_clone)
                    .await
                    .unwrap_or_else(|err| panic!("Failed to get gitlab projects : {err}"));

                    for project in projects {
                        let project_access_tokens = get_project_access_tokens(&http_client, &gitlab_baseurl_clone, &gitlab_token_clone, &project)
                            .await
                            .unwrap_or_else(|err| panic!("Failed to get gitlab token for project {} : {err}", project.path_with_namespace));
                        if !project_access_tokens.is_empty() {
                            println!("{} :", project.path_with_namespace);
                            for project_access_token in project_access_tokens {
                                println!("  {project_access_token:?}");
                                res.push_str(&build_metric(&project, &project_access_token));
                            }
                        }
                    }
                    let _ = update_sender_clone.send(res);
                });
            },
            msg = update_receiver.recv() => match msg {
                Some(update_msg) => {
                response.clear();
                response.push_str(&update_msg);
                },
                None => break "recv failed".to_owned()
            }
        }
    }
}

async fn root_handler() -> &'static str {
    "I'm Alive :D"
}

#[allow(clippy::let_underscore_must_use)]
#[allow(clippy::let_underscore_untyped)]
async fn get_gitlab_tokens_handler(State(state): State<AppState>) -> (StatusCode, String) {
    // We are going to send a message to our actor and wait for an answer
    // But first, we create a oneshot channel to get the actor's response
    let (send, recv) = oneshot::channel();
    let msg = ActorMessage::GetResponse { respond_to: send };

    // Ignore send errors. If this send fails, so does the
    // recv.await below. There's no reason to check for the
    // same failure twice.
    let _ = state.sender.send(msg).await;

    match recv.await {
        Ok(res) => match res.len() {
            0 => (StatusCode::NO_CONTENT, res),
            _ => (StatusCode::OK, res),
        },
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

#[allow(clippy::integer_division_remainder_used)] // Because clippy is not happy with the tokio::select macro
#[allow(clippy::redundant_pub_crate)] // Because clippy is not happy with the tokio::select macro
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    // An infinite stream of 'SIGTERM' signals.
    let mut sigterm_stream = signal(SignalKind::terminate())?;

    // Create a channel and then an actor
    let (sender, receiver) = mpsc::channel(8);
    let actor_handle = tokio::spawn(gitlab_tokens_actor(receiver));

    let app = Router::new()
        .route("/", get(root_handler))
        .route("/metrics", get(get_gitlab_tokens_handler))
        .with_state(AppState { sender });

    let listener = TcpListener::bind("0.0.0.0:3000").await?;

    println!("listening on {}", listener.local_addr()?);

    // Waiting for one of the following :
    // - a SIGTERM signal
    // - the actor to finish/panic
    // - the axum server to finish
    select! {
        _ = sigterm_stream.recv() => {
            Err(Box::from("Received a SIGTERM signal! exiting."))
        },
        res = actor_handle => {
            match res {
                Ok(msg) => { println!("{msg}"); }
                Err(err) => { println!("{err}"); }
            }
            Err(Box::from("The actor died! exiting."))
        },
        _ = axum::serve(listener, app).into_future() => {
            Err(Box::from("The server died! exiting."))
        }
    }
}

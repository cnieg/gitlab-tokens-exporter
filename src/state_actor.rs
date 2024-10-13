use core::time::Duration;
use dotenv::dotenv;
use std::env;
use tokio::{
    select,
    sync::{mpsc, oneshot},
    time,
};

use crate::{gitlab, prometheus_metrics};

const DATA_REFRESH_HOURS_DEFAULT: u8 = 6;

#[derive(Debug)]
pub enum ActorMessage {
    GetResponse { respond_to: oneshot::Sender<String> },
}

#[expect(
    clippy::integer_division_remainder_used,
    reason = "Because clippy is not happy with the tokio::select macro #3"
)]
pub async fn gitlab_tokens_actor(mut receiver: mpsc::Receiver<ActorMessage>) -> String {
    let mut response = String::new(); // The is the state this actor is handling
                                      // TODO: this should be an enum!
                                      // enum {
                                      //    Loading,
                                      //    Loaded(String),
                                      //    Error(String)
                                      // }

    dotenv().ok();

    let Ok(gitlab_token) = env::var("GITLAB_TOKEN") else {
        return "env variable GITLAB_TOKEN is not defined".to_owned();
    };
    let Ok(gitlab_baseurl) = env::var("GITLAB_BASEURL") else {
        return "env variable GITLAB_BASEURL is not defined".to_owned();
    };
    let data_refresh_hours =
        env::var("DATA_REFRESH_HOURS").map_or(DATA_REFRESH_HOURS_DEFAULT, |env_value| {
            env_value
                .parse()
                .map_or(DATA_REFRESH_HOURS_DEFAULT, |env_value_u8| {
                    if env_value_u8 > 0 && env_value_u8 <= 24 {
                        env_value_u8
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
                    // TODO: use a function that returns a Result<Vec<Project>, Error>
                    let mut res = String::new();

                    // Create an HTTP client
                    let http_client = reqwest::Client::new();

                    let projects = gitlab::get_all_projects(&http_client, &gitlab_baseurl_clone, &gitlab_token_clone)
                    .await
                    .unwrap_or_else(|err| panic!("Failed to get gitlab projects : {err}"));

                    for project in projects {
                        let project_access_tokens = gitlab::get_project_access_tokens(&http_client, &gitlab_baseurl_clone, &gitlab_token_clone, &project)
                            .await
                            .unwrap_or_else(|err| panic!("Failed to get gitlab token for project {} : {err}", project.path_with_namespace));
                        if !project_access_tokens.is_empty() {
                            println!("{} :", project.path_with_namespace);
                            for project_access_token in project_access_tokens {
                                println!("  {project_access_token:?}");
                                res.push_str(&prometheus_metrics::build(&project, &project_access_token).unwrap_or_default());
                                // TODO: return an Error if prometheus_metrics::build() fails
                            }
                        }
                    }
                    if let Err(_err) = update_sender_clone.send(res) {
                        panic!("send failed");
                    }
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

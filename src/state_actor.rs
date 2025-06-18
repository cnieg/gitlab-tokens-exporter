//! This is the main actor, it handles all [`Message`]

use core::error::Error;
use dotenv::dotenv;
use regex::Regex;
use reqwest::Client;
use std::collections::HashMap;
use std::env;
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinSet;
use tokio::time::Instant;
use tracing::{debug, error, info, instrument, warn};

use crate::gitlab::{Group, OffsetBasedPagination as _, Project, Token, get_group_full_path};
use crate::{gitlab, prometheus_metrics};

/// Default value for `max_concurrent_requests`, which is passed to [`get_gitlab_data`]
const MAX_CONCURRENT_REQUESTS_DEFAULT: u16 = 5;

/// Defines possible states
#[derive(Clone, Debug)]
pub enum ActorState {
    /// Stores an error string if [`get_gitlab_data`] fails
    Error(String),
    /// Stores the string that is returned when requesting `/metrics`
    Loaded(String),
    /// First state when the program starts
    Loading,
    /// Used when no token has been found
    NoToken,
}

/// Defines the messages handled by the state actor
#[derive(Debug)]
pub enum Message {
    /// Get the state and send it to `respond_to`
    Get {
        /// Channel we have to send the state to
        respond_to: oneshot::Sender<ActorState>,
    },
    /// This message is sent by the update task when it finishes
    Set(Result<String, String>),
    /// This message is only send by the [timer](crate::timer) actor
    Update,
}

/// Handles [`send()`](mpsc::Sender::send) result by dismissing it ;)
async fn send_msg(sender: mpsc::Sender<Message>, msg: Message) {
    match sender.send(msg).await {
        Ok(send_res) => send_res,
        Err(err) => {
            // We can't do anything at this point. If this fails, we're in bad shape :(
            error!("Failed to send a message: {err}");
        }
    }
}

#[instrument(skip_all)]
/// Get projects tokens and convert them to prometheus metrics
async fn get_projects_tokens_metrics(
    http_client: Client,
    hostname: &str,
    gitlab_token: &str,
    owned_entities_only: bool,
    max_concurrent_requests: u16,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let time = Instant::now();
    info!("getting projects...");

    let mut res = String::new();

    #[expect(clippy::as_conversions, reason = "AccessLevel::Owner (50) < 256")]
    let url = format!(
        "https://{hostname}/api/v4/projects?per_page=100&archived=false{}",
        if owned_entities_only {
            format!("&min_access_level={}", gitlab::AccessLevel::Owner as u8)
        } else {
            String::new()
        }
    );

    let projects = gitlab::Project::get_all(&http_client, url, gitlab_token).await?;

    info!(
        "got {} project{} in {:?}",
        projects.len(),
        match projects.len() {
            0 | 1 => "",
            _ => "s",
        },
        time.elapsed()
    );

    info!("getting projects tokens");
    #[expect(clippy::shadow_unrelated, reason = "we want to 'reset' time")]
    let time = Instant::now();

    for chunk in projects.chunks(max_concurrent_requests.into()) {
        // For each chunk, we are going to create a JoinSet, so that we can await the completion all of the tasks
        let mut set: JoinSet<Result<String, Box<dyn Error + Send + Sync>>> = JoinSet::new();
        for project in chunk {
            let project_tokens_url = format!(
                "https://{hostname}/api/v4/projects/{}/access_tokens?per_page=100",
                project.id
            );
            set.spawn(get_project_access_tokens_task(
                http_client.clone(),
                gitlab_token.into(),
                project_tokens_url,
                project.clone(),
            ));
        }

        // Now that `set` is initialized, we wait for all the tasks to finish
        // If we get *any* error, the whole function fails
        debug!("waiting for {} tasks to complete", set.len());
        while let Some(join_result) = set.join_next().await {
            match join_result {
                Ok(task_result) => match task_result {
                    Ok(metric_value) => res.push_str(&metric_value),
                    Err(err) => return Err(err),
                },
                Err(err) => return Err(Box::new(err)),
            }
        }
        debug!("tasks completed");
    }

    info!("got all projects tokens in {:?}", time.elapsed());

    Ok(res)
}

#[instrument(skip_all)]
/// This function is used in [`get_projects_tokens_metrics`] as an async task template
async fn get_project_access_tokens_task(
    http_client: Client,
    gitlab_token: String,
    url: String,
    project: Project,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let mut res = String::new();
    let project_tokens = gitlab::AccessToken::get_all(&http_client, url, &gitlab_token).await?;
    for project_token in project_tokens {
        let token = Token::Project {
            token: project_token,
            full_path: project.path_with_namespace.clone(),
            web_url: project.web_url.clone(),
        };
        let token_metric_str = prometheus_metrics::build(&token)?;
        res.push_str(&token_metric_str);
    }
    Ok(res)
}

#[instrument(skip_all)]
/// Get groups tokens and convert them to prometheus metrics
async fn get_groups_tokens_metrics(
    http_client: Client,
    hostname: &str,
    gitlab_token: &str,
    owned_entities_only: bool,
    max_concurrent_requests: u16,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let time = Instant::now();
    info!("getting groups...");

    // This will be used by gitlab::get_group_full_path() to avoid generating multiple API queries for the same group id
    let group_id_cache: Arc<Mutex<HashMap<usize, Group>>> = Arc::new(Mutex::new(HashMap::new()));

    let mut res = String::new();
    #[expect(clippy::as_conversions, reason = "AccessLevel::Owner (50) < 256")]
    let url = format!(
        "https://{hostname}/api/v4/groups?per_page=100&archived=false{}",
        if owned_entities_only {
            format!("&min_access_level={}", gitlab::AccessLevel::Owner as u8)
        } else {
            String::new()
        }
    );

    let groups = gitlab::Group::get_all(&http_client, url, gitlab_token).await?;

    info!(
        "got {} group{} in {:?}",
        groups.len(),
        match groups.len() {
            0 | 1 => "",
            _ => "s",
        },
        time.elapsed()
    );

    info!("getting groups tokens");
    #[expect(clippy::shadow_unrelated, reason = "we want to 'reset' time")]
    let time = Instant::now();

    for chunk in groups.chunks(max_concurrent_requests.into()) {
        // For each chunk, we are going to create a JoinSet, so that we can await the completion all of the tasks
        let mut set: JoinSet<Result<String, Box<dyn Error + Send + Sync>>> = JoinSet::new();
        for group in chunk {
            set.spawn(get_group_access_tokens_task(
                http_client.clone(),
                gitlab_token.into(),
                hostname.into(),
                group.clone(),
                Arc::clone(&group_id_cache),
            ));
        }

        // Now that `set` is initialized, we wait for all the tasks to finish
        // If we get *any* error, the whole function fails
        debug!("waiting for {} tasks to complete", set.len());
        while let Some(join_result) = set.join_next().await {
            match join_result {
                Ok(task_result) => match task_result {
                    Ok(metric_value) => res.push_str(&metric_value),
                    Err(err) => return Err(err),
                },
                Err(err) => return Err(Box::new(err)),
            }
        }
        debug!("tasks completed");
    }

    info!("got all groups tokens in {:?}", time.elapsed());

    Ok(res)
}

#[instrument(skip_all)]
/// This function is used in [`get_groups_tokens_metrics`] as an async task template
async fn get_group_access_tokens_task(
    http_client: Client,
    gitlab_token: String,
    hostname: String,
    group: Group,
    group_id_cache: Arc<Mutex<HashMap<usize, Group>>>,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let mut res = String::new();
    let url = format!(
        "https://{hostname}/api/v4/groups/{}/access_tokens?per_page=100",
        group.id
    );
    let group_tokens = gitlab::AccessToken::get_all(&http_client, url, &gitlab_token).await?;
    for group_token in group_tokens {
        let token = Token::Group {
            token: group_token,
            full_path: get_group_full_path(
                &http_client,
                &hostname,
                &gitlab_token,
                &group,
                &group_id_cache,
            )
            .await?,
            web_url: group.web_url.clone(),
        };
        let token_metric_str = prometheus_metrics::build(&token)?;
        res.push_str(&token_metric_str);
    }
    Ok(res)
}

#[instrument(skip_all)]
/// Get users tokens and convert them to prometheus metrics
async fn get_users_tokens_metrics(
    http_client: Client,
    hostname: &str,
    gitlab_token: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let mut res = String::new();
    let mut url = format!("https://{hostname}/api/v4/users?per_page=100");
    // First, we must check that the token we are using have the necessary rights
    // If not, we return an empty string

    let current_user = gitlab::get_current_user(&http_client, hostname, gitlab_token).await?;
    if current_user.is_admin {
        let time = Instant::now();
        info!("getting users...");

        let users = gitlab::User::get_all(&http_client, url, gitlab_token).await?;

        info!(
            "got {} user{} in {:?}",
            users.len(),
            match users.len() {
                0 | 1 => "",
                _ => "s",
            },
            time.elapsed()
        );

        let human_users_re = Regex::new("(project|group)_[0-9]+_bot_[0-9a-f]{32,}")?;
        let user_ids: HashMap<_, _> = users
            .iter()
            .filter(|user| !human_users_re.is_match(&user.username))
            .map(|user| (user.id, user.username.clone()))
            .collect();

        // Get all personnal access tokens
        url = format!("https://{hostname}/api/v4/personal_access_tokens?per_page=100");
        let mut personnal_access_tokens =
            gitlab::PersonalAccessToken::get_all(&http_client, url, gitlab_token).await?;
        // Retain personnal access tokens of human users
        personnal_access_tokens.retain(|pat| user_ids.contains_key(&pat.user_id));

        for personnal_access_token in personnal_access_tokens {
            let username = user_ids
                .get(&personnal_access_token.user_id)
                .map_or("", |val| val);
            let token_str = prometheus_metrics::build(&Token::User {
                token: personnal_access_token,
                full_path: username.to_owned(),
            })?;
            res.push_str(&token_str);
        }

        Ok(res)
    } else {
        warn!(
            "Can't get users tokens with the current GITLAB_TOKEN (current_user.is_admin == false)"
        );
        Ok(String::new())
    }
}

#[instrument(skip_all)]
/// Handles [`Message::Update`] messages
///
/// When finished, it sends its result by sending [`Message::Set`] to the main actor
async fn get_gitlab_data(
    hostname: String,
    gitlab_token: String,
    accept_invalid_certs: bool,
    owned_entities_only: bool,
    sender: mpsc::Sender<Message>,
    max_concurrent_requests: u16,
) {
    info!("starting...");

    // This variable will be [`Message::Set`] parameter
    let mut return_value = String::new();

    // Create an HTTP client
    let http_client = match reqwest::ClientBuilder::new()
        .danger_accept_invalid_certs(accept_invalid_certs)
        .build()
    {
        Ok(res) => res,
        Err(err) => {
            let msg = format!("Failed to build an HTTP client: {err}");
            error!(msg);
            send_msg(sender, Message::Set(Err(msg))).await;
            return;
        }
    };

    match get_projects_tokens_metrics(
        http_client.clone(),
        &hostname,
        &gitlab_token,
        owned_entities_only,
        max_concurrent_requests,
    )
    .await
    {
        Ok(value) => return_value.push_str(&value),
        Err(err) => {
            let msg = format!("Failed to get projects tokens: {err}");
            error!(msg);
            send_msg(sender, Message::Set(Err(msg))).await;
            return;
        }
    }

    match get_groups_tokens_metrics(
        http_client.clone(),
        &hostname,
        &gitlab_token,
        owned_entities_only,
        max_concurrent_requests,
    )
    .await
    {
        Ok(value) => return_value.push_str(&value),
        Err(err) => {
            let msg = format!("Failed to get groups tokens: {err}");
            error!(msg);
            send_msg(sender, Message::Set(Err(msg))).await;
            return;
        }
    }

    match get_users_tokens_metrics(http_client, &hostname, &gitlab_token).await {
        Ok(value) => return_value.push_str(&value),
        Err(err) => {
            let msg = format!("Failed to get users tokens: {err:?}");
            error!(msg);
            send_msg(sender, Message::Set(Err(msg))).await;
            return;
        }
    }

    send_msg(sender, Message::Set(Ok(return_value))).await;
    info!("done");
}

#[instrument(skip_all)]
/// Main actor, receives all [`Message`]
pub async fn gitlab_tokens_actor(
    mut receiver: mpsc::Receiver<Message>,
    sender: mpsc::Sender<Message>,
) {
    let mut state = ActorState::Loading;

    let _res = dotenv();

    let Ok(token) = env::var("GITLAB_TOKEN") else {
        error!("env variable GITLAB_TOKEN is not defined");
        return;
    };
    let Ok(hostname) = env::var("GITLAB_HOSTNAME") else {
        error!("env variable GITLAB_HOSTNAME is not defined");
        return;
    };

    // Checking ACCEPT_INVALID_CERTS env variable
    let accept_invalid_cert = match env::var("ACCEPT_INVALID_CERTS") {
        Ok(value) => {
            if value == "yes" {
                true
            } else {
                error!(
                    "The environment variable 'ACCEPT_INVALID_CERTS' is set, but not to its only possible value : 'yes'"
                );
                return;
            }
        }
        Err(_) => false,
    };

    // Checking OWNED_ENTITIES_ONLY env variable
    let owned_entities_only = match env::var("OWNED_ENTITIES_ONLY") {
        Ok(value) => {
            if value == "yes" {
                true
            } else {
                error!(
                    "The environment variable 'OWNED_ENTITIES_ONLY' is set, but not to its only possible value : 'yes'"
                );
                return;
            }
        }
        Err(_) => false,
    };

    // Checking MAX_CONCURRENT_REQUESTS env variable
    let max_concurrent_requests = env::var("MAX_CONCURRENT_REQUESTS")
        .map_or(MAX_CONCURRENT_REQUESTS_DEFAULT, |value| {
            value.parse().unwrap_or(MAX_CONCURRENT_REQUESTS_DEFAULT)
        });

    // We now wait for some messages
    loop {
        let msg = receiver.recv().await;
        if let Some(msg_value) = msg {
            match msg_value {
                Message::Get { respond_to } => {
                    debug!("received Message::Get");
                    respond_to.send(state.clone()).unwrap_or_else(|_| {
                        warn!("Failed to send reponse : oneshot channel was closed");
                    });
                }
                Message::Update => {
                    // We are going to spawn a async task to get the data from gitlab.
                    // This task will send us Message::Set with the result to
                    // update our 'state' variable
                    debug!("received Message::Update");
                    tokio::spawn(get_gitlab_data(
                        hostname.clone(),
                        token.clone(),
                        accept_invalid_cert,
                        owned_entities_only,
                        sender.clone(),
                        max_concurrent_requests,
                    ));
                }
                Message::Set(gitlab_data) => {
                    debug!("received Message::Set");
                    match gitlab_data {
                        Ok(data) => {
                            if data.is_empty() {
                                warn!("No token has been found");
                                state = ActorState::NoToken;
                            } else {
                                state = ActorState::Loaded(data);
                            }
                        }
                        Err(err) => state = ActorState::Error(err),
                    }
                }
            }
        } else {
            error!("recv failed");
            break;
        }
    }
}

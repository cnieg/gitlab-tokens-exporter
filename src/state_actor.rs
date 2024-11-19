//! This is the main actor, it handles all [Message]

use dotenv::dotenv;
use std::env;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, info, instrument, warn};

use crate::gitlab;
use crate::gitlab::{OffsetBasedPagination, Tokens};

/// Defines the messages handled by the state actor
#[derive(Debug)]
pub enum Message {
    /// Get the state and send it to `respond_to`
    Get {
        /// Channel we have to send the state to
        respond_to: oneshot::Sender<ActorState>,
    },
    /// This message is only send by the [timer](crate::timer) actor
    Update,
    /// This message is sent by the update task when it finishes
    Set(Result<String, String>),
}

/// Defines possible states
#[derive(Clone, Debug)]
pub enum ActorState {
    /// First state when the program starts
    Loading,
    /// Used when no token has been found
    NoToken,
    /// Stores the string that is returned when requesting `/metrics`
    Loaded(String),
    /// Stores an error string if [`gitlab_get_data`] fails
    Error(String),
}

/// Handles `send()` result by dismissing it ;)
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
/// Handles [Message::Update] messages
///
/// When finished, it sends its result by sending Message::Set to the main actor
async fn gitlab_get_data(
    hostname: String,
    token: String,
    accept_invalid_certs: bool,
    sender: mpsc::Sender<Message>,
) {
    info!("Starting...");

    let mut ok_return_value = String::new();

    // Create an HTTP client
    let http_client = match reqwest::ClientBuilder::new()
        .danger_accept_invalid_certs(accept_invalid_certs)
        .build()
    {
        Ok(res) => res,
        Err(err) => {
            error!("{err}");
            send_msg(sender, Message::Set(Err(format!("{err:?}")))).await;
            return;
        }
    };

    // TODO: Spawn 3 tasks to speed up the data collection (3 requests at a time)
    // One to get the projects tokens
    // One to get the groups tokens
    // One to get the users tokens

    // Get all projects
    let mut url = format!("https://{hostname}/api/v4/projects?per_page=100&archived=false");
    let projects = match gitlab::Project::get_all(&http_client, url, &token).await {
        Ok(res) => res,
        Err(err) => {
            let msg = format!("Failed to get all projects: {err:?}");
            error!(msg);
            send_msg(sender, Message::Set(Err(msg))).await;
            return;
        }
    };

    // Get access tokens for each project
    for project in projects {
        let project_tokens = match project.get_tokens(&http_client, &hostname, &token).await {
            Ok(res) => res,
            Err(err) => {
                let msg = format!("Failed to get tokens for all projects: {err:?}");
                error!(msg);
                send_msg(sender, Message::Set(Err(msg))).await;
                return;
            }
        };
        ok_return_value.push_str(&project_tokens);
    }

    // Get gitlab groups
    url = format!("https://{hostname}/api/v4/groups?per_page=100");
    let groups = match gitlab::Group::get_all(&http_client, url, &token).await {
        Ok(res) => res,
        Err(err) => {
            let msg = format!("Failed to get all groups: {err:?}");
            error!(msg);
            send_msg(sender, Message::Set(Err(msg))).await;
            return;
        }
    };

    // Get access tokens for each group
    for group in groups {
        let group_access_tokens = match group.get_tokens(&http_client, &hostname, &token).await {
            Ok(res) => res,
            Err(err) => {
                error!("{err}");
                send_msg(sender, Message::Set(Err(format!("{err:?}")))).await;
                return;
            }
        };
        ok_return_value.push_str(&group_access_tokens);
    }

    info!("end.");

    send_msg(sender, Message::Set(Ok(ok_return_value))).await;
}

#[instrument(skip_all)]
/// Main actor, receives all [Message]
pub async fn gitlab_tokens_actor(
    mut receiver: mpsc::Receiver<Message>,
    sender: mpsc::Sender<Message>,
) {
    let mut state = ActorState::Loading;

    dotenv().ok().take();

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
                error!("The environment variable 'ACCEPT_INVALID_CERTS' is set, but not to its only value : 'yes'");
                return;
            }
        }
        Err(_) => false,
    };

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
                    tokio::spawn(gitlab_get_data(
                        hostname.clone(),
                        token.clone(),
                        accept_invalid_cert,
                        sender.clone(),
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

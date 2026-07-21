//! This is the main actor, it handles all [`Message`]

use anyhow::Context as _;
use regex::Regex;
use std::collections::HashMap;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinSet;
use tokio::time::Instant;
use tracing::{debug, error, info, instrument, warn};

use crate::config::CONFIG;
use crate::gitlab::group::Group;
use crate::gitlab::pagination::{GitLabResourceLister, TokenFetcher};
use crate::gitlab::project::Project;
use crate::gitlab::token::{PersonalAccessToken, Token};
use crate::gitlab::user::{self, User};
use crate::prometheus_metrics;

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
            error!("failed to send a message: {err}");
        }
    }
}

#[instrument(skip_all, err)]
/// Get tokens from a [`Project`] or a [`Group`] and convert them to prometheus metrics
async fn get_tokens_metrics<T>() -> Result<String, anyhow::Error>
where
    T: for<'serde> serde::Deserialize<'serde> + GitLabResourceLister<T> + TokenFetcher + Clone,
{
    info!("getting {}s", T::type_name());

    let mut time = Instant::now();
    let mut res = String::new();

    let items = T::get_all()
        .await
        .with_context(|| format!("failed to get {}s", T::type_name()))?;

    info!(
        "got {} {}{} in {:?}",
        items.len(),
        T::type_name(),
        match items.len() {
            0 | 1 => "",
            _ => "s",
        },
        time.elapsed()
    );

    info!("getting {} tokens", T::type_name());

    time = Instant::now();

    for chunk in items.chunks(CONFIG.max_concurrent_requests.div_euclid(2).into()) {
        // For each chunk, we are going to create a JoinSet, so that we can await the completion all of the tasks
        let mut set: JoinSet<Result<String, anyhow::Error>> = JoinSet::new();
        for item in chunk {
            // TODO: I didn't find a way to get a chunk of owned Ts... (maybe with something other that a Vec<T> ?)
            // not possible with a Vec : cf https://github.com/rust-lang/rust/issues/40708
            // maybe using `array_chunks` when it'ss stabilized ? https://doc.rust-lang.org/std/iter/trait.Iterator.html#method.array_chunks
            set.spawn(get_access_tokens_task(item.clone()));
        }

        // Now that `set` is initialized, we wait for all the tasks to finish
        // If we get *any* error, the whole function fails
        debug!("waiting for {} tasks to complete", set.len());
        while let Some(join_result) = set.join_next().await {
            let task_result = join_result.context("failed to join task")?;

            match task_result {
                Ok(metric_value) => res.push_str(&metric_value),
                Err(err) => return Err(err),
            }
        }
        debug!("tasks completed");
    }

    info!("got all tokens in {:?}", time.elapsed());

    Ok(res)
}

#[instrument(skip_all, err)]
/// This function is used in [`get_tokens_metrics`] as an async task template
///
/// `resource` is a specific [`Project`] or [`Group`]
async fn get_access_tokens_task<T>(resource: T) -> Result<String, anyhow::Error>
where
    T: TokenFetcher,
{
    let mut res = String::new();

    let tokens = resource
        .get_all_tokens()
        .await
        .with_context(|| format!("failed to get tokens for project {}", resource.name()))?;

    for token in tokens {
        if !(CONFIG.skip_non_expiring_tokens && token.expires_at.is_none()) {
            let generic_token = resource.create_generic_token(token).await?;
            let token_metric_str =
                prometheus_metrics::build(&generic_token).with_context(|| {
                    format!("failed to build prometheus metric for token={generic_token:?}")
                })?;
            res.push_str(&token_metric_str);
        }
    }
    Ok(res)
}

#[instrument(skip_all, err)]
/// Get users tokens and convert them to prometheus metrics
async fn get_users_tokens_metrics() -> Result<String, anyhow::Error> {
    info!("starting");

    let mut res = String::new();

    // First, we must check that the token we are using have the necessary rights
    // If not, we return an empty string

    let current_user = user::get_current()
        .await
        .context("failed to get current user")?;
    if current_user.is_admin {
        let time = Instant::now();
        info!("getting users");

        let users = User::get_all().await.context("failed to get users")?;

        info!(
            "got {} user{} in {:?}",
            users.len(),
            match users.len() {
                0 | 1 => "",
                _ => "s",
            },
            time.elapsed()
        );

        let human_users_re = Regex::new("(project|group)_[0-9]+_bot_[0-9a-f]{32,}")
            .context("failed to compile human_users_re regex")?;
        let user_ids: HashMap<_, _> = users
            .iter()
            .filter(|user| !human_users_re.is_match(&user.username))
            .filter(|user| match &CONFIG.usernames_filter {
                Some(filter) => filter.contains(&user.username),
                None => true,
            })
            .map(|user| (user.id, user.username.clone()))
            .collect();

        let mut personnal_access_tokens = PersonalAccessToken::get_all()
            .await
            .context("failed to get personnal access tokens")?;
        // Retain personnal access tokens of users listed in `user_ids`
        personnal_access_tokens.retain(|pat| user_ids.contains_key(&pat.user_id));

        if CONFIG.usernames_filter.is_some() && personnal_access_tokens.is_empty() {
            warn!("no token matched USERNAMES_FILTER");
        }

        for personnal_access_token in personnal_access_tokens {
            if !(CONFIG.skip_non_expiring_tokens && personnal_access_token.expires_at.is_none()) {
                let username = user_ids
                    .get(&personnal_access_token.user_id)
                    .map_or("", |val| val);
                let token = Token::User {
                    token: personnal_access_token,
                    full_path: username.to_owned(),
                };
                let token_str = prometheus_metrics::build(&token).with_context(|| {
                    format!("failed to build prometheus metric from token={token:?}")
                })?;
                res.push_str(&token_str);
            }
        }

        Ok(res)
    } else {
        warn!(
            "can't get users tokens with the current GITLAB_TOKEN (current_user.is_admin == false)"
        );
        Ok(String::new())
    }
}

#[instrument(skip_all)]
/// Handles [`Message::Update`] messages
///
/// When finished, it sends its result by sending [`Message::Set`] to the main actor
async fn get_gitlab_data(sender: mpsc::Sender<Message>) {
    info!("starting");

    // This variable will be [`Message::Set`] parameter
    let mut return_value = String::from(
        "# HELP gitlab_token_days_remaining Days before Gitlab token expires\n# TYPE gitlab_token_days_remaining gauge\n",
    );

    // Using a tokio JoinSet to run get_tokens_metrics() twice concurrently
    let mut set: JoinSet<Result<String, anyhow::Error>> = JoinSet::new();

    set.spawn(get_tokens_metrics::<Project>());
    set.spawn(get_tokens_metrics::<Group>());

    if CONFIG.skip_users_tokens {
        debug!("skipping users tokens as requested by SKIP_USERS_TOKENS env variable");
    } else {
        if CONFIG.usernames_filter.is_some() {
            debug!("getting users tokens matching USERNAMES_FILTER");
        } else {
            debug!("getting all users tokens");
        }
        set.spawn(get_users_tokens_metrics());
    }

    // Now that `set` is initialized, we wait for all the tasks to finish
    // If we get *any* error, we send an error message
    debug!("waiting for {} tasks to complete", set.len());
    while let Some(join_result) = set.join_next().await {
        match join_result {
            Ok(task_result) => match task_result {
                Ok(metric_value) => return_value.push_str(&metric_value),
                Err(err) => {
                    let msg = format!("failed to get tokens: {err:?}");
                    error!("{msg}");
                    send_msg(sender, Message::Set(Err(msg))).await;
                    return;
                }
            },
            Err(err) => {
                let msg = format!("failed to join a task: {err}");
                error!("{msg}");
                send_msg(sender, Message::Set(Err(msg))).await;
                return;
            }
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

    // wait for some messages
    loop {
        let Some(msg) = receiver.recv().await else {
            error!("recv failed");
            break;
        };

        match msg {
            Message::Get { respond_to } => {
                debug!("received Message::Get");
                respond_to.send(state.clone()).unwrap_or_else(|_| {
                    warn!("failed to send reponse : oneshot channel was closed");
                });
            }
            Message::Update => {
                // We are going to spawn a async task to get the data from gitlab.
                // This task will send us Message::Set with the result to
                // update our 'state' variable
                debug!("received Message::Update");
                tokio::spawn(get_gitlab_data(sender.clone()));
            }
            Message::Set(gitlab_data) => {
                debug!("received Message::Set");
                match gitlab_data {
                    Ok(data) => {
                        if data.is_empty() {
                            warn!("no token has been found");
                            state = ActorState::NoToken;
                        } else {
                            state = ActorState::Loaded(data);
                        }
                    }
                    Err(err) => state = ActorState::Error(err),
                }
            }
        }
    }
}

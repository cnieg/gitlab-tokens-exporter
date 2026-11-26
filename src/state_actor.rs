//! This is the main actor, it handles all [`Message`]

use anyhow::Context as _;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinSet;
use tokio::time::Instant;
use tracing::{debug, error, info, instrument, warn};

use crate::config::CONFIG;
use crate::gitlab::group::Group;
use crate::gitlab::pagination::get_paginated_items;
use crate::gitlab::project::Project;
use crate::gitlab::root_resource::GitlabRootResource;
use crate::gitlab::service_account::ServiceAccount;
use crate::gitlab::token::access::AccessToken;
use crate::gitlab::token::deploy::DeployToken;
use crate::gitlab::token::personal::PersonalAccessToken;
use crate::gitlab::token::pipeline::PipelineTriggerToken;
use crate::gitlab::token::{SpecificToken, TokenOrigin, TokenSource};
use crate::gitlab::user::{self, User};

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
    T: Clone + GitlabRootResource + TokenSource + DeserializeOwned + Send + Sync + 'static,
{
    info!("getting {}s", T::resource_type_name());

    let mut time = Instant::now();
    let mut res = String::new();

    let items: Vec<T> = get_paginated_items(&T::first_url())
        .await
        .with_context(|| format!("failed to get {}s", T::resource_type_name()))?;

    info!(
        "got {} {}{} in {:?}",
        items.len(),
        T::resource_type_name(),
        match items.len() {
            0 | 1 => "",
            _ => "s",
        },
        time.elapsed()
    );

    info!("getting all {} tokens", T::resource_type_name());

    time = Instant::now();

    let mut set: JoinSet<Result<String, anyhow::Error>> = JoinSet::new();

    for item in items {
        let origin = item.token_origin().await?;
        let access_tokens_url = item.access_tokens_url();
        let deploy_tokens_url = item.deploy_tokens_url();
        let pipeline_trigger_tokens_url = item.pipeline_trigger_tokens_url();

        set.spawn(get_token_metrics_for_origin::<AccessToken>(
            access_tokens_url,
            origin.clone(),
        ));

        set.spawn(get_token_metrics_for_origin::<DeployToken>(
            deploy_tokens_url,
            origin.clone(),
        ));

        if let Some(url) = pipeline_trigger_tokens_url {
            set.spawn(get_token_metrics_for_origin::<PipelineTriggerToken>(
                url, origin,
            ));
        }

        // We are now going get service accounts
        let service_accounts_url = item.service_accounts_url();
        let service_accounts: Vec<ServiceAccount> = get_paginated_items(&service_accounts_url)
            .await
            .with_context(|| {
                format!("failed to get service accounts from {service_accounts_url}")
            })?;

        #[expect(
            clippy::shadow_unrelated,
            reason = "we won't use the original `origin` here"
        )]
        for service_account in service_accounts {
            let service_account_pat_url = item.service_account_pat_url(service_account.id);
            let origin = item.service_account_origin(&service_account).await?;
            set.spawn(get_token_metrics_for_origin::<PersonalAccessToken>(
                service_account_pat_url,
                origin,
            ));
        }
    }

    // Now that `set` is initialized, we wait for all the tasks to finish
    // If we get *any* error, the whole function fails
    debug!("waiting for {} tasks to complete", set.len());
    while let Some(join_result) = set.join_next().await {
        res.push_str(&join_result.context("failed to join task")??);
    }
    debug!("tasks completed");

    info!(
        "got all {} tokens in {:?}",
        T::resource_type_name(),
        time.elapsed()
    );

    Ok(res)
}

/// Get token (of type `T`) metrics for a specific [`TokenOrigin`], starting from `url`
async fn get_token_metrics_for_origin<T>(
    url: String,
    origin: TokenOrigin,
) -> Result<String, anyhow::Error>
where
    T: DeserializeOwned + SpecificToken + Send + 'static,
{
    let tokens: Vec<T> = get_paginated_items(&url).await.with_context(|| {
        format!(
            "failed to get {} {} tokens",
            origin.type_name(),
            T::type_name()
        )
    })?;

    tokens
        .into_iter()
        .filter(|token| !(CONFIG.skip_non_expiring_tokens && token.is_non_expiring()))
        .map(|token| String::try_from(&token.into_token(origin.clone())))
        .collect()
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
        info!("getting {}s", User::resource_type_name());

        let users: Vec<User> = get_paginated_items(&User::first_url())
            .await
            .context("failed to get users")?;

        info!(
            "got {} user{} in {:?}",
            users.len(),
            match users.len() {
                0 | 1 => "",
                _ => "s",
            },
            time.elapsed()
        );

        let user_ids: HashMap<_, _> = users
            .iter()
            .filter(|user| !CONFIG.bot_users_re.is_match(&user.username))
            .filter(|user| match &CONFIG.usernames_filter {
                Some(filter) => filter.contains(&user.username),
                None => true,
            })
            .map(|user| (user.id, user.username.as_str()))
            .collect();

        let mut personnal_access_tokens: Vec<PersonalAccessToken> =
            get_paginated_items(&PersonalAccessToken::first_url())
                .await
                .context("failed to get personnal access tokens")?;

        // Retain personnal access tokens of users listed in `user_ids`
        personnal_access_tokens.retain(|pat| user_ids.contains_key(&pat.user_id));

        if CONFIG.usernames_filter.is_some() && personnal_access_tokens.is_empty() {
            warn!("no token matched USERNAMES_FILTER");
        }

        for personnal_access_token in personnal_access_tokens
            .into_iter()
            .filter(|token| !(CONFIG.skip_non_expiring_tokens && token.is_non_expiring()))
        {
            let username = user_ids
                .get(&personnal_access_token.user_id)
                .map_or("", |val| val);

            let origin = TokenOrigin::User {
                user_name: username.to_owned(),
            };
            let token_string = String::try_from(&personnal_access_token.into_token(origin))
                .context("failed to build prometheus metric from personnal access token")?;

            res.push_str(&token_string);
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
    let time = Instant::now();

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

    info!("done! got all tokens in {:?}", time.elapsed());
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

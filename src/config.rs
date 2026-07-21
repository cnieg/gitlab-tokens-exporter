//! Creates the exporter's [`Config`] in the static variable [`CONFIG`]

use core::time::Duration;
use std::{collections::HashSet, env, sync::LazyLock};

use anyhow::{Context as _, anyhow};
use dotenvy::dotenv;
use tracing::{instrument, warn};

use crate::gitlab::connection::Connection;

/// Default value for `max_concurrent_requests`
const MAX_CONCURRENT_REQUESTS_DEFAULT: u16 = 10;

/// Default value for `data_refresh_hours`
const DATA_REFRESH_HOURS_DEFAULT: u8 = 6;

/// Default number of times a transient gitlab API error is retried
const MAX_RETRIES_DEFAULT: u32 = 4;

/// Default base delay (in milliseconds) for the retry exponential backoff
const RETRY_BACKOFF_MS_DEFAULT: u64 = 500;

/// This config will be available to all tasks
#[expect(clippy::unwrap_used, reason = "we *want* to crash if this fails")]
pub static CONFIG: LazyLock<Config> = LazyLock::new(|| Config::new().unwrap());

/// Defines the exporter's configuration
#[derive(Clone)]
pub struct Config {
    /// Connection to gitlab
    pub connection: Connection,
    /// Time interval between updates
    pub data_refresh_hours: u8,
    /// Total (for **all** tasks) number of concurrent requests
    pub max_concurrent_requests: u16,
    /// Only handle owned tokens if set to `true`
    pub owned_entities_only: bool,
    /// Skip non expiring tokens if set to `true`
    pub skip_non_expiring_tokens: bool,
    /// Skip users tokens if set to `true`
    pub skip_users_tokens: bool,
    /// Filter users tokens by username
    pub usernames_filter: Option<HashSet<String>>,
}

impl Config {
    #[instrument(skip_all, err)]
    /// Creates a new [`Config`]
    pub fn new() -> Result<Self, anyhow::Error> {
        let _res = dotenv();

        let Ok(token) = env::var("GITLAB_TOKEN") else {
            return Err(anyhow!("env variable GITLAB_TOKEN is not defined"));
        };
        let Ok(hostname) = env::var("GITLAB_HOSTNAME") else {
            return Err(anyhow!("env variable GITLAB_HOSTNAME is not defined"));
        };

        // Checking ACCEPT_INVALID_CERTS env variable
        let accept_invalid_certs = get_bool_or_false("ACCEPT_INVALID_CERTS")?;

        // Checking OWNED_ENTITIES_ONLY env variable
        let owned_entities_only = get_bool_or_false("OWNED_ENTITIES_ONLY")?;

        // Checking MAX_CONCURRENT_REQUESTS env variable
        let max_concurrent_requests = env::var("MAX_CONCURRENT_REQUESTS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(MAX_CONCURRENT_REQUESTS_DEFAULT);

        // Checking SKIP_USERS_TOKENS env variable
        let skip_users_tokens = get_bool_or_false("SKIP_USERS_TOKENS")?;

        // Checking USERNAMES_FILTER env variable
        let usernames_filter = get_usernames_filter()?;

        if skip_users_tokens && usernames_filter.is_some() {
            warn!("USERNAMES_FILTER is ignored because SKIP_USERS_TOKENS is set to yes");
        }

        // Checking SKIP_NON_EXPIRING_TOKENS env variable
        let skip_non_expiring_tokens = get_bool_or_false("SKIP_NON_EXPIRING_TOKENS")?;

        let data_refresh_hours = env::var("DATA_REFRESH_HOURS")
            .ok()
            .and_then(|env_value| env_value.parse().ok())
            .filter(|env_value_u8| *env_value_u8 > 0 && *env_value_u8 <= 24)
            .unwrap_or(DATA_REFRESH_HOURS_DEFAULT);

        // Checking MAX_RETRIES env variable
        let max_retries = env::var("MAX_RETRIES")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(MAX_RETRIES_DEFAULT);

        // Checking RETRY_BACKOFF_MS env variable
        let retry_backoff_ms = env::var("RETRY_BACKOFF_MS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(RETRY_BACKOFF_MS_DEFAULT);

        let connection = Connection::new(
            hostname,
            token,
            accept_invalid_certs,
            max_retries,
            Duration::from_millis(retry_backoff_ms),
        )
        .context("failed to create gitlab_connection")?;

        Ok(Self {
            connection,
            data_refresh_hours,
            max_concurrent_requests,
            owned_entities_only,
            skip_non_expiring_tokens,
            skip_users_tokens,
            usernames_filter,
        })
    }
}

/// Returns the boolean value of `env_var_name`, or `false` if the
/// environment variable is not defined
fn get_bool_or_false(env_var_name: &str) -> Result<bool, anyhow::Error> {
    match env::var(env_var_name).as_deref() {
        Ok("yes") => Ok(true),
        Ok("no") => Ok(false),
        Ok(value) => Err(anyhow!(
            "invalid value for '{env_var_name}': '{value}'. expected 'yes' or 'no'.",
        )),
        Err(err) => match err {
            env::VarError::NotPresent => Ok(false),
            env::VarError::NotUnicode(value) => Err(anyhow!(
                "invalid value for '{env_var_name}': '{}'. expected 'yes' or 'no'.",
                value.display()
            )),
        },
    }
}

/// Returns the usernames configured in `USERNAMES_FILTER`,
/// or `None` if the environment variable is not defined.
fn get_usernames_filter() -> Result<Option<HashSet<String>>, anyhow::Error> {
    match env::var("USERNAMES_FILTER") {
        Ok(value) => {
            let users = value
                .split(',')
                .map(|item| item.trim().to_owned())
                .filter(|user| !user.is_empty())
                .collect();

            Ok(Some(users))
        }
        Err(err) => match err {
            env::VarError::NotPresent => Ok(None),
            env::VarError::NotUnicode(value) => Err(anyhow!(
                "invalid value for 'USERNAMES_FILTER': '{}'.",
                value.display()
            )),
        },
    }
}

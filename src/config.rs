//! Creates the exporter's [`Config`]

use std::env;

use anyhow::{Context as _, anyhow};
use dotenvy::dotenv;
use tracing::instrument;

use crate::gitlab::connection::Connection;

/// Default value for `max_concurrent_requests`
const MAX_CONCURRENT_REQUESTS_DEFAULT: u16 = 10;

/// Default value for `data_refresh_hours`
const DATA_REFRESH_HOURS_DEFAULT: u8 = 6;

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
        let accept_invalid_certs = match env::var("ACCEPT_INVALID_CERTS").as_deref() {
            Ok("yes") => true,
            Ok(value) => {
                return Err(anyhow!(
                    "invalid value for 'ACCEPT_INVALID_CERTS': '{value}'. expected 'yes'.",
                ));
            }
            Err(_) => false,
        };

        // Checking OWNED_ENTITIES_ONLY env variable
        let owned_entities_only = match env::var("OWNED_ENTITIES_ONLY").as_deref() {
            Ok("yes") => true,
            Err(_) => false,
            Ok(value) => {
                return Err(anyhow!(
                    "invalid value for 'OWNED_ENTITIES_ONLY': '{value}'. expected 'yes'.",
                ));
            }
        };

        // Checking MAX_CONCURRENT_REQUESTS env variable
        let max_concurrent_requests = env::var("MAX_CONCURRENT_REQUESTS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(MAX_CONCURRENT_REQUESTS_DEFAULT);

        // Checking SKIP_USERS_TOKENS env variable
        let skip_users_tokens = match env::var("SKIP_USERS_TOKENS").as_deref() {
            Ok("yes") => true,
            Ok("no") | Err(_) => false,
            Ok(value) => {
                return Err(anyhow!(
                    "invalid value for 'SKIP_USERS_TOKENS': '{value}'. expected 'yes' or 'no'.",
                ));
            }
        };

        // Checking SKIP_NON_EXPIRING_TOKENS env variable
        let skip_non_expiring_tokens = match env::var("SKIP_NON_EXPIRING_TOKENS").as_deref() {
            Ok("yes") => true,
            Ok("no") | Err(_) => false,
            Ok(value) => {
                return Err(anyhow!(
                    "invalid value for 'SKIP_USERS_TOKENS': '{value}'. expected 'yes' or 'no'.",
                ));
            }
        };

        let data_refresh_hours = env::var("DATA_REFRESH_HOURS")
            .ok()
            .and_then(|env_value| env_value.parse().ok())
            .filter(|env_value_u8| *env_value_u8 > 0 && *env_value_u8 <= 24)
            .unwrap_or(DATA_REFRESH_HOURS_DEFAULT);

        let connection = Connection::new(hostname, token, accept_invalid_certs)
            .context("failed to create gitlab_connection")?;

        Ok(Self {
            connection,
            data_refresh_hours,
            max_concurrent_requests,
            owned_entities_only,
            skip_non_expiring_tokens,
            skip_users_tokens,
        })
    }
}

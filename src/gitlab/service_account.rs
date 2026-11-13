//! gitlab group service accounts and their personal access tokens
//!
//! Service accounts are a Premium/Ultimate feature and live on top-level groups.
//! Listing them (and their tokens) requires the Owner role on the group, which is
//! why this whole code path is gated behind the `SCAN_SERVICE_ACCOUNTS` feature
//! flag (see [`Config`](crate::config::Config)).

use anyhow::Context as _;
use reqwest::StatusCode;
use serde::Deserialize;
use tracing::{debug, instrument};

use crate::{
    config::CONFIG,
    gitlab::{pagination::get_all_gitlab_items, token::PersonalAccessToken},
};

/// Defines a [gitlab group service account](https://docs.gitlab.com/api/group_service_accounts/)
#[derive(Debug, Deserialize)]
pub struct ServiceAccount {
    /// Service account user id
    pub id: usize,
    /// Service account username
    pub username: String,
}

impl ServiceAccount {
    /// Lists the personal access tokens of this service account in `group_id`.
    #[instrument(skip_all, err)]
    pub async fn get_all_tokens(
        &self,
        group_id: usize,
    ) -> Result<Vec<PersonalAccessToken>, anyhow::Error> {
        let url = format!(
            "https://{}/api/v4/groups/{group_id}/service_accounts/{}/personal_access_tokens?per_page=100",
            CONFIG.connection.hostname, self.id
        );
        get_all_gitlab_items(&url).await
    }

    /// Lists the service accounts of a (top-level) group.
    ///
    /// Returns an empty vec when the group has no service accounts, when the
    /// token is not Owner of the group, or when the tier has the feature
    /// disabled (`403`/`404`). This way a single inaccessible group does not
    /// fail the whole scan.
    ///
    /// Only the first page is read (`per_page=100`); a single group is not
    /// expected to hold more service accounts than that.
    #[instrument(skip_all, err)]
    pub async fn list_for_group(group_id: usize) -> Result<Vec<Self>, anyhow::Error> {
        let url = format!(
            "https://{}/api/v4/groups/{group_id}/service_accounts?per_page=100",
            CONFIG.connection.hostname
        );

        // We do this first request ourselves (instead of get_all_gitlab_items)
        // so that we can turn 403/404 into "no service accounts here" rather
        // than a hard error that aborts the whole scan.
        let response = CONFIG
            .connection
            .http_client
            .get(&url)
            .header("PRIVATE-TOKEN", &CONFIG.connection.token)
            .send()
            .await
            .with_context(|| format!("failed to GET {url}"))?;

        if matches!(
            response.status(),
            StatusCode::FORBIDDEN | StatusCode::NOT_FOUND
        ) {
            debug!(
                "group {group_id}: service accounts not accessible ({}), skipping",
                response.status()
            );
            return Ok(Vec::new());
        }

        let raw_json = response
            .error_for_status()
            .with_context(|| format!("URL {url} returned an error"))?
            .text()
            .await
            .with_context(|| format!("failed to get response text from {url}"))?;

        let service_accounts: Vec<Self> = serde_json::from_str(&raw_json)
            .with_context(|| format!("failed to decode raw_json={raw_json}"))?;

        Ok(service_accounts)
    }
}

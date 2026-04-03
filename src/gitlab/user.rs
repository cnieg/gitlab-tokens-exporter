//! Defines a gitab user

use anyhow::Context as _;
use serde::Deserialize;
use tracing::{debug, instrument};

use crate::gitlab::{connection::Connection, pagination::OffsetBasedPagination};

/// Defines a [gitlab user](https://docs.gitlab.com/api/users/#list-users)
#[derive(Debug, Deserialize)]
pub struct User {
    /// User id
    pub id: usize,
    /// This field is not available if the query is made with a non-admin token
    #[serde(default)]
    pub is_admin: bool,
    /// User name (without spaces)
    pub username: String,
}

#[expect(clippy::missing_trait_methods, reason = "we don't need it")]
impl OffsetBasedPagination<Self> for User {}

/// Get the current gitlab user
#[instrument(skip_all, err)]
pub async fn get_current(connection: &Connection) -> Result<User, anyhow::Error> {
    let current_url = format!("https://{}/api/v4/user", connection.hostname);

    debug!("getting current user");

    let resp = connection
        .http_client
        .get(&current_url)
        .header("PRIVATE-TOKEN", &connection.token)
        .send()
        .await
        .with_context(|| format!("failed to GET {current_url}"))?
        .error_for_status()
        .with_context(|| format!("URL {current_url} returned an error"))?;

    let raw_json = resp
        .text()
        .await
        .with_context(|| format!("failed to get response text from {current_url}"))?;

    let user = serde_json::from_str(&raw_json)
        .with_context(|| format!("failed to decode raw_json={raw_json}"))?;

    Ok(user)
}

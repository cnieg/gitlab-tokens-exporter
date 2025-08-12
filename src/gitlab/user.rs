//! Defines a gitab user

use serde::Deserialize;
use tracing::{debug, instrument};

use crate::{
    error::BoxedError,
    gitlab::{connection::Connection, pagination::OffsetBasedPagination},
};

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
pub async fn get_current(connection: &Connection) -> Result<User, BoxedError> {
    let current_url = format!("https://{}/api/v4/user", connection.hostname);

    debug!("getting current user");

    let resp = connection
        .http_client
        .get(&current_url)
        .header("PRIVATE-TOKEN", &connection.token)
        .send()
        .await?
        .error_for_status()?;

    let raw_json = resp.text().await?;

    let user = serde_json::from_str(&raw_json).map_err(|err| {
        #[expect(clippy::absolute_paths, reason = "Use a specific Error type")]
        std::io::Error::other(format!("error decoding raw_json={raw_json} : {err}"))
    })?;

    Ok(user)
}

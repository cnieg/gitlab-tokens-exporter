//! gitab service account

use serde::Deserialize;

/// Defines a [gitlab service account](https://docs.gitlab.com/api/service_accounts/)
#[derive(Debug, Deserialize)]
pub struct ServiceAccount {
    /// Service account user id
    pub id: u64,
    /// Service account name
    pub name: String,
    /// Service account username
    pub username: String,
}

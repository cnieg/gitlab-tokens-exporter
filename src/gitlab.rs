//! Handles the communication with gitlab

use core::error::Error;
use core::fmt::{Display, Formatter};
use serde::Deserialize;
use serde_repr::Deserialize_repr;
use tracing::{error, instrument};

/// cf <https://docs.gitlab.com/api/project_access_tokens/#create-a-project-access-token>
#[derive(Debug, Deserialize_repr)]
#[repr(u8)]
pub enum AccessLevel {
    Developer = 30,
    Guest = 10,
    Maintainer = 40,
    Owner = 50,
    Reporter = 20,
}

impl Display for AccessLevel {
    #[expect(clippy::min_ident_chars, reason = "Parameter name from std trait")]
    #[expect(clippy::absolute_paths, reason = "Use a specific Result type")]
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{}",
            match *self {
                Self::Guest => "guest",
                Self::Reporter => "reporter",
                Self::Developer => "developer",
                Self::Maintainer => "maintainer",
                Self::Owner => "owner",
            },
        )
    }
}

/// Defines a [gitlab access token](https://docs.gitlab.com/api/project_access_tokens/#list-project-access-tokens)
#[derive(Debug, Deserialize)]
pub struct AccessToken {
    /// Access level
    pub access_level: AccessLevel,
    /// Active
    pub active: bool,
    /// Expiration date
    pub expires_at: chrono::NaiveDate,
    /// Name
    pub name: String,
    /// Revoked
    pub revoked: bool,
    /// Scopes
    pub scopes: Vec<String>,
}

#[expect(clippy::missing_trait_methods, reason = "we don't need it")]
impl OffsetBasedPagination<Self> for AccessToken {}

/// Defines a [gitlab group](https://docs.gitlab.com/api/groups/)
#[derive(Debug, Deserialize)]
pub struct Group {
    /// Group id
    pub id: usize,
    /// Group path
    pub path: String,
}

#[expect(clippy::missing_trait_methods, reason = "we don't need it")]
impl OffsetBasedPagination<Self> for Group {}

/// cf <https://docs.gitlab.com/api/rest/#offset-based-pagination>
pub trait OffsetBasedPagination<T: for<'serde> serde::Deserialize<'serde>> {
    #[instrument(skip_all)]
    async fn get_all(
        http_client: &reqwest::Client,
        url: String,
        token: &str,
    ) -> Result<Vec<T>, Box<dyn Error + Send + Sync>> {
        let mut result: Vec<T> = Vec::new();
        let mut next_url: Option<String> = Some(url);

        while let Some(ref current_url) = next_url {
            let resp = http_client
                .get(current_url)
                .header("PRIVATE-TOKEN", token)
                .send()
                .await?;

            let err_copy = resp.error_for_status_ref().map(|_| ()); // Keep the error for later if needed
            match resp.error_for_status_ref() {
                Ok(_) => {
                    next_url = resp
                        .headers()
                        .get("link")
                        .and_then(|header_value| header_value.to_str().ok())
                        .and_then(|header_value_str| {
                            parse_link_header::parse_with_rel(header_value_str).ok()
                        })
                        .and_then(|links| links.get("next").map(|link| link.raw_uri.clone()));

                    let mut items: Vec<T> = resp.json().await?;
                    result.append(&mut items);
                }
                Err(err) => {
                    error!(
                        "{} - {} : {}",
                        current_url,
                        err.status().unwrap_or_default(),
                        resp.text().await?
                    );
                    err_copy?; // This will exit the function with the original error
                }
            }
        }

        Ok(result)
    }
}

/// Defines a [gitlab personal access token](https://docs.gitlab.com/api/personal_access_tokens/#list-personal-access-tokens)
#[derive(Debug, Deserialize)]
pub struct PersonalAccessToken {
    /// Active
    pub active: bool,
    /// Expiration date
    pub expires_at: chrono::NaiveDate,
    /// Name
    pub name: String,
    /// Revoked
    pub revoked: bool,
    /// Scopes
    pub scopes: Vec<String>,
    /// User id
    pub user_id: usize,
}

#[expect(clippy::missing_trait_methods, reason = "we don't need it")]
impl OffsetBasedPagination<Self> for PersonalAccessToken {}

/// Defines a [gitlab project](https://docs.gitlab.com/api/projects/#get-a-single-project)
#[derive(Debug, Deserialize)]
pub struct Project {
    /// Project id
    pub id: usize,
    /// Project path
    pub path_with_namespace: String,
}

#[expect(clippy::missing_trait_methods, reason = "we don't need it")]
impl OffsetBasedPagination<Self> for Project {}

#[derive(Debug)]
/// A common token type
///
/// The second field is used to identify where a token comes from
pub enum Token {
    /// Group token
    Group(AccessToken, String),
    /// Project token
    Project(AccessToken, String),
    /// User token
    User(PersonalAccessToken, String),
}

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
#[instrument(skip_all)]
pub async fn get_current_user(
    http_client: &reqwest::Client,
    hostname: &str,
    token: &str,
) -> Result<User, Box<dyn Error + Send + Sync>> {
    let current_url = format!("https://{hostname}/api/v4/user");
    let resp = http_client
        .get(&current_url)
        .header("PRIVATE-TOKEN", token)
        .send()
        .await?;

    match resp.error_for_status_ref() {
        Ok(_) => {
            let user: User = resp.json().await?;
            Ok(user)
        }
        Err(err) => {
            error!(
                "{} - {} : {}",
                current_url,
                err.status().unwrap_or_default(),
                resp.text().await?
            );
            Err(Box::new(err))
        }
    }
}

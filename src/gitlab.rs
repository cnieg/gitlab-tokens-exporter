//! Handles the communication with gitlab

use core::error::Error;
use core::fmt::{Display, Formatter};
use serde::Deserialize;
use serde_repr::Deserialize_repr;
use tracing::{error, info, instrument};

use crate::prometheus_metrics;

/// Defines a gitlab project
#[derive(Debug, Deserialize)]
pub struct Project {
    /// Project id
    pub id: usize,
    /// Project path
    pub path_with_namespace: String,
}

/// cf <https://docs.gitlab.com/ee/api/project_access_tokens.html#create-a-project-access-token>
#[derive(Debug, Deserialize_repr)]
#[repr(u8)]
pub enum AccessLevel {
    Guest = 10,
    Reporter = 20,
    Developer = 30,
    Maintainer = 40,
    Owner = 50,
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

/// cf <https://docs.gitlab.com/ee/api/project_access_tokens.html#list-project-access-tokens>
#[derive(Debug, Deserialize)]
pub struct AccessToken {
    /// Scopes
    pub scopes: Vec<String>,
    /// Name
    pub name: String,
    /// Expiration date
    pub expires_at: chrono::NaiveDate,
    /// Active
    pub active: bool,
    /// Revoked
    pub revoked: bool,
    /// Access level
    pub access_level: AccessLevel,
}

/// cf <https://docs.gitlab.com/ee/api/personal_access_tokens.html#list-personal-access-tokens>
#[derive(Debug, Deserialize)]
pub struct PersonalAccessToken {
    /// Scopes
    pub scopes: Vec<String>,
    /// Name
    pub name: String,
    /// Expiration date
    pub expires_at: chrono::NaiveDate,
    /// Active
    pub active: bool,
    /// Revoked
    pub revoked: bool,
    /// User id
    pub user_id: usize,
}

/// Defines a gitlab group
#[derive(Debug, Deserialize)]
pub struct Group {
    /// Group id
    pub id: usize,
    /// Group path
    pub path: String,
}

/// Defines a gitlab user
#[derive(Debug, Deserialize)]
pub struct User {
    /// User id
    pub id: usize,
    /// User name (without spaces)
    pub username: String,
    /// This field is not available if the query is made with a non-admin token
    #[serde(default)]
    pub is_admin: bool,
}

/// cf <https://docs.gitlab.com/ee/api/rest/#offset-based-pagination>
pub trait OffsetBasedPagination<T: for<'serde> serde::Deserialize<'serde>> {
    #[instrument(skip_all)]
    async fn get_all(
        http_client: &reqwest::Client,
        url: String,
        token: &str,
    ) -> Result<Vec<T>, Box<dyn Error + Send + Sync>> {
        let mut result: Vec<T> = Vec::new();
        let mut next_url: Option<String> = Some(url);

        #[expect(clippy::ref_patterns, reason = "I don't know how to make clippy happy")]
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

#[expect(clippy::missing_trait_methods, reason = "we don't need it")]
impl OffsetBasedPagination<Self> for Project {}
#[expect(clippy::missing_trait_methods, reason = "we don't need it")]
impl OffsetBasedPagination<Self> for AccessToken {}
#[expect(clippy::missing_trait_methods, reason = "we don't need it")]
impl OffsetBasedPagination<Self> for Group {}
#[expect(clippy::missing_trait_methods, reason = "we don't need it")]
impl OffsetBasedPagination<Self> for User {}
#[expect(clippy::missing_trait_methods, reason = "we don't need it")]
impl OffsetBasedPagination<Self> for PersonalAccessToken {}

#[derive(Debug)]
/// Used to identify where a specific comes from
pub enum TokenType {
    /// Project token
    Project(AccessToken),
    /// Group token
    Group(AccessToken),
    /// User token
    User(PersonalAccessToken),
}

impl Display for TokenType {
    #[expect(clippy::min_ident_chars, reason = "Parameter name from std trait")]
    #[expect(clippy::absolute_paths, reason = "Specific Result type")]
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{}",
            match *self {
                Self::Project(_) => "project",
                Self::Group(_) => "group",
                Self::User(_) => "user",
            },
        )
    }
}

/// A way to get tokens for a particular type (Projects, Groups). Users tokens are not handled here
pub trait Tokens {
    /// Get all tokens for a particular type
    async fn get_tokens(
        &self,
        http_client: &reqwest::Client,
        hostname: &str,
        token: &str,
    ) -> Result<String, Box<dyn Error + Send + Sync>>;
}

impl Tokens for Project {
    async fn get_tokens(
        &self,
        http_client: &reqwest::Client,
        hostname: &str,
        token: &str,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        let url = format!(
            "https://{hostname}/api/v4/projects/{}/access_tokens?per_page=100",
            self.id
        );
        let project_access_tokens = AccessToken::get_all(http_client, url, token).await?;
        let mut ok_return_value = String::new();
        for project_access_token in project_access_tokens {
            info!("{}: {project_access_token:?}", self.path_with_namespace);
            let token_str = prometheus_metrics::build(
                &self.path_with_namespace,
                TokenType::Project(project_access_token),
            )?;
            ok_return_value.push_str(&token_str);
        }
        Ok(ok_return_value)
    }
}

impl Tokens for Group {
    async fn get_tokens(
        &self,
        http_client: &reqwest::Client,
        hostname: &str,
        token: &str,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        let url = format!(
            "https://{hostname}/api/v4/groups/{}/access_tokens?per_page=100",
            self.id
        );
        let group_access_tokens = AccessToken::get_all(http_client, url, token).await?;
        let mut ok_return_value = String::new();
        for group_access_token in group_access_tokens {
            info!("{}: {group_access_token:?}", self.path);
            let token_str =
                prometheus_metrics::build(&self.path, TokenType::Group(group_access_token))?;
            ok_return_value.push_str(&token_str);
        }
        Ok(ok_return_value)
    }
}

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

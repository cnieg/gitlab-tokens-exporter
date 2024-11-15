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

/// Defines a gitlab group
#[derive(Debug, Deserialize)]
pub struct Group {
    /// Group id
    pub id: usize,
    /// Group path
    pub path: String,
}

/// cf <https://docs.gitlab.com/ee/api/rest/#offset-based-pagination>
pub trait OffsetBasedPagination<T: for<'serde> serde::Deserialize<'serde>> {
    #[instrument(skip_all, target = "gitlab")]
    async fn get_all(
        http_client: &reqwest::Client,
        url: String,
        gitlab_token: &str,
    ) -> Result<Vec<T>, Box<dyn Error + Send + Sync>> {
        let mut result: Vec<T> = Vec::new();
        let mut next_url: Option<String> = Some(url);

        #[expect(clippy::ref_patterns, reason = "I don't know how to make clippy happy")]
        while let Some(ref current_url) = next_url {
            let resp = http_client
                .get(current_url)
                .header("PRIVATE-TOKEN", gitlab_token)
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

/// A way to get tokens for a particuliar type (Projects, Groups, Users)
pub trait Tokens {
    /// Get all tokens for a particuliar type
    async fn get_tokens(
        &self,
        http_client: &reqwest::Client,
        base_url: &str,
        gitlab_token: &str,
    ) -> Result<String, Box<dyn Error + Send + Sync>>;
    /// Get token type name
    fn get_type_name(&self) -> &'static str;
}

impl Tokens for Project {
    async fn get_tokens(
        &self,
        http_client: &reqwest::Client,
        base_url: &str,
        gitlab_token: &str,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        let url = format!(
            "https://{base_url}/api/v4/projects/{}/access_tokens?per_page=100",
            self.id
        );
        let project_access_tokens = AccessToken::get_all(http_client, url, gitlab_token).await?;
        let mut ok_return_value = String::new();
        for project_access_token in project_access_tokens {
            info!("{}: {project_access_token:?}", self.path_with_namespace);
            let token_str = prometheus_metrics::build(
                &self.path_with_namespace,
                self.get_type_name(),
                &project_access_token,
            )?;
            ok_return_value.push_str(&token_str);
        }
        Ok(ok_return_value)
    }
    fn get_type_name(&self) -> &'static str {
        "project"
    }
}

impl Tokens for Group {
    async fn get_tokens(
        &self,
        http_client: &reqwest::Client,
        base_url: &str,
        gitlab_token: &str,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        let url = format!(
            "https://{base_url}/api/v4/groups/{}/access_tokens?per_page=100",
            self.id
        );
        let group_access_tokens = AccessToken::get_all(http_client, url, gitlab_token).await?;
        let mut ok_return_value = String::new();
        for group_access_token in group_access_tokens {
            info!("{}: {group_access_token:?}", self.path);
            let token_str =
                prometheus_metrics::build(&self.path, self.get_type_name(), &group_access_token)?;
            ok_return_value.push_str(&token_str);
        }
        Ok(ok_return_value)
    }
    fn get_type_name(&self) -> &'static str {
        "group"
    }
}

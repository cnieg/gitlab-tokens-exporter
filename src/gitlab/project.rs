//! gitab project definition and traits implementations

use serde::Deserialize;

use crate::{
    config::CONFIG,
    gitlab::{
        pagination::{GitLabResourceLister, TokenFetcher},
        token,
    },
};

/// Defines a [gitlab project](https://docs.gitlab.com/api/projects/#get-a-single-project)
#[derive(Clone, Debug, Deserialize)]
pub struct Project {
    /// Project id
    pub id: usize,
    /// Project path
    pub path_with_namespace: String,
    /// Project URL
    pub web_url: String,
}

impl GitLabResourceLister<Self> for Project {
    fn first_url() -> String {
        format!(
            "https://{}/api/v4/projects?per_page=100&archived=false{}",
            CONFIG.connection.hostname,
            if CONFIG.owned_entities_only {
                format!("&min_access_level={}", token::AccessLevel::Owner)
            } else {
                String::new()
            }
        )
    }
}

impl TokenFetcher for Project {
    async fn create_generic_token(
        &self,
        token: token::AccessToken,
    ) -> Result<token::Token, anyhow::Error> {
        Ok(token::Token::Project {
            token,
            full_path: self.path_with_namespace.clone(),
            web_url: self.web_url.clone(),
        })
    }

    fn first_url(&self) -> String {
        format!(
            "https://{}/api/v4/projects/{}/access_tokens?per_page=100",
            CONFIG.connection.hostname, self.id
        )
    }

    fn name(&self) -> String {
        self.path_with_namespace.clone()
    }

    fn type_name() -> &'static str {
        "project"
    }
}

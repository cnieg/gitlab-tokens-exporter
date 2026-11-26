//! gitab project definition and traits implementations

use serde::Deserialize;

use crate::{
    config::CONFIG,
    gitlab::{
        root_resource::GitlabRootResource,
        service_account::ServiceAccount,
        token::{TokenOrigin, TokenSource, access::AccessLevel},
    },
};

/// Defines a [gitlab project](https://docs.gitlab.com/api/projects/#get-a-single-project)
#[derive(Clone, Debug, Deserialize)]
pub struct Project {
    /// Project id
    pub id: u64,
    /// Project path
    pub path_with_namespace: String,
    /// Project URL
    pub web_url: String,
}

impl GitlabRootResource for Project {
    #[expect(clippy::as_conversions, reason = "AccessLevel values are < 255")]
    fn first_url() -> String {
        format!(
            "https://{}/api/v4/projects?per_page=100&archived=false{}",
            CONFIG.connection.hostname,
            if CONFIG.owned_entities_only {
                // `min_access_level` requires an integer, so we use `as u8` to get the numeric value
                format!("&min_access_level={}", AccessLevel::Owner as u8)
            } else {
                String::new()
            }
        )
    }

    fn resource_type_name() -> &'static str {
        "project"
    }
}

#[expect(
    clippy::unused_async_trait_impl,
    reason = "this impl doesn't call any other async function"
)]
impl TokenSource for Project {
    fn access_tokens_url(&self) -> String {
        format!(
            "https://{}/api/v4/projects/{}/access_tokens?per_page=100",
            CONFIG.connection.hostname, self.id
        )
    }

    fn deploy_tokens_url(&self) -> String {
        format!(
            "https://{}/api/v4/projects/{}/deploy_tokens?per_page=100",
            CONFIG.connection.hostname, self.id
        )
    }

    fn pipeline_trigger_tokens_url(&self) -> Option<String> {
        Some(format!(
            "https://{}/api/v4/projects/{}/triggers?per_page=100",
            CONFIG.connection.hostname, self.id
        ))
    }

    async fn service_account_origin(
        &self,
        service_account: &ServiceAccount,
    ) -> Result<TokenOrigin, anyhow::Error> {
        Ok(TokenOrigin::ProjectServiceAccount {
            project_name: self.path_with_namespace.clone(),
            service_account_name: service_account.name.clone(),
            service_account_username: service_account.username.clone(),
            web_url: self.web_url.clone(),
        })
    }

    fn service_account_pat_url(&self, service_account_id: u64) -> String {
        format!(
            "https://{}/api/v4/projects/{}/service_accounts/{service_account_id}/personal_access_tokens?per_page=100",
            CONFIG.connection.hostname, self.id
        )
    }

    fn service_accounts_url(&self) -> String {
        format!(
            "https://{}/api/v4/projects/{}/service_accounts?per_page=100",
            CONFIG.connection.hostname, self.id
        )
    }

    async fn token_origin(&self) -> Result<TokenOrigin, anyhow::Error> {
        Ok(TokenOrigin::Project {
            project_name: self.path_with_namespace.clone(),
            web_url: self.web_url.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        config::CONFIG, gitlab::project::Project, gitlab::root_resource::GitlabRootResource as _,
    };

    #[test]
    fn project_min_access_level_parameter() {
        if CONFIG.owned_entities_only {
            let url = Project::first_url();
            assert!(url.contains("/api/v4/projects"));
            assert!(url.ends_with("&min_access_level=50"));
        } else {
            panic!("this test needs the env var OWNED_ENTITIES_ONLY=yes")
        }
    }
}

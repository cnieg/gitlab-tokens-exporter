//! gitab group definition and traits implementations

use anyhow::Context as _;
use serde::Deserialize;
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
};
use tracing::{debug, instrument};

use crate::{
    config::CONFIG,
    gitlab::{
        pagination::{GitLabResourceLister, TokenFetcher},
        token,
    },
};

/// Group id cache. Used in [`Group::get_full_path`]
static GROUP_ID_CACHE: LazyLock<Mutex<HashMap<usize, Group>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Defines a [gitlab group](https://docs.gitlab.com/api/groups/)
#[derive(Clone, Debug, Deserialize)]
pub struct Group {
    /// Group id
    pub id: usize,
    /// Group parent id
    pub parent_id: Option<usize>,
    /// Group path
    pub path: String,
    /// Group URL
    pub web_url: String,
}

impl Group {
    /// Creates a string containing `group` full path
    ///
    /// Because the gitlab API gives us `path_with_namespace` for [`projects`](crate::gitlab::project::Project) but not for [`groups`](crate::gitlab::group::Group)
    #[expect(
        clippy::unwrap_used,
        reason = "
    this function calls unwrap() if the mutex is poisoned, crashing is ok in our case
"
    )]
    #[instrument(skip_all, err)]
    pub async fn get_full_path(&self) -> Result<String, anyhow::Error> {
        debug!("group: {self:?}");

        // This variable will contain the String returned by this function
        let mut res = self.path.clone();

        // This variable will be overwritten in the while loop below
        let mut tmp_group = GROUP_ID_CACHE
            .lock()
            .unwrap()
            .entry(self.id)
            .or_insert_with(|| self.clone())
            .clone();

        while let Some(parent_group_id) = tmp_group.parent_id {
            if let Some(cached_group) = GROUP_ID_CACHE.lock().unwrap().get(&parent_group_id) {
                debug!("parent group id {parent_group_id} found in cache");
                tmp_group = cached_group.clone();
            } else {
                // We have to query gitlab
                debug!("getting group id {parent_group_id} from gitlab");

                let url = format!(
                    "https://{}/api/v4/groups/{parent_group_id}",
                    CONFIG.connection.hostname
                );
                let resp = CONFIG
                    .connection
                    .http_client
                    .get(&url)
                    .header("PRIVATE-TOKEN", &CONFIG.connection.token)
                    .send()
                    .await
                    .with_context(|| format!("failed to GET {url}"))?
                    .error_for_status()
                    .with_context(|| format!("URL {url} returned an error"))?;

                let raw_json = resp
                    .text()
                    .await
                    .with_context(|| format!("failed to get response text from {url}"))?;

                let group_from_gitlab: Self = serde_json::from_str(&raw_json)
                    .with_context(|| format!("failed to decode raw_json={raw_json}"))?;

                // Storing the result in the cache
                tmp_group = GROUP_ID_CACHE
                    .lock()
                    .unwrap()
                    .entry(group_from_gitlab.id)
                    .or_insert_with(|| group_from_gitlab.clone())
                    .clone();

                debug!("group id {parent_group_id} inserted in cache");
            }

            res = format!("{}/{res}", tmp_group.path);
        }

        Ok(res)
    }
}

impl GitLabResourceLister<Self> for Group {
    fn first_url() -> String {
        format!(
            "https://{}/api/v4/groups?per_page=100&archived=false{}",
            CONFIG.connection.hostname,
            if CONFIG.owned_entities_only {
                format!("&min_access_level={}", token::AccessLevel::Owner)
            } else {
                String::new()
            }
        )
    }
}

impl TokenFetcher for Group {
    async fn create_generic_token(
        &self,
        token: token::AccessToken,
    ) -> Result<token::Token, anyhow::Error> {
        Ok(token::Token::Group {
            token,
            full_path: self
                .get_full_path()
                .await
                .context("failed to get full path")?,
            web_url: self.web_url.clone(),
        })
    }

    fn first_url(&self) -> String {
        format!(
            "https://{}/api/v4/groups/{}/access_tokens?per_page=100",
            CONFIG.connection.hostname, self.id
        )
    }

    fn name(&self) -> String {
        self.path.clone()
    }

    fn type_name() -> &'static str {
        "group"
    }
}

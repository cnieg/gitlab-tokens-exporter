//! Defines a gitab group

use anyhow::Context as _;
use serde::Deserialize;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tracing::{debug, instrument};

use crate::{config::CONFIG, gitlab::pagination::OffsetBasedPagination};

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

#[expect(clippy::missing_trait_methods, reason = "we don't need it")]
impl OffsetBasedPagination<Self> for Group {}

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
pub async fn get_full_path(
    group: &Group,
    cache: &Arc<Mutex<HashMap<usize, Group>>>,
) -> Result<String, anyhow::Error> {
    debug!("group: {group:?}");

    // This variable will contain the String returned by this function
    let mut res = group.path.clone();

    // This variable will be overwritten in the while loop below
    let mut tmp_group = cache
        .lock()
        .unwrap()
        .entry(group.id)
        .or_insert_with(|| group.clone())
        .clone();

    while let Some(parent_group_id) = tmp_group.parent_id {
        if let Some(cached_group) = cache.lock().unwrap().get(&parent_group_id) {
            tmp_group = cached_group.clone();
        } else {
            // We have to query gitlab
            debug!("getting group {parent_group_id} from gitlab");

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

            let group_from_gitlab: Group = serde_json::from_str(&raw_json)
                .with_context(|| format!("failed to decode raw_json={raw_json}"))?;

            // Storing the result in the cache
            tmp_group = cache
                .lock()
                .unwrap()
                .entry(group_from_gitlab.id)
                .or_insert_with(|| group_from_gitlab.clone())
                .clone();
        }

        res = format!("{}/{res}", tmp_group.path);
    }

    Ok(res)
}

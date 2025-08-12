//! Defines a gitab group

use serde::Deserialize;
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tracing::{debug, instrument};

use crate::{
    error::BoxedError,
    gitlab::{connection::Connection, pagination::OffsetBasedPagination},
};

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
    This function calls unwrap() for 2 reasons:
      - If the mutex is poisoned, crashing is ok in our case
      - There is another call to unwrap() but it is safe to do because we check if the Option is_none()
        (The 'else' branch we are in is therefore guranteed to be Some())
"
)]
#[instrument(skip_all, err)]
pub async fn get_full_path(
    connection: &Connection,
    group: &Group,
    cache: &Arc<Mutex<HashMap<usize, Group>>>,
) -> Result<String, BoxedError> {
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
        let cached_group = match cache.lock().unwrap().entry(parent_group_id) {
            Occupied(entry) => Some(entry.get().clone()),
            Vacant(_) => None,
        };

        if cached_group.is_none() {
            // We have to query gitlab
            debug!("Getting group {parent_group_id} from gitlab");

            let resp = connection
                .http_client
                .get(format!(
                    "https://{}/api/v4/groups/{parent_group_id}",
                    connection.hostname
                ))
                .header("PRIVATE-TOKEN", &connection.token)
                .send()
                .await?
                .error_for_status()?;

            let raw_json = resp.text().await?;

            let group_from_gitlab: Group = serde_json::from_str(&raw_json).map_err(|err| {
                #[expect(clippy::absolute_paths, reason = "Use a specific Error type")]
                std::io::Error::other(format!("error decoding raw_json={raw_json} : {err}"))
            })?;

            // Storing the result in the cache
            tmp_group = cache
                .lock()
                .unwrap()
                .entry(group_from_gitlab.id)
                .or_insert_with(|| group_from_gitlab.clone())
                .clone();
        } else {
            tmp_group = cached_group.unwrap();
        }

        res = format!("{}/{res}", tmp_group.path);
    }

    Ok(res)
}

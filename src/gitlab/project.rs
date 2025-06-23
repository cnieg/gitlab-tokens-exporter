//! Defines a gitab project

use serde::Deserialize;

use crate::gitlab::pagination::OffsetBasedPagination;

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

#[expect(clippy::missing_trait_methods, reason = "we don't need it")]
impl OffsetBasedPagination<Self> for Project {}

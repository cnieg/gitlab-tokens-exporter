//! Retrieve resources (projects, groups and users) and the associated tokens using gitlab offset based pagination

use anyhow::Context as _;
use core::future::Future;
use tracing::{debug, instrument};

use crate::{
    config::CONFIG,
    gitlab::token::{AccessToken, Token},
};

/// Trait used to get [`Project`](crate::gitlab::project::Project), [`Group`](crate::gitlab::group::Group), [`User`](crate::gitlab::user::User) and [`PersonalAccessToken`](crate::gitlab::token)
pub trait GitLabResourceLister<T: for<'serde> serde::Deserialize<'serde> + GitLabResourceLister<T>>
{
    /// This function must return the URL of the first page to get a list of `T`
    fn first_url() -> String;
    /// Returns `Vec<T>` using [`get_all_gitlab_items`], starting from [`first_url`](GitLabResourceLister::first_url)
    async fn get_all() -> Result<Vec<T>, anyhow::Error> {
        let first_url = T::first_url();
        get_all_gitlab_items(&first_url).await
    }
}

/// Trait used to fetch tokens from a specific [`Project`](crate::gitlab::project::Project) or [`Group`](crate::gitlab::group::Group)
pub trait TokenFetcher: Send + Sync + 'static {
    /// Generates a (common) [`Token`] from an [`AccessToken`]
    fn create_generic_token(
        &self,
        token: AccessToken,
    ) -> impl Future<Output = Result<Token, anyhow::Error>> + Send;

    /// This function must return the URL of the first page to get a list of [`AccessToken`]
    fn first_url(&self) -> String;

    /// Get tokens for a specific [`Project`](crate::gitlab::project::Project) or [`Group`](crate::gitlab::group::Group), starting from [`first_url`](TokenFetcher::first_url)
    fn get_all_tokens(
        &self,
    ) -> impl Future<Output = Result<Vec<AccessToken>, anyhow::Error>> + Send {
        async {
            let first_url = self.first_url();
            get_all_gitlab_items(&first_url).await
        }
    }

    /// [`Project`](crate::gitlab::project::Project) or [`Group`](crate::gitlab::group::Group) name
    fn name(&self) -> String;

    /// Name of the type ("project" or "group")
    fn type_name() -> &'static str;
}

#[instrument(skip_all, err)]
/// Starting from `start_url`, get all the items, using the 'link' header to go through all the pages
/// cf <https://docs.gitlab.com/api/rest/#offset-based-pagination>
async fn get_all_gitlab_items<T: for<'serde> serde::Deserialize<'serde>>(
    start_url: &str,
) -> Result<Vec<T>, anyhow::Error> {
    let mut result: Vec<T> = Vec::new();
    let mut next_url: Option<String> = Some(start_url.to_owned());

    while let Some(current_url) = next_url {
        debug!("trying to GET {current_url}");

        let resp = CONFIG
            .connection
            .http_client
            .get(&current_url)
            .header("PRIVATE-TOKEN", &CONFIG.connection.token)
            .send()
            .await
            .with_context(|| format!("failed to GET {current_url}"))?
            .error_for_status()
            .with_context(|| format!("URL {current_url} returned an error"))?;

        debug!("Got a response for {current_url}");

        next_url = resp
            .headers()
            .get("link")
            .and_then(|header_value| header_value.to_str().ok())
            .and_then(|header_value_str| parse_link_header::parse_with_rel(header_value_str).ok())
            .and_then(|mut links| links.remove("next").map(|link| link.raw_uri));

        debug!(?next_url);

        let raw_json = resp
            .text()
            .await
            .with_context(|| format!("failed to get response text from {current_url}"))?;

        let mut items: Vec<T> = serde_json::from_str(&raw_json)
            .with_context(|| format!("failed to decode raw_json={raw_json}"))?;

        result.append(&mut items);
    }

    debug!("done! (start_url was {start_url})");

    Ok(result)
}

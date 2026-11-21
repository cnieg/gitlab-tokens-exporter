//! Retrieve resources (projects, groups and users) and the associated tokens using gitlab offset based pagination

use anyhow::Context as _;
use serde::de::DeserializeOwned;
use tracing::{debug, instrument};

use crate::config::CONFIG;

#[instrument(skip_all, err)]
/// Starting from `start_url`, get all the items, using the 'link' header to go through all the pages
/// cf <https://docs.gitlab.com/api/rest/#offset-based-pagination>
pub async fn get_paginated_items<T>(start_url: &str) -> Result<Vec<T>, anyhow::Error>
where
    T: DeserializeOwned,
{
    let mut result: Vec<T> = Vec::new();
    let mut next_url: Option<String> = Some(start_url.to_owned());

    while let Some(current_url) = next_url {
        let permit = CONFIG
            .concurrent_requests_semaphore
            .acquire()
            .await
            .context("failed to acquire `concurrent_requests_semaphore`")?;

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

        drop(permit);
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

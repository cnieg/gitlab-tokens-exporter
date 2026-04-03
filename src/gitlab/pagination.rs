//! Implements gitlab offset based pagination

use anyhow::Context as _;
use tracing::{debug, instrument};

use crate::gitlab::connection::Connection;

/// cf <https://docs.gitlab.com/api/rest/#offset-based-pagination>
pub trait OffsetBasedPagination<T: for<'serde> serde::Deserialize<'serde>> {
    #[instrument(skip_all, err)]
    /// Starting from `url`, get all the items, using the 'link' header to go through all the pages
    async fn get_all(connection: &Connection, url: &str) -> Result<Vec<T>, anyhow::Error> {
        let mut result: Vec<T> = Vec::new();
        let mut next_url: Option<String> = Some(url.to_owned());

        debug!("starting");

        while let Some(current_url) = next_url {
            debug!("trying to GET {current_url}");

            let resp = connection
                .http_client
                .get(&current_url)
                .header("PRIVATE-TOKEN", &connection.token)
                .send()
                .await
                .with_context(|| format!("failed to GET {current_url}"))?
                .error_for_status()
                .with_context(|| format!("URL {current_url} returned an error"))?;

            next_url = resp
                .headers()
                .get("link")
                .and_then(|header_value| header_value.to_str().ok())
                .and_then(|header_value_str| {
                    parse_link_header::parse_with_rel(header_value_str).ok()
                })
                .and_then(|mut links| links.remove("next").map(|link| link.raw_uri));

            let raw_json = resp
                .text()
                .await
                .with_context(|| format!("failed to get response text from {current_url}"))?;

            let mut items: Vec<T> = serde_json::from_str(&raw_json)
                .with_context(|| format!("failed to decode raw_json={raw_json}"))?;

            result.append(&mut items);
        }

        debug!("ok!");

        Ok(result)
    }
}

//! Implements gitlab offset based pagination

use tracing::{debug, instrument};

use crate::{error::BoxedError, gitlab::connection::Connection};

/// cf <https://docs.gitlab.com/api/rest/#offset-based-pagination>
pub trait OffsetBasedPagination<T: for<'serde> serde::Deserialize<'serde>> {
    #[instrument(skip_all, err)]
    /// Starting from `url`, get all the items, using the 'link' header to go through all the pages
    async fn get_all(connection: &Connection, url: String) -> Result<Vec<T>, BoxedError> {
        let mut result: Vec<T> = Vec::new();
        let mut next_url: Option<String> = Some(url);

        debug!("starting");

        while let Some(ref current_url) = next_url {
            debug!("trying to GET {current_url}");

            let resp = connection
                .http_client
                .get(current_url)
                .header("PRIVATE-TOKEN", &connection.token)
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

                    let raw_json = resp.text().await?;

                    let mut items: Vec<T> = serde_json::from_str(&raw_json).map_err(|err| {
                        #[expect(clippy::absolute_paths, reason = "Use a specific Error type")]
                        std::io::Error::other(format!("error decoding raw_json={raw_json} : {err}"))
                    })?;
                    result.append(&mut items);
                }
                Err(_) => {
                    err_copy?; // This will exit the function with the original error
                }
            }
        }

        debug!("Ok!");

        Ok(result)
    }
}

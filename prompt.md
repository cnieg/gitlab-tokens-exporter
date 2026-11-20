I want you to help me design some rust structs, traits and functions that are reading data from gitlab's API.
This code uses the tokio async runtime.

The following already exists:

- a function to get all items of type `T`:
  ```rust
  /// Starting from `start_url`, get all the items, using the 'link' header to go through all the pages
  async fn get_paginated_items<T: serde::de::DeserializeOwned>(start_url: &str) -> Result<Vec<T>, anyhow::Error>
  ```

  so, to be able to use this function, each resource must know its first URL.
  This method is using a semaphore so that concurrency is limited. It is a free function.

Existing structs :
- `struct Project`
- `struct Group`
- `struct AccessToken`
- `struct PersonalAccessToken`
- `struct DeployToken`
- `struct TriggerToken`
- `struct ServiceAccount`

with all that, I want you to design an async function that does the following (using multiple tokio tasks so that they run in parallel when possible):

1. get all `Project` (URL is `/projects`)
2. get all `Group` (URL is `/groups`)
3. for each `Project`, get all `AccessToken` (URL is `/projects/:project_id/access_tokens`), `TriggerToken` (URL is `/projects/:project_id/triggers`), `DeployToken` (URL is `/projects/:project_id/deploy_tokens`) and `ServiceAccount` (URL is `/projects/:project_id/service_accounts`)
4. for each `ServiceAccount` in a `Project`, get all `PersonalAccessToken` (URL is like /projects/:project_id/service_accounts/:service_account_id/personal_access_tokens)
5. for each `Group`, get all `AccessToken` (URL is `/groups/:group_id/access_tokens`), `DeployToken` (URL is `/groups/:group_id/deploy_tokens`) and `ServiceAccount` (URL is `/groups/:group_id/service_accounts`)
6. for each `ServiceAccount` in a `Group`, get all `PersonalAccessToken` (URL is /groups/:group_id/service_accounts/:service_account_id/personal_access_tokens)

All those tokens **must** be represented as a `Token` enum **with** its origin (Project, Group, Project ServiceAccount, Group ServiceAccount) so that I can later generate detailed prometheus metrics from this `Token` type

You don't need to write all the code, juste the types, enum, traits, relevant functions, impl and the general shape of the code (you can use`todo!()`), but you **must** add doc comments everywhere!

if one operation fails, we must exit early

the code **must** be idiomatic rust code, must be easily readable and **must** avoid duplication (by using generics whenever possible)

ask me if you need more information or have any question

---
```rust
/// Represents the origin of a token discovered through the GitLab API.
#[derive(Debug, Clone)]
pub enum TokenOrigin {
    /// A token belonging directly to a project.
    Project {
        /// The name of the project.
        project_name: String,
    },

    /// A token belonging directly to a group.
    Group {
        /// The name of the group.
        group_name: String,
    },

    /// A personal access token belonging to a project service account.
    ProjectServiceAccount {
        /// The name of the project.
        project_name: String,

        /// The name of the service account.
        service_account_name: String,
    },

    /// A personal access token belonging to a group service account.
    GroupServiceAccount {
        /// The name of the group.
        group_name: String,

        /// The name of the service account.
        service_account_name: String,
    },
}

/// Represents any token discovered through the GitLab API,
/// together with its origin.
#[derive(Debug, Clone)]
pub enum Token {
    /// A project or group access token.
    AccessToken {
        /// The underlying access token.
        token: AccessToken,

        /// Where the token was discovered.
        origin: TokenOrigin,
    },

    /// A project or group deploy token.
    DeployToken {
        /// The underlying deploy token.
        token: DeployToken,

        /// Where the token was discovered.
        origin: TokenOrigin,
    },

    /// A project trigger token.
    TriggerToken {
        /// The underlying trigger token.
        token: TriggerToken,

        /// Where the token was discovered.
        origin: TokenOrigin,
    },

    /// A personal access token belonging to a service account.
    PersonalAccessToken {
        /// The underlying personal access token.
        token: PersonalAccessToken,

        /// Where the token was discovered.
        origin: TokenOrigin,
    },
}

/// Describes a GitLab resource that can own tokens and service accounts.
///
/// Implementations provide the URLs needed to discover tokens belonging
/// to that resource.
pub trait TokenSource {
    /// Returns the URL used to retrieve access tokens.
    fn access_tokens_url(&self) -> String;

    /// Returns the URL used to retrieve deploy tokens.
    fn deploy_tokens_url(&self) -> String;

    /// Returns the URL used to retrieve service accounts.
    fn service_accounts_url(&self) -> String;

    /// Returns the URL used to retrieve trigger tokens, if supported.
    fn trigger_tokens_url(&self) -> Option<String>;

    /// Returns the URL used to retrieve personal access tokens
    /// belonging to a service account.
    fn personal_access_tokens_url(
        &self,
        service_account_id: u64,
    ) -> String;

    /// Returns the origin associated with tokens belonging directly
    /// to this resource.
    fn token_origin(&self) -> TokenOrigin;

    /// Returns the origin associated with tokens belonging to
    /// a service account of this resource.
    fn service_account_origin(
        &self,
        service_account_id: u64,
    ) -> TokenOrigin;
}

/// Controls how [`get_paginated_items`] handles `403 Forbidden` and `404 Not Found`
/// responses from GitLab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchPolicy {
    /// Treat every non-success HTTP status (including `403` and `404`) as an error.
    Required,
    /// Treat `403 Forbidden` or `404 Not Found` on `start_url` as an empty result,
    /// indicating that the GitLab feature is disabled for that project or group.
    AllowDisabled,
}

#[instrument(skip_all, err)]
/// Starting from `start_url`, get all the items, using the `'link'` header to go through all the pages.
///
/// When `policy` is [`FetchPolicy::AllowDisabled`], a `403 Forbidden` or `404 Not Found`
/// response on `start_url` resolves to an empty [`Vec`] without logging an error.
///
/// cf. <https://docs.gitlab.com/api/rest/#offset-based-pagination>
async fn get_paginated_items<T>(
    start_url: &str,
    policy: FetchPolicy,
) -> Result<Vec<T>, anyhow::Error>
where
    T: for<'serde> serde::Deserialize<'serde>,
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
            .with_context(|| format!("failed to GET {current_url}"))?;

        if policy == FetchPolicy::AllowDisabled
            && current_url == start_url
            && matches!(resp.status(), StatusCode::FORBIDDEN | StatusCode::NOT_FOUND)
        {
            debug!(
                status = %resp.status(),
                "feature disabled or unavailable at {current_url}, returning empty list"
            );
            return Ok(Vec::new());
        }

        let resp = resp
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
```

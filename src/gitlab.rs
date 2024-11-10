use core::error::Error;
use core::fmt::{Display, Formatter};
use serde::Deserialize;
use serde_repr::Deserialize_repr;
use tracing::instrument;

#[derive(Debug, Deserialize)]
pub struct Project {
    pub id: usize,
    pub path_with_namespace: String,
}

#[derive(Debug, Deserialize_repr)]
#[repr(u8)]
pub enum AccessLevel {
    Guest = 10,
    Reporter = 20,
    Developer = 30,
    Maintainer = 40,
    Owner = 50,
}

impl Display for AccessLevel {
    #[expect(clippy::min_ident_chars, reason = "Parameter name from std trait")]
    #[expect(clippy::absolute_paths, reason = "Use a specific Result type")]
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{}",
            match *self {
                Self::Guest => "guest",
                Self::Reporter => "reporter",
                Self::Developer => "developer",
                Self::Maintainer => "maintainer",
                Self::Owner => "owner",
            },
        )
    }
}

#[derive(Debug, Deserialize)]
pub struct AccessToken {
    pub scopes: Vec<String>,
    pub name: String,
    pub expires_at: chrono::NaiveDate,
    pub active: bool,
    pub revoked: bool,
    pub access_level: AccessLevel,
}

#[instrument(err, skip_all, target = "gitlab")]
pub async fn get_all_projects(
    http_client: &reqwest::Client,
    gitlab_baseurl: &str,
    gitlab_token: &str,
) -> Result<Vec<Project>, Box<dyn Error + Send + Sync>> {
    let mut result: Vec<Project> = Vec::new();
    let mut next_url: Option<String> = Some(format!(
        "https://{gitlab_baseurl}/api/v4/projects?per_page=100&archived=false"
    ));

    while let Some(url) = next_url {
        let resp = http_client
            .get(url)
            .header("PRIVATE-TOKEN", gitlab_token)
            .send()
            .await?
            .error_for_status()?;

        next_url = resp
            .headers()
            .get("link")
            .and_then(|header_value| header_value.to_str().ok())
            .and_then(|header_value_str| parse_link_header::parse_with_rel(header_value_str).ok())
            .and_then(|links| links.get("next").map(|link| link.raw_uri.clone()));

        let mut projects: Vec<Project> = resp.json().await?;
        result.append(&mut projects);
    }

    Ok(result)
}

#[instrument(err, skip_all, target = "gitlab")]
pub async fn get_project_access_tokens(
    req_client: &reqwest::Client,
    gitlab_baseurl: &str,
    gitlab_token: &str,
    project: &Project,
) -> Result<Vec<AccessToken>, Box<dyn Error + Send + Sync>> {
    let mut result: Vec<AccessToken> = Vec::new();
    let mut next_url: Option<String> = Some(format!(
        "https://{gitlab_baseurl}/api/v4/projects/{}/access_tokens?per_page=100",
        project.id
    ));

    while let Some(url) = next_url {
        let resp = req_client
            .get(url)
            .header("PRIVATE-TOKEN", gitlab_token)
            .send()
            .await?
            .error_for_status()?;

        next_url = resp
            .headers()
            .get("link")
            .and_then(|header_value| header_value.to_str().ok())
            .and_then(|header_value_str| parse_link_header::parse_with_rel(header_value_str).ok())
            .and_then(|links| links.get("next").map(|link| link.raw_uri.clone()));

        let mut access_tokens: Vec<AccessToken> = resp.json().await?;
        result.append(&mut access_tokens);
    }

    Ok(result)
}

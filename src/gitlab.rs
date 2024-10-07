use core::error::Error;
use serde::Deserialize;
use serde_repr::Deserialize_repr;

#[derive(Debug, Deserialize)]
pub struct Project {
    id: usize,
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

#[derive(Debug, Deserialize)]
pub struct AccessToken {
    pub scopes: Vec<String>,
    pub name: String,
    pub expires_at: chrono::NaiveDate,
    pub active: bool,
    pub revoked: bool,
    pub access_level: AccessLevel,
}

pub async fn get_all_projects(
    http_client: &reqwest::Client,
    gitlab_baseurl: &String,
    gitlab_token: &String,
) -> Result<Vec<Project>, Box<dyn Error>> {
    let mut result: Vec<Project> = Vec::new();
    let mut next_url: Option<String> = Some(format!(
        "https://{gitlab_baseurl}/api/v4/projects?per_page=100"
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

pub async fn get_project_access_tokens(
    req_client: &reqwest::Client,
    gitlab_baseurl: &String,
    gitlab_token: &String,
    project: &Project,
) -> Result<Vec<AccessToken>, Box<dyn Error>> {
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

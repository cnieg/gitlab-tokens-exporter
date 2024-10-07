use crate::{AccessToken, Project};
use core::error::Error;

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

//! Generates the prometheus metrics

use core::fmt::Error;
use core::fmt::Write as _; // To be able to use the `Write` trait

use crate::gitlab::{AccessToken, Project};

/// Generates prometheus metrics in the expected format
/// The metric name always starts with `gitlab_token_`
#[expect(clippy::arithmetic_side_effects, reason = "Not handled by chrono")]
pub fn build(project: &Project, access_token: &AccessToken) -> Result<String, Error> {
    let mut res = String::new();
    let date_now = chrono::Utc::now().date_naive();

    let metric_name = format!(
        "gitlab_token_{}_{}",
        project.path_with_namespace, access_token.name
    )
    .replace(['-', '/', ' '], "_"); // TODO : see https://prometheus.io/docs/concepts/data_model/ for authorized characters

    writeln!(res, "# HELP {metric_name} Gitlab token")?;
    writeln!(res, "# TYPE {metric_name} gauge")?;
    let access_level = format!("{:?}", access_token.access_level).replace('"', "");
    let scopes = format!("{:?}", access_token.scopes).replace('"', "");
    writeln!(
        res,
        "{metric_name}\
         {{project=\"{}\",\
         token_name=\"{}\",\
         active=\"{}\",\
         revoked=\"{}\",\
         access_level=\"{access_level}\",\
         scopes=\"{scopes}\",\
         expires_at=\"{}\"}} {}\
        ",
        project.path_with_namespace,
        access_token.name,
        access_token.active,
        access_token.revoked,
        access_token.expires_at,
        (access_token.expires_at - date_now).num_days()
    )?;

    Ok(res)
}

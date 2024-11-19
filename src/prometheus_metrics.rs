//! Generates the prometheus metrics

use core::error::Error;
use core::fmt::Write as _; // To be able to use the `Write` trait
use tracing::instrument;

use crate::gitlab::AccessToken;

/// Generates prometheus metrics in the expected format.
/// The metric names always start with `gitlab_token_`
#[expect(clippy::arithmetic_side_effects, reason = "Not handled by chrono")]
#[instrument(err)]
pub fn build(
    token_path: &str,
    token_type: &str,
    access_token: &AccessToken,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let mut res = String::new();
    let date_now = chrono::Utc::now().date_naive();

    // We have to generate a metric name with authorized characters only
    let metric_name: String = format!("gitlab_token_{token_path}_{}", access_token.name)
        .chars()
        .map(|char| match char {
            // see https://prometheus.io/docs/concepts/data_model/ for authorized characters
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | ':' => char,
            _ => '_', // default character if not authorized
        })
        .collect();

    // Use the debug format because we cannot implement the Display trait on Vec<String>
    // We also use replace() because prometheus values cannot contain a double-quote character
    let scopes = format!("{:?}", access_token.scopes).replace('"', "");

    writeln!(res, "# HELP {metric_name} Gitlab token")?;
    writeln!(res, "# TYPE {metric_name} gauge")?;

    writeln!(
        res,
        "{metric_name}\
         {{{token_type}=\"{token_path}\",\
         token_name=\"{}\",\
         active=\"{}\",\
         revoked=\"{}\",\
         access_level=\"{}\",\
         scopes=\"{scopes}\",\
         expires_at=\"{}\"}} {}\
        ",
        access_token.name,
        access_token.active,
        access_token.revoked,
        access_token.access_level,
        access_token.expires_at,
        (access_token.expires_at - date_now).num_days()
    )?;

    Ok(res)
}

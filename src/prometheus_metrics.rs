//! Generates the prometheus metrics

use core::error::Error;
use core::fmt::Write as _; // To be able to use the `Write` trait
use tracing::instrument;

use crate::gitlab::TokenType;

/// Generates prometheus metrics in the expected format.
/// The metric names always start with `gitlab_token_`
#[expect(clippy::arithmetic_side_effects, reason = "Not handled by chrono")]
#[instrument(err)]
pub fn build(token_path: &str, token: TokenType) -> Result<String, Box<dyn Error + Send + Sync>> {
    let mut res = String::new();
    let date_now = chrono::Utc::now().date_naive();

    let token_type = format!("{token}");
    let (name, scopes, active, revoked, expires_at, access_level) = match token {
        TokenType::Project(value) | TokenType::Group(value) => (
            value.name,
            value.scopes,
            value.active,
            value.revoked,
            value.expires_at,
            Some(value.access_level),
        ),
        TokenType::User(value) => (
            value.name,
            value.scopes,
            value.active,
            value.revoked,
            value.expires_at,
            None,
        ),
    };

    // We have to generate a metric name with authorized characters only
    let metric_name: String = format!("gitlab_token_{token_path}_{name}")
        .chars()
        .map(|char| match char {
            // see https://prometheus.io/docs/concepts/data_model/ for authorized characters
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | ':' => char,
            _ => '_', // default character if not authorized
        })
        .collect();

    // Use the debug format because we cannot implement the Display trait on Vec<String>
    // We also use replace() because prometheus values cannot contain a double-quote character
    let scopes_str = format!("{scopes:?}").replace('"', "");

    writeln!(res, "# HELP {metric_name} Gitlab token")?;
    writeln!(res, "# TYPE {metric_name} gauge")?;

    write!(
        res,
        "{metric_name}\
         {{{token_type}=\"{token_path}\",\
         token_name=\"{name}\",\
         active=\"{active}\",\
         revoked=\"{revoked}\","
    )?;

    if let Some(val) = access_level {
        write!(res, "access_level=\"{val}\",")?;
    }

    writeln!(
        res,
        "scopes=\"{scopes_str}\",\
         expires_at=\"{expires_at}\"}} {}\
        ",
        (expires_at - date_now).num_days()
    )?;

    Ok(res)
}

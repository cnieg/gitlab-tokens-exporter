//! Generates the prometheus metrics

use core::error::Error;
use core::fmt::Write as _; // To be able to use the `Write` trait
use tracing::instrument;

use crate::gitlab::Token;

/// Generates prometheus metrics in the expected format.
/// The metric names always start with `gitlab_token_`
#[expect(clippy::arithmetic_side_effects, reason = "Not handled by chrono")]
#[instrument(err)]
pub fn build(token: Token) -> Result<String, Box<dyn Error + Send + Sync>> {
    let mut res = String::new();
    let date_now = chrono::Utc::now().date_naive();

    let token_type = match token {
        Token::Project(_, _) => "project",
        Token::Group(_, _) => "group",
        Token::User(_, _) => "user",
    };

    let (name, scopes, active, revoked, expires_at, access_level, path) = match token {
        Token::Project(access_token, path) | Token::Group(access_token, path) => (
            access_token.name,
            access_token.scopes,
            access_token.active,
            access_token.revoked,
            access_token.expires_at,
            Some(access_token.access_level),
            path,
        ),
        Token::User(pat, path) => (
            pat.name,
            pat.scopes,
            pat.active,
            pat.revoked,
            pat.expires_at,
            None,
            path,
        ),
    };

    // We have to generate a metric name with authorized characters only
    let metric_name: String = format!("gitlab_token_{path}_{name}")
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
         {{{token_type}=\"{path}\",\
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

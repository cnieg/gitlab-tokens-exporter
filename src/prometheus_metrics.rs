//! Generates the prometheus metrics

use core::error::Error;
use core::fmt::Write as _; // To be able to use the `Write` trait
use tracing::{info, instrument};

use crate::gitlab::Token;

/// Generates prometheus metrics in the expected format.
/// The metric names always start with `gitlab_token_`
#[expect(clippy::arithmetic_side_effects, reason = "Not handled by chrono")]
#[instrument(err, skip_all)]
pub fn build(gitlab_token: Token) -> Result<String, Box<dyn Error + Send + Sync>> {
    let mut res = String::new();
    let date_now = chrono::Utc::now().date_naive();

    let token_type = match gitlab_token {
        Token::Group { .. } => "group",
        Token::Project { .. } => "project",
        Token::User { .. } => "user",
    };

    let token_scopes = gitlab_token.scopes()?;

    let (name, active, revoked, expires_at, access_level, path, web_url) = match gitlab_token {
        Token::Group {
            token,
            full_path,
            web_url,
        }
        | Token::Project {
            token,
            full_path,
            web_url,
        } => (
            token.name,
            token.active,
            token.revoked,
            token.expires_at,
            Some(token.access_level),
            full_path,
            Some(web_url),
        ),
        Token::User { token, full_path } => (
            token.name,
            token.active,
            token.revoked,
            token.expires_at,
            None,
            full_path,
            None,
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

    writeln!(res, "# HELP {metric_name} Gitlab token")?;
    writeln!(res, "# TYPE {metric_name} gauge")?;

    let mut metric_str = String::new();
    write!(
        metric_str,
        "{metric_name}\
         {{{token_type}=\"{path}\",\
         token_name=\"{name}\",\
         active=\"{active}\",\
         revoked=\"{revoked}\","
    )?;

    if let Some(val) = access_level {
        write!(metric_str, "access_level=\"{val}\",")?;
    }

    if let Some(val) = web_url {
        write!(metric_str, "web_url=\"{val}\",")?;
    }

    writeln!(
        metric_str,
        "scopes=\"{token_scopes}\",\
         expires_at=\"{expires_at}\"}} {}\
        ",
        (expires_at - date_now).num_days()
    )?;

    info!("{}", metric_str.replace('"', "'").replace('\n', ""));
    res.push_str(&metric_str);
    Ok(res)
}

//-------------------------------------------
//
// Unit tests
//
// ------------------------------------------

#[cfg(test)]
mod tests {

    use chrono::NaiveDate;
    use regex::Regex;

    use crate::gitlab::{
        AccessLevel, AccessToken, AccessTokenScope, PersonalAccessToken, PersonalAccessTokenScope,
        Token,
    };

    //
    // Utility functions
    //
    fn get_first_non_comment_line(text: &str) -> &str {
        text.lines().find(|line| !line.starts_with('#')).unwrap()
    }

    fn default_project_token() -> Token {
        Token::Project {
            token: AccessToken {
                access_level: AccessLevel::Guest,
                active: true,
                expires_at: NaiveDate::parse_from_str("2199-06-29", "%Y-%m-%d").unwrap(),
                name: "name".to_string(),
                revoked: false,
                scopes: vec![AccessTokenScope::Api],
            },
            full_path: "whatever".to_string(),
            web_url: "whatever".to_string(),
        }
    }

    fn default_group_token() -> Token {
        Token::Group {
            token: AccessToken {
                access_level: AccessLevel::Guest,
                active: true,
                expires_at: NaiveDate::parse_from_str("2199-06-29", "%Y-%m-%d").unwrap(),
                name: "name".to_string(),
                revoked: false,
                scopes: vec![AccessTokenScope::Api],
            },
            full_path: "whatever".to_string(),
            web_url: "whatever".to_string(),
        }
    }

    fn default_user_token() -> Token {
        Token::User {
            token: PersonalAccessToken {
                active: true,
                expires_at: NaiveDate::parse_from_str("2199-06-29", "%Y-%m-%d").unwrap(),
                name: "name".to_string(),
                revoked: false,
                scopes: vec![PersonalAccessTokenScope::Api],
                user_id: 123,
            },
            full_path: "whatever".to_string(),
        }
    }

    #[test]
    /// Ensures that the group token metric name begins with "gitlab_token_"
    fn group_token_metric_name_start() {
        let token = default_group_token();
        let text = crate::prometheus_metrics::build(token).unwrap();
        let metric = get_first_non_comment_line(&text);
        assert!(metric.starts_with("gitlab_token_"));
    }

    #[test]
    /// Ensures that the project token metric name begins with "gitlab_token_"
    fn project_token_metric_name_start() {
        let token = default_project_token();
        let text = crate::prometheus_metrics::build(token).unwrap();
        let metric = get_first_non_comment_line(&text);
        assert!(metric.starts_with("gitlab_token_"));
    }

    #[test]
    /// Ensures that the user token metric name begins with "gitlab_token_"
    fn user_token_metric_name_start() {
        let token = default_user_token();
        let text = crate::prometheus_metrics::build(token).unwrap();
        let metric = get_first_non_comment_line(&text);
        assert!(metric.starts_with("gitlab_token_"));
    }
}

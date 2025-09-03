//! Generates the prometheus metrics

use core::fmt::Write as _; // To be able to use the `Write` trait
use tracing::{info, instrument};

use crate::error::BoxedError;
use crate::gitlab::token::Token;

/// Default value when a token has no expiration date
const DEFAULT_TOKEN_VALIDITY_DAYS: u16 = 9999;

/// Generates prometheus metrics in the expected format.
/// The metric names always start with `gitlab_token_`
#[expect(clippy::arithmetic_side_effects, reason = "Not handled by chrono")]
#[instrument(err, skip_all)]
pub fn build(gitlab_token: &Token) -> Result<String, BoxedError> {
    let mut res = String::new();
    let date_now = chrono::Utc::now().date_naive();

    let token_type = match *gitlab_token {
        Token::Group { .. } => "group",
        Token::Project { .. } => "project",
        Token::User { .. } => "user",
    };

    let token_scopes = gitlab_token.scopes()?;

    let (name, active, revoked, expires_at, access_level, full_path, web_url) = match *gitlab_token
    {
        Token::Group {
            ref token,
            ref full_path,
            ref web_url,
        }
        | Token::Project {
            ref token,
            ref full_path,
            ref web_url,
        } => (
            &token.name,
            token.active,
            token.revoked,
            token.expires_at,
            Some(&token.access_level),
            full_path,
            Some(web_url),
        ),
        Token::User {
            ref token,
            ref full_path,
        } => (
            &token.name,
            token.active,
            token.revoked,
            token.expires_at,
            None,
            full_path,
            None,
        ),
    };

    // We have to generate a metric name with authorized characters only
    let metric_name: String = format!("gitlab_token_{full_path}_{name}")
        .chars()
        .map(|char| match char {
            // see https://prometheus.io/docs/concepts/data_model/ for authorized characters
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | ':' => char,
            _ => '_', // default character if not authorized
        })
        .collect();

    writeln!(res, "# HELP {metric_name} Days before Gitlab token expires")?;
    writeln!(res, "# TYPE {metric_name} gauge")?;

    let mut metric_str = String::new();
    write!(
        metric_str,
        "{metric_name}\
         {{{token_type}=\"{full_path}\",\
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

    write!(metric_str, "scopes=\"{token_scopes}\"")?;

    if let Some(expiration_date) = expires_at {
        write!(
            metric_str,
            ",expires_at=\"{expiration_date}\"}} {}",
            (expiration_date - date_now).num_days()
        )?;
    } else {
        write!(metric_str, "}} {DEFAULT_TOKEN_VALIDITY_DAYS}")?;
    }

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

    use chrono::{Days, NaiveDate};
    use once_cell::sync::Lazy;
    use regex::Regex;

    use crate::{
        gitlab::token::{
            AccessLevel, AccessToken, AccessTokenScope, PersonalAccessToken,
            PersonalAccessTokenScope, Token,
        },
        prometheus_metrics::DEFAULT_TOKEN_VALIDITY_DAYS,
    };

    static RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(
            r#"^(?x) # use the x flag to enable insigificant whitespace mode
gitlab_token_(?<fullname>\w+)
\{
(?<origin_type>project|group|user)="(?<origin_name>[^"]+)",
token_name="(?<name>[^"]+)",
active="(?<active>true|false)",
revoked="(?<revoked>true|false)",
(access_level="(?<access_level>(guest|reporter|developer|maintainer|owner))",)?     # Not defined for PersonalAccessToken
(web_url="(?<web_url>[^"]+)",)?                                                     # Not defined for PersonalAccessToken
(scopes="(?<scopes>\[[^\]]+\])")                                                    # Must always be defined and not empty
(,expires_at="(?<expires_at>\+?[0-9]{4,6}-[0-9]{2}-[0-9]{2})")?                     # Not defined if the token has no expiry date
\}
\s(?<days>-?[0-9]+)$
"#,
        )
        .unwrap()
    });

    /*
     * Utility functions
     */
    fn get_first_non_comment_line(text: &str) -> &str {
        text.lines().find(|line| !line.starts_with('#')).unwrap()
    }

    /*
     * Macros
     */
    macro_rules! default_access_token {
        ($token_type:path) => {{
            $token_type {
                token: AccessToken {
                    access_level: AccessLevel::Guest,
                    active: true,
                    expires_at: Some(NaiveDate::parse_from_str("2119-05-14", "%Y-%m-%d").unwrap()),
                    name: "project_token".to_string(),
                    revoked: false,
                    scopes: vec![AccessTokenScope::Api],
                },
                full_path: "project_path".to_string(),
                web_url: "http://project_web_url/".to_string(),
            }
        }};
    }

    macro_rules! default_user_token {
        ($token_type:path) => {{
            $token_type {
                token: PersonalAccessToken {
                    active: true,
                    expires_at: Some(NaiveDate::parse_from_str("2139-01-01", "%Y-%m-%d").unwrap()),
                    name: "user_token".to_string(),
                    revoked: false,
                    scopes: vec![PersonalAccessTokenScope::ReadRepository],
                    user_id: 123,
                },
                full_path: "user_path".to_string(),
            }
        }};
    }

    macro_rules! default_token {
        (Token::Project) => {
            default_access_token!(Token::Project)
        };
        (Token::Group) => {
            default_access_token!(Token::Group)
        };
        (Token::User) => {
            default_user_token!(Token::User)
        };
    }

    macro_rules! get_captures {
        ($text:expr) => {{
            let metric = get_first_non_comment_line($text);
            dbg!(&metric); // Will only be printed if the test fails

            let captures = RE.captures(&metric);
            assert!(captures.is_some(), "metric doesn't match RE!");

            let captures = captures.unwrap();
            dbg!(&captures); // Will only be printed if the test fails
            captures
        }};
    }

    macro_rules! destructure_access_token {
        ($token_name:expr, $token_type:path) => {{
            match $token_name {
                $token_type {
                    token,
                    full_path,
                    web_url,
                } => (token, full_path, web_url),
                _ => panic!(),
            }
        }};
    }

    macro_rules! destructure_user_token {
        ($token_name:expr, $token_type:path) => {{
            match $token_name {
                $token_type { token, full_path } => (token, full_path),
                _ => panic!(),
            }
        }};
    }

    macro_rules! destructure_token {
        ($token_name:expr, Token::Project) => {{ destructure_access_token!($token_name, Token::Project) }};
        ($token_name:expr, Token::Group) => {{ destructure_access_token!($token_name, Token::Group) }};
        ($token_name:expr, Token::User) => {{ destructure_user_token!($token_name, Token::User) }};
    }

    /*
     * Tests
     */
    #[test]
    fn project_token_metric_match_re() {
        let token = default_token!(Token::Project);
        let metric = crate::prometheus_metrics::build(&token).unwrap();
        let captures = get_captures!(&metric);

        let (project_token, full_path, web_url) = destructure_token!(&token, Token::Project);

        assert_eq!(
            &captures["fullname"],
            format!("{full_path}_{}", project_token.name)
        );
        assert_eq!(&captures["origin_type"], "project");

        assert_eq!(&captures["origin_name"], full_path);
        assert_eq!(&captures["name"], project_token.name);
        assert_eq!(&captures["active"], project_token.active.to_string());
        assert_eq!(&captures["revoked"], project_token.revoked.to_string());
        assert_eq!(
            &captures["access_level"],
            format!("{}", project_token.access_level)
        );
        assert_eq!(&captures["web_url"], web_url);
        assert_eq!(&captures["scopes"], token.scopes().unwrap());
        assert_eq!(
            &captures["expires_at"],
            project_token
                .expires_at
                .unwrap()
                .format("%Y-%m-%d")
                .to_string()
        );
    }

    #[test]
    fn group_token_metric_match_re() {
        let token = default_token!(Token::Group);
        let metric = crate::prometheus_metrics::build(&token).unwrap();
        let captures = get_captures!(&metric);

        let (group_token, full_path, web_url) = destructure_token!(&token, Token::Group);

        assert_eq!(
            &captures["fullname"],
            format!("{full_path}_{}", group_token.name)
        );
        assert_eq!(&captures["origin_type"], "group");

        assert_eq!(&captures["origin_name"], full_path);
        assert_eq!(&captures["name"], group_token.name);
        assert_eq!(&captures["active"], group_token.active.to_string());
        assert_eq!(&captures["revoked"], group_token.revoked.to_string());
        assert_eq!(
            &captures["access_level"],
            format!("{}", group_token.access_level)
        );
        assert_eq!(&captures["web_url"], web_url);
        assert_eq!(&captures["scopes"], token.scopes().unwrap());
        assert_eq!(
            &captures["expires_at"],
            group_token
                .expires_at
                .unwrap()
                .format("%Y-%m-%d")
                .to_string()
        );
    }

    #[test]
    fn user_token_metric_match_re() {
        let token = default_token!(Token::User);
        let metric = crate::prometheus_metrics::build(&token).unwrap();
        let captures = get_captures!(&metric);

        let (user_token, full_path) = destructure_token!(&token, Token::User);

        assert_eq!(
            &captures["fullname"],
            format!("{full_path}_{}", user_token.name)
        );
        assert_eq!(&captures["origin_type"], "user");

        assert_eq!(&captures["origin_name"], full_path);
        assert_eq!(&captures["name"], user_token.name);
        assert_eq!(&captures["active"], user_token.active.to_string());
        assert_eq!(&captures["revoked"], user_token.revoked.to_string());
        assert_eq!(&captures["scopes"], token.scopes().unwrap());
        assert_eq!(
            &captures["expires_at"],
            user_token
                .expires_at
                .unwrap()
                .format("%Y-%m-%d")
                .to_string()
        );
    }

    #[test]
    /// Check if the generated metric name contains authorized characters only
    fn project_token_metric_special_chars() {
        let token = default_token!(Token::Project);
        let (mut project_token, _, web_url) = destructure_token!(token, Token::Project);

        // Customize the default token
        project_token.name = "project token name with lot's-of_special-characters!?.|#".to_owned();

        // Redefine {token} with our customized values
        let token = Token::Project {
            token: project_token,
            full_path: "path/with-special,characters=+".to_owned(),
            web_url,
        };

        let metric = crate::prometheus_metrics::build(&token).unwrap();
        let captures = get_captures!(&metric);

        // Special characters must be replaced with underscores
        assert_eq!(
            &captures["fullname"],
            "path_with_special_characters___project_token_name_with_lot_s_of_special_characters_____"
        );
    }

    #[test]
    /// Check if the generated metric name contains authorized characters only
    fn group_token_metric_special_chars() {
        let token = default_token!(Token::Group);
        let (mut group_token, _, web_url) = destructure_token!(token, Token::Group);

        // Customize the default token
        group_token.name = "group token name with special-characters|#".to_owned();

        // Redefine {token} with our customized values
        let token = Token::Group {
            token: group_token,
            full_path: "path/with/slashes-and-dashes".to_owned(),
            web_url,
        };

        let metric = crate::prometheus_metrics::build(&token).unwrap();
        let captures = get_captures!(&metric);

        // Special characters must be replaced with underscores
        assert_eq!(
            &captures["fullname"],
            "path_with_slashes_and_dashes_group_token_name_with_special_characters__"
        );
    }

    #[test]
    /// Check if the generated metric name contains authorized characters only
    fn user_token_metric_special_chars() {
        let token = default_token!(Token::User);
        let (mut user_token, _) = destructure_token!(token, Token::User);

        // Customize the default token
        user_token.name = "user token name with spaces".to_owned();

        // Redefine {token} with our customized values
        let token = Token::User {
            token: user_token,
            full_path: "path/with/slashes".to_owned(),
        };

        let metric = crate::prometheus_metrics::build(&token).unwrap();
        let captures = get_captures!(&metric);

        // Special characters must be replaced with underscores
        assert_eq!(
            &captures["fullname"],
            "path_with_slashes_user_token_name_with_spaces"
        );
    }

    #[test]
    /// Check if the metric's value (the number of days before the token expires) is correct
    fn project_token_valid_days_remaining() {
        const DAYS: u64 = 44;

        let token = default_token!(Token::Project);
        let (mut project_token, full_path, web_url) = destructure_token!(token, Token::Project);

        // Customize the default token
        project_token.expires_at = Some(
            chrono::Local::now()
                .naive_local()
                .date()
                .checked_add_days(Days::new(DAYS))
                .unwrap(),
        );

        // Redefine {token} with our customized values
        let token = Token::Project {
            token: project_token,
            full_path,
            web_url,
        };

        let metric = crate::prometheus_metrics::build(&token).unwrap();
        let captures = get_captures!(&metric);

        assert_eq!(&captures["days"].parse().unwrap(), DAYS)
    }

    #[test]
    /// Check if the metric's value (the number of days after the token expired) is correct
    fn project_token_expired_days() {
        const DAYS: u64 = 44;

        let token = default_token!(Token::Project);
        let (mut project_token, full_path, web_url) = destructure_token!(token, Token::Project);

        // Customize the default token
        project_token.expires_at = Some(
            chrono::Local::now()
                .naive_local()
                .date()
                .checked_sub_days(Days::new(DAYS))
                .unwrap(),
        );

        // Redefine {token} with our customized values
        let token = Token::Project {
            token: project_token,
            full_path,
            web_url,
        };

        let metric = crate::prometheus_metrics::build(&token).unwrap();
        let captures = get_captures!(&metric);

        assert_eq!(&captures["days"].parse().unwrap(), -(DAYS as isize))
    }

    #[test]
    /// Check if token scopes are correct
    fn project_token_scopes() {
        let token = default_token!(Token::Project);
        let (mut project_token, full_path, web_url) = destructure_token!(token, Token::Project);

        // Customize the default token
        project_token.scopes = vec![AccessTokenScope::Api, AccessTokenScope::WriteRepository];

        // Redefine {token} with our customized values
        let token = Token::Project {
            token: project_token,
            full_path,
            web_url,
        };

        let metric = crate::prometheus_metrics::build(&token).unwrap();
        let captures = get_captures!(&metric);

        assert_eq!(&captures["scopes"], "[api,write_repository]");
    }

    #[test]
    /// Check if token scopes are correct
    fn user_token_scopes() {
        let token = default_token!(Token::User);
        let (mut user_token, full_path) = destructure_token!(token, Token::User);

        // Customize the default token
        user_token.scopes = vec![
            PersonalAccessTokenScope::AdminMode,
            PersonalAccessTokenScope::Api,
            PersonalAccessTokenScope::ReadRepository,
        ];

        // Redefine {token} with our customized values
        let token = Token::User {
            token: user_token,
            full_path,
        };

        let metric = crate::prometheus_metrics::build(&token).unwrap();
        let captures = get_captures!(&metric);

        assert_eq!(&captures["scopes"], "[admin_mode,api,read_repository]");
    }

    #[test]
    /// Check if non expiring project token metrics are rendered correctly
    fn project_token_no_expiration() {
        let token = default_token!(Token::Project);
        let (mut project_token, full_path, web_url) = destructure_token!(token, Token::Project);

        // Customize the default token
        project_token.expires_at = None;

        // Redefine {token} with our customized values
        let token = Token::Project {
            token: project_token,
            full_path,
            web_url,
        };

        let metric = crate::prometheus_metrics::build(&token).unwrap();
        let captures = get_captures!(&metric);

        assert_eq!(
            &captures["days"].parse().unwrap(),
            DEFAULT_TOKEN_VALIDITY_DAYS
        );

        assert!(captures.name("expires_at").is_none());
    }

    #[test]
    /// Check if non expiring group token metrics are rendered correctly
    fn group_token_no_expiration() {
        let token = default_token!(Token::Group);
        let (mut group_token, full_path, web_url) = destructure_token!(token, Token::Group);

        // Customize the default token
        group_token.expires_at = None;

        // Redefine {token} with our customized values
        let token = Token::Group {
            token: group_token,
            full_path,
            web_url,
        };

        let metric = crate::prometheus_metrics::build(&token).unwrap();
        let captures = get_captures!(&metric);

        assert_eq!(
            &captures["days"].parse().unwrap(),
            DEFAULT_TOKEN_VALIDITY_DAYS
        );

        assert!(captures.name("expires_at").is_none());
    }

    #[test]
    /// Check if non expiring user token metrics are rendered correctly
    fn user_token_no_expiration() {
        let token = default_token!(Token::User);
        let (mut user_token, full_path) = destructure_token!(token, Token::User);

        // Customize the default token
        user_token.expires_at = None;

        // Redefine {token} with our customized values
        let token = Token::User {
            token: user_token,
            full_path,
        };

        let metric = crate::prometheus_metrics::build(&token).unwrap();
        let captures = get_captures!(&metric);

        assert_eq!(
            &captures["days"].parse().unwrap(),
            DEFAULT_TOKEN_VALIDITY_DAYS
        );

        assert!(captures.name("expires_at").is_none());
    }

    #[test]
    /// Check if a project token with an expiry date with a 5 digits year is rendered correctly
    fn project_token_expiry_year_10000() {
        let token = default_token!(Token::Project);
        let (mut project_token, full_path, web_url) = destructure_token!(token, Token::Project);

        // Customize the default token
        project_token.expires_at = Some(NaiveDate::from_ymd_opt(10000, 12, 31).unwrap());

        // Redefine {token} with our customized values
        let token = Token::Project {
            token: project_token,
            full_path,
            web_url,
        };

        let metric = crate::prometheus_metrics::build(&token).unwrap();
        let captures = get_captures!(&metric);

        assert_eq!(&captures["expires_at"], "+10000-12-31");
    }

    #[test]
    /// Check if a group token with an expiry date with a 5 digits year is rendered correctly
    fn group_token_expiry_year_10000() {
        let token = default_token!(Token::Group);
        let (mut group_token, full_path, web_url) = destructure_token!(token, Token::Group);

        // Customize the default token
        group_token.expires_at = Some(NaiveDate::from_ymd_opt(10000, 12, 31).unwrap());

        // Redefine {token} with our customized values
        let token = Token::Group {
            token: group_token,
            full_path,
            web_url,
        };

        let metric = crate::prometheus_metrics::build(&token).unwrap();
        let captures = get_captures!(&metric);

        assert_eq!(&captures["expires_at"], "+10000-12-31");
    }

    #[test]
    /// Check if a user token with an expiry date with a 5 digits year is rendered correctly
    fn user_token_expiry_year_10000() {
        let token = default_token!(Token::User);
        let (mut user_token, full_path) = destructure_token!(token, Token::User);

        // Customize the default token
        user_token.expires_at = Some(NaiveDate::from_ymd_opt(10000, 12, 31).unwrap());

        // Redefine {token} with our customized values
        let token = Token::User {
            token: user_token,
            full_path,
        };

        let metric = crate::prometheus_metrics::build(&token).unwrap();
        let captures = get_captures!(&metric);

        assert_eq!(&captures["expires_at"], "+10000-12-31");
    }

    #[test]
    /// Check if a project token with an expiry date with a 6 digits year is rendered correctly
    fn project_token_expiry_year_250000() {
        let token = default_token!(Token::Project);
        let (mut project_token, full_path, web_url) = destructure_token!(token, Token::Project);

        // Customize the default token
        project_token.expires_at = Some(NaiveDate::from_ymd_opt(250000, 12, 31).unwrap());

        // Redefine {token} with our customized values
        let token = Token::Project {
            token: project_token,
            full_path,
            web_url,
        };

        let metric = crate::prometheus_metrics::build(&token).unwrap();
        let captures = get_captures!(&metric);

        assert_eq!(&captures["expires_at"], "+250000-12-31");
    }

    #[test]
    /// Check if a group token with an expiry date with a 6 digits year is rendered correctly
    fn group_token_expiry_year_250000() {
        let token = default_token!(Token::Group);
        let (mut group_token, full_path, web_url) = destructure_token!(token, Token::Group);

        // Customize the default token
        group_token.expires_at = Some(NaiveDate::from_ymd_opt(250000, 12, 31).unwrap());

        // Redefine {token} with our customized values
        let token = Token::Group {
            token: group_token,
            full_path,
            web_url,
        };

        let metric = crate::prometheus_metrics::build(&token).unwrap();
        let captures = get_captures!(&metric);

        assert_eq!(&captures["expires_at"], "+250000-12-31");
    }

    #[test]
    /// Check if a user token with an expiry date with a 6 digits year is rendered correctly
    fn user_token_expiry_year_250000() {
        let token = default_token!(Token::User);
        let (mut user_token, full_path) = destructure_token!(token, Token::User);

        // Customize the default token
        user_token.expires_at = Some(NaiveDate::from_ymd_opt(250000, 12, 31).unwrap());

        // Redefine {token} with our customized values
        let token = Token::User {
            token: user_token,
            full_path,
        };

        let metric = crate::prometheus_metrics::build(&token).unwrap();
        let captures = get_captures!(&metric);

        assert_eq!(&captures["expires_at"], "+250000-12-31");
    }
}

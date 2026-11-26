//! Generates the prometheus metrics

use anyhow::Context as _;
use core::fmt::Write as _; // To be able to use the `write` macro
use std::borrow::Cow;
use tracing::{info, instrument};

use crate::gitlab::token::Token;

/// Default value when a token has no expiration date
const DEFAULT_TOKEN_VALIDITY_DAYS: u16 = 9999;

impl TryFrom<&Token> for String {
    type Error = anyhow::Error;

    /// Generates prometheus metrics in the expected format.
    /// The metric name is always `gitlab_token_days_remaining` with labels indicating its name, id, type, ...
    #[instrument(err, skip_all)]
    #[expect(clippy::arithmetic_side_effects, reason = "not handled by chrono")]
    #[inline]
    fn try_from(token: &Token) -> Result<Self, Self::Error> {
        let mut res = Self::new();

        write!(
            res,
            "gitlab_token_days_remaining\
            {{name=\"{}\",\
            id=\"{}\",\
            type=\"{}\",\
            {}=\"{}\",\
            {}",
            escape_label_value(token.name()),
            token.id(),
            token.type_(),
            token.origin_type(),
            escape_label_value(token.origin_value()),
            token
                .labels()
                .context("failed to write tokens labels to `res`")?,
        )
        .context("failed to write token details to metric_str")?;

        if let Some(expiration_date) = token.expiration_date() {
            let date_now = chrono::Utc::now().date_naive();
            writeln!(
                res,
                ",expires_at=\"{expiration_date}\"}} {}",
                (expiration_date - date_now).num_days()
            )
            .context("failed to write expiration date to `res`")?;
        } else {
            writeln!(res, "}} {DEFAULT_TOKEN_VALIDITY_DAYS}")
                .context("failed to write default token validity days to `res`")?;
        }

        info!("{}", res.replace('\n', ""));
        Ok(res)
    }
}

/// Escapes a label value for the [prometheus text exposition format](https://prometheus.io/docs/instrumenting/exposition_formats/#text-format-details)
///
/// The backslash, double-quote and line feed characters have to be written as
/// `\\`, `\"` and `\n`.
///
/// Borrows when there is nothing to escape, which is the common case.
pub fn escape_label_value(value: &str) -> Cow<'_, str> {
    if value.contains(['\\', '"', '\n']) {
        let mut escaped = String::with_capacity(value.len());
        for character in value.chars() {
            match character {
                '\\' => escaped.push_str("\\\\"),
                '"' => escaped.push_str("\\\""),
                '\n' => escaped.push_str("\\n"),
                _ => escaped.push(character),
            }
        }
        Cow::Owned(escaped)
    } else {
        Cow::Borrowed(value)
    }
}

//-------------------------------------------
//
// Unit tests
//
// ------------------------------------------

#[cfg(test)]
mod tests {

    use std::sync::LazyLock;

    use chrono::{Days, NaiveDate};
    use regex::Regex;
    use serde::Deserialize;

    use crate::{
        gitlab::token::{
            HasScopes, Token, TokenOrigin,
            access::{AccessLevel, AccessToken, AccessTokenScope},
            deploy::{DeployToken, DeployTokenScope},
            deserialize_optional_date,
            personal::{PersonalAccessToken, PersonalAccessTokenScope},
            pipeline::{Owner, PipelineTriggerToken},
        },
        prometheus_metrics::{DEFAULT_TOKEN_VALIDITY_DAYS, escape_label_value},
    };

    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
                r#"^(?x) # use the x flag to enable insigificant whitespace mode
    gitlab_token_days_remaining
    \{
    name="(?<name>(?:[^"\\]|\\.)+)",
    id="(?<id>[^"]+)",
    type="(?<type>(project_access|group_access|user_personal_access|project_deploy|group_deploy|project_pipeline_trigger))",
    (?<origin_type>project|group|user)="(?<origin_name>(?:[^"\\]|\\.)+)"
    (,active="(?<active>true|false)")?                                                  # Optionnal label
    (,revoked="(?<revoked>true|false)")?                                                # Optionnal label
    (,scopes="(?<scopes>\[[^\]]+\])")?                                                  # Optionnal label
    (,access_level="(?<access_level>(guest|reporter|developer|maintainer|owner))")?     # Optionnal label
    (,expired="(?<expired>true|false)")?                                                # Optionnal label
    (,username="(?<username>(?:[^"\\]|\\.)+)")?                                         # Optionnal label
    (,web_url="(?<web_url>(?:[^"\\]|\\.)+)")?                                           # Optionnal label
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

    fn default_access_token() -> AccessToken {
        AccessToken {
            access_level: AccessLevel::Guest,
            active: true,
            expires_at: Some(NaiveDate::parse_from_str("2119-05-14", "%Y-%m-%d").unwrap()),
            id: 1234,
            name: "access_token".to_string(),
            revoked: false,
            scopes: vec![AccessTokenScope::Api],
        }
    }

    fn default_personal_access_token() -> PersonalAccessToken {
        PersonalAccessToken {
            active: true,
            expires_at: Some(NaiveDate::parse_from_str("2139-01-01", "%Y-%m-%d").unwrap()),
            id: 1234,
            name: "user_token".to_owned(),
            revoked: false,
            scopes: vec![PersonalAccessTokenScope::ReadRepository],
            user_id: 123,
        }
    }

    fn default_deploy_token() -> DeployToken {
        DeployToken {
            expired: false,
            expires_at: Some(NaiveDate::parse_from_str("2115-06-11", "%Y-%m-%d").unwrap()),
            id: 1234,
            name: "deploy_token".to_string(),
            revoked: false,
            scopes: vec![DeployTokenScope::ReadVirtualRegistry],
            username: "default_username".to_string(),
        }
    }

    fn default_pipeline_trigger_token() -> PipelineTriggerToken {
        PipelineTriggerToken {
            description: "default_pipeline_trigger_token".to_string(),
            expires_at: Some(NaiveDate::parse_from_str("2189-02-07", "%Y-%m-%d").unwrap()),
            id: 5678,
            owner: Owner {
                id: 222,
                username: "default_username".to_string(),
            },
        }
    }

    fn default_project_origin() -> TokenOrigin {
        TokenOrigin::Project {
            project_name: "default_project".to_owned(),
            web_url: "default_project_web_url".to_owned(),
        }
    }

    fn default_group_origin() -> TokenOrigin {
        TokenOrigin::Group {
            group_name: "default_group".to_owned(),
            web_url: "default_group_web_url".to_owned(),
        }
    }

    /*
     * Macros
     */
    macro_rules! default_token {
        (ProjectAccessToken) => {{
            Token::AccessToken {
                token: default_access_token(),
                origin: default_project_origin(),
            }
        }};
        (ProjectDeployToken) => {{
            Token::DeployToken {
                token: default_deploy_token(),
                origin: default_project_origin(),
            }
        }};
        (ProjectPipelineTriggerToken) => {{
            Token::PipelineTriggerToken {
                token: default_pipeline_trigger_token(),
                origin: default_project_origin(),
            }
        }};
        (GroupAccessToken) => {{
            Token::AccessToken {
                token: default_access_token(),
                origin: default_group_origin(),
            }
        }};
        (GroupDeployToken) => {{
            Token::DeployToken {
                token: default_deploy_token(),
                origin: default_group_origin(),
            }
        }};
        (UserPersonalAccessToken) => {{
            Token::PersonalAccessToken {
                token: default_personal_access_token(),
                origin: TokenOrigin::User {
                    user_name: "default_user_name".to_owned(),
                },
            }
        }};
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

    // Destructures a `Token` into its fields : `token` and `origin`
    macro_rules! destructure_token {
        ($token:expr, $kind:ident) => {{
            match $token {
                Token::$kind { token, origin } => (token, origin),
                _ => panic!(),
            }
        }};
    }

    /*
     * Tests
     */
    #[test]
    fn escape_label_value_escapes_backslash_quote_and_line_feed() {
        assert_eq!(escape_label_value("nothing special"), "nothing special");
        assert_eq!(escape_label_value(r#"a"b"#), r#"a\"b"#);
        assert_eq!(escape_label_value(r"a\b"), r"a\\b");
        assert_eq!(escape_label_value("a\nb"), r"a\nb");
    }

    #[test]
    fn user_personal_access_token_name_with_special_characters_is_escaped() {
        let token = default_token!(UserPersonalAccessToken);
        let (mut user_token, origin) = destructure_token!(token, PersonalAccessToken);

        user_token.name = r#"MacBook Pro 14" registry R\W"#.to_string();

        let token = Token::PersonalAccessToken {
            token: user_token,
            origin,
        };

        let metric = String::try_from(&token).unwrap();
        let captures = get_captures!(&metric);

        assert_eq!(&captures["name"], r#"MacBook Pro 14\" registry R\\W"#);
    }

    #[test]
    fn user_personal_access_token_full_path_with_special_characters_is_escaped() {
        let token = default_token!(UserPersonalAccessToken);
        let (user_token, origin) = destructure_token!(token, PersonalAccessToken);

        let origin = match origin {
            TokenOrigin::User { .. } => TokenOrigin::User {
                user_name: r#"user"path\with_backslash"#.to_owned(),
            },
            _ => panic!(),
        };
        let token = Token::PersonalAccessToken {
            token: user_token,
            origin,
        };

        let metric = String::try_from(&token).unwrap();
        let captures = get_captures!(&metric);

        assert_eq!(&captures["origin_name"], r#"user\"path\\with_backslash"#);
    }

    #[test]
    fn token_web_url_with_special_characters_is_escaped() {
        let token = default_token!(ProjectAccessToken);
        let (project_token, origin) = destructure_token!(token, AccessToken);

        let token = Token::AccessToken {
            token: project_token,
            origin: match origin {
                TokenOrigin::Project { project_name, .. } => TokenOrigin::Project {
                    project_name,
                    web_url: r#"http://project"web_url\with_backslash/"#.to_string(),
                },
                _ => panic!(),
            },
        };

        let metric = String::try_from(&token).unwrap();
        let captures = get_captures!(&metric);

        assert_eq!(
            &captures["web_url"],
            r#"http://project\"web_url\\with_backslash/"#
        );
    }

    #[test]
    fn project_access_token_metric_match_re() {
        let token = default_token!(ProjectAccessToken);
        let metric = String::try_from(&token).unwrap();
        let captures = get_captures!(&metric);

        assert!(metric.ends_with('\n'));

        let (token, origin) = destructure_token!(&token, AccessToken);

        let (origin_type, origin_name, origin_web_url) = match origin {
            TokenOrigin::Project {
                project_name,
                web_url,
            } => ("project", project_name, web_url),
            _ => panic!(),
        };

        assert_eq!(&captures["name"], token.name);
        assert_eq!(&captures["id"], token.id.to_string());
        assert_eq!(&captures["type"], "project_access");
        assert_eq!(&captures["origin_type"], origin_type);
        assert_eq!(&captures["origin_name"], origin_name);
        assert_eq!(&captures["active"], token.active.to_string());
        assert_eq!(&captures["revoked"], token.revoked.to_string());
        assert_eq!(&captures["scopes"], token.scopes().unwrap());
        assert_eq!(&captures["access_level"], format!("{}", token.access_level));
        assert!(captures.name("expired").is_none());
        assert!(captures.name("username").is_none());
        assert_eq!(&captures["web_url"], origin_web_url);
        assert_eq!(
            &captures["expires_at"],
            token.expires_at.unwrap().format("%Y-%m-%d").to_string()
        );
    }

    #[test]
    fn group_access_token_metric_match_re() {
        let token = default_token!(GroupAccessToken);
        let metric = String::try_from(&token).unwrap();
        let captures = get_captures!(&metric);

        assert!(metric.ends_with('\n'));

        let (token, origin) = destructure_token!(&token, AccessToken);

        let (origin_type, origin_name, origin_web_url) = match origin {
            TokenOrigin::Group {
                group_name,
                web_url,
            } => ("group", group_name, web_url),
            _ => panic!(),
        };

        assert_eq!(&captures["name"], token.name);
        assert_eq!(&captures["id"], token.id.to_string());
        assert_eq!(&captures["type"], "group_access");
        assert_eq!(&captures["origin_type"], origin_type);
        assert_eq!(&captures["origin_name"], origin_name);
        assert_eq!(&captures["active"], token.active.to_string());
        assert_eq!(&captures["revoked"], token.revoked.to_string());
        assert_eq!(&captures["scopes"], token.scopes().unwrap());
        assert_eq!(&captures["access_level"], format!("{}", token.access_level));
        assert!(captures.name("expired").is_none());
        assert!(captures.name("username").is_none());
        assert_eq!(&captures["web_url"], origin_web_url);
        assert_eq!(
            &captures["expires_at"],
            token.expires_at.unwrap().format("%Y-%m-%d").to_string()
        );
    }

    #[test]
    fn user_personal_access_token_metric_match_re() {
        let token = default_token!(UserPersonalAccessToken);
        let metric = String::try_from(&token).unwrap();
        let captures = get_captures!(&metric);

        assert!(metric.ends_with('\n'));

        let (token, origin) = destructure_token!(&token, PersonalAccessToken);

        let (origin_type, origin_name) = match origin {
            TokenOrigin::User { user_name } => ("user", user_name),
            _ => panic!(),
        };

        assert_eq!(&captures["name"], token.name);
        assert_eq!(&captures["id"], token.id.to_string());
        assert_eq!(&captures["type"], "user_personal_access");
        assert_eq!(&captures["origin_type"], origin_type);
        assert_eq!(&captures["origin_name"], origin_name);
        assert_eq!(&captures["active"], token.active.to_string());
        assert_eq!(&captures["revoked"], token.revoked.to_string());
        assert_eq!(&captures["scopes"], token.scopes().unwrap());
        assert!(captures.name("access_level").is_none());
        assert!(captures.name("expired").is_none());
        assert!(captures.name("username").is_none());
        assert!(captures.name("web_url").is_none());
        assert_eq!(
            &captures["expires_at"],
            token.expires_at.unwrap().format("%Y-%m-%d").to_string()
        );
    }

    #[test]
    /// Check if the metric's value (the number of days before the token expires) is correct
    fn project_access_token_valid_days_remaining() {
        const DAYS: u64 = 44;

        let token = default_token!(ProjectAccessToken);
        let (mut token, origin) = destructure_token!(token, AccessToken);

        // Customize the default token
        token.expires_at = Some(
            chrono::Local::now()
                .naive_local()
                .date()
                .checked_add_days(Days::new(DAYS))
                .unwrap(),
        );

        // Redefine {token} with our customized values
        let token = Token::AccessToken { token, origin };

        let metric = String::try_from(&token).unwrap();
        let captures = get_captures!(&metric);

        assert!(metric.ends_with('\n'));

        assert_eq!(&captures["days"].parse().unwrap(), DAYS)
    }

    #[test]
    /// Check if the metric's value (the number of days after the token expired) is correct
    fn project_access_token_expired_days() {
        const DAYS: u64 = 44;

        let token = default_token!(ProjectAccessToken);
        let (mut token, origin) = destructure_token!(token, AccessToken);

        // Customize the default token
        token.expires_at = Some(
            chrono::Local::now()
                .naive_local()
                .date()
                .checked_sub_days(Days::new(DAYS))
                .unwrap(),
        );

        // Redefine {token} with our customized values
        let token = Token::AccessToken { token, origin };

        let metric = String::try_from(&token).unwrap();
        let captures = get_captures!(&metric);

        assert!(metric.ends_with('\n'));

        assert_eq!(&captures["days"].parse().unwrap(), -(DAYS as isize))
    }

    #[test]
    /// Check if token scopes are correct
    fn project_access_token_scopes() {
        let token = default_token!(ProjectAccessToken);
        let (mut token, origin) = destructure_token!(token, AccessToken);

        // Customize the default token
        token.scopes = vec![AccessTokenScope::Api, AccessTokenScope::WriteRepository];

        // Redefine {token} with our customized values
        let token = Token::AccessToken { token, origin };

        let metric = String::try_from(&token).unwrap();
        let captures = get_captures!(&metric);

        assert!(metric.ends_with('\n'));

        assert_eq!(&captures["scopes"], "[api,write_repository]");
    }

    #[test]
    /// Check if token scopes are correct
    fn user_personal_access_token_scopes() {
        let token = default_token!(UserPersonalAccessToken);
        let (mut token, origin) = destructure_token!(token, PersonalAccessToken);

        // Customize the default token
        token.scopes = vec![
            PersonalAccessTokenScope::AdminMode,
            PersonalAccessTokenScope::Api,
            PersonalAccessTokenScope::ReadRepository,
        ];

        // Redefine {token} with our customized values
        let token = Token::PersonalAccessToken { token, origin };

        let metric = String::try_from(&token).unwrap();
        let captures = get_captures!(&metric);

        assert!(metric.ends_with('\n'));

        assert_eq!(&captures["scopes"], "[admin_mode,api,read_repository]");
    }

    #[test]
    /// Check if non expiring project token metrics are rendered correctly
    fn project_access_token_no_expiration() {
        let token = default_token!(ProjectAccessToken);
        let (mut token, origin) = destructure_token!(token, AccessToken);

        // Customize the default token
        token.expires_at = None;

        // Redefine {token} with our customized values
        let token = Token::AccessToken { token, origin };

        let metric = String::try_from(&token).unwrap();
        let captures = get_captures!(&metric);

        assert!(metric.ends_with('\n'));

        assert_eq!(
            &captures["days"].parse().unwrap(),
            DEFAULT_TOKEN_VALIDITY_DAYS
        );

        assert!(captures.name("expires_at").is_none());
    }

    #[test]
    /// Check if non expiring group token metrics are rendered correctly
    fn group_access_token_no_expiration() {
        let token = default_token!(GroupAccessToken);
        let (mut token, origin) = destructure_token!(token, AccessToken);

        // Customize the default token
        token.expires_at = None;

        // Redefine {token} with our customized values
        let token = Token::AccessToken {
            token: token,
            origin,
        };

        let metric = String::try_from(&token).unwrap();
        let captures = get_captures!(&metric);

        assert!(metric.ends_with('\n'));

        assert_eq!(
            &captures["days"].parse().unwrap(),
            DEFAULT_TOKEN_VALIDITY_DAYS
        );

        assert!(captures.name("expires_at").is_none());
    }

    #[test]
    /// Check if non expiring user token metrics are rendered correctly
    fn user_personal_access_token_no_expiration() {
        let token = default_token!(UserPersonalAccessToken);
        let (mut token, origin) = destructure_token!(token, PersonalAccessToken);

        // Customize the default token
        token.expires_at = None;

        // Redefine {token} with our customized values
        let token = Token::PersonalAccessToken { token, origin };

        let metric = String::try_from(&token).unwrap();
        let captures = get_captures!(&metric);

        assert!(metric.ends_with('\n'));

        assert_eq!(
            &captures["days"].parse().unwrap(),
            DEFAULT_TOKEN_VALIDITY_DAYS
        );

        assert!(captures.name("expires_at").is_none());
    }

    #[test]
    /// Check if a project token with an expiry date with a 5 digits year is rendered correctly
    fn project_access_token_expiry_year_10000() {
        let token = default_token!(ProjectAccessToken);
        let (mut token, origin) = destructure_token!(token, AccessToken);

        // Customize the default token
        token.expires_at = Some(NaiveDate::from_ymd_opt(10000, 12, 31).unwrap());

        // Redefine {token} with our customized values
        let token = Token::AccessToken { token, origin };

        let metric = String::try_from(&token).unwrap();
        let captures = get_captures!(&metric);

        assert!(metric.ends_with('\n'));

        assert_eq!(&captures["expires_at"], "+10000-12-31");
    }

    #[test]
    /// Check if a group token with an expiry date with a 5 digits year is rendered correctly
    fn group_access_token_expiry_year_10000() {
        let token = default_token!(GroupAccessToken);
        let (mut token, origin) = destructure_token!(token, AccessToken);

        // Customize the default token
        token.expires_at = Some(NaiveDate::from_ymd_opt(10000, 12, 31).unwrap());

        // Redefine {token} with our customized values
        let token = Token::AccessToken { token, origin };

        let metric = String::try_from(&token).unwrap();
        let captures = get_captures!(&metric);

        assert!(metric.ends_with('\n'));

        assert_eq!(&captures["expires_at"], "+10000-12-31");
    }

    #[test]
    /// Check if a user token with an expiry date with a 5 digits year is rendered correctly
    fn user_personal_access_token_expiry_year_10000() {
        let token = default_token!(UserPersonalAccessToken);
        let (mut token, origin) = destructure_token!(token, PersonalAccessToken);

        // Customize the default token
        token.expires_at = Some(NaiveDate::from_ymd_opt(10000, 12, 31).unwrap());

        // Redefine {token} with our customized values
        let token = Token::PersonalAccessToken { token, origin };

        let metric = String::try_from(&token).unwrap();
        let captures = get_captures!(&metric);

        assert!(metric.ends_with('\n'));

        assert_eq!(&captures["expires_at"], "+10000-12-31");
    }

    #[test]
    /// Check if a project token with an expiry date with a 6 digits year is rendered correctly
    fn project_access_token_expiry_year_250000() {
        let token = default_token!(ProjectAccessToken);
        let (mut token, origin) = destructure_token!(token, AccessToken);

        // Customize the default token
        token.expires_at = Some(NaiveDate::from_ymd_opt(250000, 12, 31).unwrap());

        // Redefine {token} with our customized values
        let token = Token::AccessToken { token, origin };

        let metric = String::try_from(&token).unwrap();
        let captures = get_captures!(&metric);

        assert!(metric.ends_with('\n'));

        assert_eq!(&captures["expires_at"], "+250000-12-31");
    }

    #[test]
    /// Check if a group token with an expiry date with a 6 digits year is rendered correctly
    fn group_access_token_expiry_year_250000() {
        let token = default_token!(GroupAccessToken);
        let (mut token, origin) = destructure_token!(token, AccessToken);

        // Customize the default token
        token.expires_at = Some(NaiveDate::from_ymd_opt(250000, 12, 31).unwrap());

        // Redefine {token} with our customized values
        let token = Token::AccessToken { token, origin };

        let metric = String::try_from(&token).unwrap();
        let captures = get_captures!(&metric);

        assert!(metric.ends_with('\n'));

        assert_eq!(&captures["expires_at"], "+250000-12-31");
    }

    #[test]
    /// Check if a user token with an expiry date with a 6 digits year is rendered correctly
    fn user_personal_access_token_expiry_year_250000() {
        let token = default_token!(UserPersonalAccessToken);
        let (mut token, origin) = destructure_token!(token, PersonalAccessToken);

        // Customize the default token
        token.expires_at = Some(NaiveDate::from_ymd_opt(250000, 12, 31).unwrap());

        // Redefine {token} with our customized values
        let token = Token::PersonalAccessToken { token, origin };

        let metric = String::try_from(&token).unwrap();
        let captures = get_captures!(&metric);

        assert!(metric.ends_with('\n'));

        assert_eq!(&captures["expires_at"], "+250000-12-31");
    }

    #[derive(Debug, Deserialize)]
    struct TestDate {
        #[serde(deserialize_with = "deserialize_optional_date")]
        date: Option<NaiveDate>,
    }

    #[test]
    fn test_deserialize_valid_date() {
        let result: TestDate = serde_json::from_str(r#"{"date": "2025-06-28"}"#).unwrap();

        assert_eq!(
            result.date,
            Some(NaiveDate::from_ymd_opt(2025, 6, 28).unwrap())
        );
    }

    #[test]
    fn test_deserialize_null_date() {
        let result: TestDate = serde_json::from_str(r#"{"date": null}"#).unwrap();

        assert_eq!(result.date, None);
    }

    #[test]
    fn test_deserialize_deploy_token_valid_date() {
        let result: TestDate =
            serde_json::from_str(r#"{"date": "2020-02-14T00:00:00.000Z"}"#).unwrap();

        assert_eq!(
            result.date,
            Some(NaiveDate::from_ymd_opt(2020, 2, 14).unwrap())
        );
    }

    #[test]
    fn project_deploy_token_metric_match_re() {
        let token = default_token!(ProjectDeployToken);
        let metric = String::try_from(&token).unwrap();
        let captures = get_captures!(&metric);

        assert!(metric.ends_with('\n'));

        let (token, origin) = destructure_token!(&token, DeployToken);

        let (origin_type, origin_name, web_url) = match origin {
            TokenOrigin::Project {
                project_name,
                web_url,
            } => ("project", project_name, web_url),
            _ => panic!(),
        };

        assert_eq!(&captures["name"], token.name);
        assert_eq!(&captures["id"], token.id.to_string());
        assert_eq!(&captures["type"], "project_deploy");
        assert_eq!(&captures["origin_type"], origin_type);
        assert_eq!(&captures["origin_name"], origin_name);
        assert!(&captures.name("active").is_none());
        assert_eq!(&captures["revoked"], token.revoked.to_string());
        assert_eq!(&captures["scopes"], token.scopes().unwrap());
        assert!(&captures.name("access_level").is_none());
        assert_eq!(&captures["expired"], token.expired.to_string());
        assert_eq!(&captures["username"], token.username);
        assert_eq!(&captures["web_url"], web_url);
        assert_eq!(
            &captures["expires_at"],
            token.expires_at.unwrap().format("%Y-%m-%d").to_string()
        );
    }

    #[test]
    fn group_deploy_token_metric_match_re() {
        let token = default_token!(GroupDeployToken);
        let metric = String::try_from(&token).unwrap();
        let captures = get_captures!(&metric);

        assert!(metric.ends_with('\n'));

        let (token, origin) = destructure_token!(&token, DeployToken);

        let (origin_type, origin_name, web_url) = match origin {
            TokenOrigin::Group {
                group_name,
                web_url,
            } => ("group", group_name, web_url),
            _ => panic!(),
        };

        assert_eq!(&captures["name"], token.name);
        assert_eq!(&captures["id"], token.id.to_string());
        assert_eq!(&captures["type"], "group_deploy");
        assert_eq!(&captures["origin_type"], origin_type);
        assert_eq!(&captures["origin_name"], origin_name);
        assert!(&captures.name("active").is_none());
        assert_eq!(&captures["revoked"], token.revoked.to_string());
        assert_eq!(&captures["scopes"], token.scopes().unwrap());
        assert!(&captures.name("access_level").is_none());
        assert_eq!(&captures["expired"], token.expired.to_string());
        assert_eq!(&captures["username"], token.username);
        assert_eq!(&captures["web_url"], web_url);
        assert_eq!(
            &captures["expires_at"],
            token.expires_at.unwrap().format("%Y-%m-%d").to_string()
        );
    }

    #[test]
    fn project_pipeline_trigger_token_metric_match_re() {
        let token = default_token!(ProjectPipelineTriggerToken);
        let metric = String::try_from(&token).unwrap();
        let captures = get_captures!(&metric);

        assert!(metric.ends_with('\n'));

        let (token, origin) = destructure_token!(&token, PipelineTriggerToken);

        let (origin_type, origin_name, web_url) = match origin {
            TokenOrigin::Project {
                project_name,
                web_url,
            } => ("project", project_name, web_url),
            _ => panic!(),
        };

        assert_eq!(&captures["name"], token.description);
        assert_eq!(&captures["id"], token.id.to_string());
        assert_eq!(&captures["type"], "project_pipeline_trigger");
        assert_eq!(&captures["origin_type"], origin_type);
        assert_eq!(&captures["origin_name"], origin_name);
        assert!(&captures.name("active").is_none());
        assert!(&captures.name("revoked").is_none());
        assert!(&captures.name("scopes").is_none());
        assert!(&captures.name("access_level").is_none());
        assert!(&captures.name("expired").is_none());
        assert_eq!(&captures["username"], token.owner.username);
        assert_eq!(&captures["web_url"], web_url);

        assert_eq!(
            &captures["expires_at"],
            token.expires_at.unwrap().format("%Y-%m-%d").to_string()
        );
    }
}

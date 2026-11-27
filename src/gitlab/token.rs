//! Defines a generic [`Token`]

pub mod access;
pub mod personal;

use anyhow::Context as _;
use chrono::NaiveDate;
use core::fmt::Write as _; // To be able to use the `write` macro
use serde::Deserialize as _;

use crate::{
    gitlab::token::{access::AccessToken, personal::PersonalAccessToken},
    prometheus_metrics::escape_label_value,
};

/// Defined for tokens with scopes
pub trait HasScopes {
    /// Returns the list of scopes as a String
    fn scopes(&self) -> Result<String, anyhow::Error>;
}

/// Represents the origin of a token discovered through the GitLab API.
#[derive(Debug, Clone)]
pub enum TokenOrigin {
    /// A token belonging directly to a group.
    Group {
        /// The name of the group.
        group_name: String,
        /// Group's URL
        web_url: String,
    },

    /// A token belonging directly to a project.
    Project {
        /// The name of the project.
        project_name: String,
        /// Project's URL
        web_url: String,
    },

    /// A token belonging to a user.
    User {
        /// Username
        user_name: String,
    },
}

/// Represents any token discovered through the GitLab API,
/// together with its origin.
#[derive(Debug)]
pub enum Token {
    /// A project or group access token.
    AccessToken {
        /// The underlying access token.
        token: AccessToken,
        /// Where the token was discovered.
        origin: TokenOrigin,
    },

    /// A user or service account personal access token.
    PersonalAccessToken {
        /// The underlying personal access token.
        token: PersonalAccessToken,
        /// Where the token was discovered.
        origin: TokenOrigin,
    },
}

impl Token {
    /// Returns the token's expiration date, if defined
    pub const fn expiration_date(&self) -> Option<chrono::NaiveDate> {
        match self {
            Self::AccessToken { token, .. } => token.expires_at,
            Self::PersonalAccessToken { token, .. } => token.expires_at,
        }
    }

    /// Returns the `id` field
    pub const fn id(&self) -> u64 {
        match self {
            Self::AccessToken { token, .. } => token.id,
            Self::PersonalAccessToken { token, .. } => token.id,
        }
    }

    /// Returns a String containing prometheus labels specific to `&self`
    pub fn labels(&self) -> Result<String, anyhow::Error> {
        let mut res = String::new();
        match self {
            Self::AccessToken { token, .. } => {
                write!(
                    res,
                    "active=\"{}\",\
                    revoked=\"{}\",\
                    scopes=\"{}\",\
                    access_level=\"{}\"\
                    ",
                    token.active,
                    token.revoked,
                    token.scopes()?,
                    token.access_level
                )?;
            }
            Self::PersonalAccessToken { token, .. } => {
                write!(
                    res,
                    "active=\"{}\",\
                    revoked=\"{}\",\
                    scopes=\"{}\"\
                    ",
                    token.active,
                    token.revoked,
                    token.scopes()?
                )?;
            }
        }

        let origin = match self {
            Self::AccessToken { origin, .. } | Self::PersonalAccessToken { origin, .. } => origin,
        };
        match origin {
            TokenOrigin::Group { web_url, .. } | TokenOrigin::Project { web_url, .. } => {
                write!(res, ",web_url=\"{}\"", escape_label_value(web_url))
                    .context("failed to write web_url to `res`")?;
            }
            TokenOrigin::User { .. } => (),
        }

        Ok(res)
    }

    /// Returns the `name` field
    pub fn name(&self) -> &str {
        match self {
            Self::AccessToken { token, .. } => &token.name,
            Self::PersonalAccessToken { token, .. } => &token.name,
        }
    }

    /// Returns the token's origin type
    pub const fn origin_type(&self) -> &str {
        let origin = match self {
            Self::AccessToken { origin, .. } | Self::PersonalAccessToken { origin, .. } => origin,
        };

        match origin {
            TokenOrigin::Group { .. } => "group",
            TokenOrigin::Project { .. } => "project",
            TokenOrigin::User { .. } => "user",
        }
    }

    /// Returns the token's origin value
    pub fn origin_value(&self) -> &str {
        let origin = match self {
            Self::AccessToken { origin, .. } | Self::PersonalAccessToken { origin, .. } => origin,
        };

        match origin {
            TokenOrigin::Group { group_name, .. } => group_name,
            TokenOrigin::Project { project_name, .. } => project_name,
            TokenOrigin::User { user_name } => user_name,
        }
    }

    /// Returns the token "type" (its origin and its kind)
    #[expect(clippy::unreachable, reason = "some combinations are invalid")]
    pub fn type_(&self) -> &str {
        match self {
            Self::AccessToken { origin, .. } => match origin {
                TokenOrigin::Group { .. } => "group_access",
                TokenOrigin::Project { .. } => "project_access",
                TokenOrigin::User { .. } => unreachable!(),
            },
            Self::PersonalAccessToken { origin, .. } => match origin {
                TokenOrigin::Group { .. } | TokenOrigin::Project { .. } => unreachable!(),
                TokenOrigin::User { .. } => "user_personal_access",
            },
        }
    }
}

/// Describes a GitLab resource that can own tokens and service accounts.
///
/// Implementations provide the URLs needed to discover tokens belonging
/// to that resource.
pub trait TokenSource {
    /// Returns the URL used to retrieve access tokens.
    fn access_tokens_url(&self) -> String;

    /// Returns the origin associated with tokens belonging directly
    /// to this resource.
    fn token_origin(&self) -> impl Future<Output = Result<TokenOrigin, anyhow::Error>> + Send;
}

/// Custom date deserialization function to handle years > 9999
///
/// The default `chrono` deserializer doesn't handle years > 9999, so we have
/// to use `NaiveDate::from_ymd_opt()`
#[expect(
    clippy::indexing_slicing,
    reason = "we check the size of the vec before indexing"
)]
fn deserialize_optional_date<'de, D>(deserializer: D) -> Result<Option<chrono::NaiveDate>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{Error, Unexpected};

    let Some(date_string) = Option::<String>::deserialize(deserializer)? else {
        return Ok(None);
    };

    // `date` format *must* be year-month-day. For example : `2025-06-28` or `10000-12-31`
    let date_split: Vec<_> = date_string.split('-').collect();
    if date_split.len() != 3 {
        return Err(Error::invalid_length(
            date_split.len(),
            &"a year-month-day date format",
        ));
    }

    let year = date_split[0].parse().map_err(|_err| {
        Error::invalid_value(Unexpected::Str(date_split[0]), &"a valid year value")
    })?;
    let month = date_split[1].parse().map_err(|_err| {
        Error::invalid_value(Unexpected::Str(date_split[1]), &"a valid month value")
    })?;
    let day = date_split[2].parse().map_err(|_err| {
        Error::invalid_value(Unexpected::Str(date_split[2]), &"a valid day value")
    })?;

    NaiveDate::from_ymd_opt(year, month, day)
        .ok_or_else(|| Error::invalid_value(Unexpected::Str(&date_string), &"a valid NaiveDate"))
        .map(Some)
}

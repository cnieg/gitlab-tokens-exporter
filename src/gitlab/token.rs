//! Defines a generic [`Token`]

pub mod access;
pub mod deploy;
pub mod personal;
pub mod pipeline;

use anyhow::Context as _;
use chrono::NaiveDate;
use core::fmt::Write as _; // To be able to use the `write` macro
use serde::Deserialize as _;

use crate::{
    gitlab::{
        service_account::ServiceAccount,
        token::{
            access::AccessToken, deploy::DeployToken, personal::PersonalAccessToken,
            pipeline::PipelineTriggerToken,
        },
    },
    prometheus_metrics::escape_label_value,
};

/// Defined for tokens with scopes
pub trait HasScopes {
    /// Returns the list of scopes as a String
    fn scopes(&self) -> Result<String, anyhow::Error>;
}

/// Trait implemented by specific token types
pub trait SpecificToken {
    /// Converts a specific token and its origin into a [`Token`]
    fn into_token(self, origin: TokenOrigin) -> Token;
    /// Return `true` when the token's expiration date is `None`
    fn is_non_expiring(&self) -> bool;
    /// Get the token's type as a string slice
    fn type_name() -> &'static str;
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

    /// A token belonging to a group service account.
    GroupServiceAccount {
        /// The name of the group.
        group_name: String,
        /// Service account name
        service_account_name: String,
        /// Service account username
        service_account_username: String,
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

    /// A token belonging to a project service account.
    ProjectServiceAccount {
        /// The name of the project.
        project_name: String,
        /// Service account name
        service_account_name: String,
        /// Service account username
        service_account_username: String,
        /// Project's URL
        web_url: String,
    },

    /// A token belonging to a user.
    User {
        /// Username
        user_name: String,
    },
}

impl TokenOrigin {
    /// Returns the origin type name
    pub const fn type_name(&self) -> &str {
        match self {
            Self::Group { .. } => "group",
            Self::GroupServiceAccount { .. } => "group_service_account",
            Self::Project { .. } => "project",
            Self::ProjectServiceAccount { .. } => "project_service_account",
            Self::User { .. } => "user",
        }
    }
}

/// Represents any token discovered through the GitLab API,
/// together with its origin.
#[derive(Debug)]
#[expect(clippy::enum_variant_names, reason = "more readable when using Self::")]
pub enum Token {
    /// A project or group access token.
    AccessToken {
        /// The underlying access token.
        token: AccessToken,
        /// Where the token was discovered.
        origin: TokenOrigin,
    },

    /// A project or group deploy token.
    DeployToken {
        /// The underlying deploy token.
        token: DeployToken,

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

    /// A project pipeline trigger token.
    PipelineTriggerToken {
        /// The underlying pipeline trigger token.
        token: PipelineTriggerToken,
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
            Self::DeployToken { token, .. } => token.expires_at,
            Self::PipelineTriggerToken { token, .. } => token.expires_at,
        }
    }

    /// Returns the `id` field
    pub const fn id(&self) -> u64 {
        match self {
            Self::AccessToken { token, .. } => token.id,
            Self::PersonalAccessToken { token, .. } => token.id,
            Self::DeployToken { token, .. } => token.id,
            Self::PipelineTriggerToken { token, .. } => token.id,
        }
    }

    /// Returns a String containing prometheus labels specific to `&self`
    /// Common labels (name, id, type, origin type) are already handled in `src/prometheus_metrics.rs`
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
            Self::DeployToken { token, .. } => {
                write!(
                    res,
                    "revoked=\"{}\",scopes=\"{}\",expired=\"{}\",username=\"{}\"",
                    token.revoked,
                    token.scopes()?,
                    token.expired,
                    escape_label_value(&token.username)
                )?;
            }
            Self::PipelineTriggerToken { token, .. } => {
                write!(
                    res,
                    "username=\"{}\"",
                    escape_label_value(&token.owner.username)
                )?;
            }
        }

        match self.origin() {
            TokenOrigin::Group { web_url, .. }
            | TokenOrigin::Project { web_url, .. }
            | TokenOrigin::GroupServiceAccount { web_url, .. }
            | TokenOrigin::ProjectServiceAccount { web_url, .. } => {
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
            Self::DeployToken { token, .. } => &token.name,
            Self::PipelineTriggerToken { token, .. } => &token.description,
        }
    }

    /// Returns the [`TokenOrigin`] of a [`Token`]
    const fn origin(&self) -> &TokenOrigin {
        match self {
            Self::AccessToken { origin, .. }
            | Self::PersonalAccessToken { origin, .. }
            | Self::DeployToken { origin, .. }
            | Self::PipelineTriggerToken { origin, .. } => origin,
        }
    }

    /// Returns the token's origin type as a string slice
    pub const fn origin_type(&self) -> &str {
        let origin = match self {
            Self::AccessToken { origin, .. }
            | Self::PersonalAccessToken { origin, .. }
            | Self::DeployToken { origin, .. }
            | Self::PipelineTriggerToken { origin, .. } => origin,
        };

        origin.type_name()
    }

    /// Returns the token's origin value
    pub fn origin_value(&self) -> &str {
        let origin = match self {
            Self::AccessToken { origin, .. }
            | Self::PersonalAccessToken { origin, .. }
            | Self::DeployToken { origin, .. }
            | Self::PipelineTriggerToken { origin, .. } => origin,
        };

        match origin {
            TokenOrigin::Group { group_name, .. } => group_name,
            TokenOrigin::GroupServiceAccount {
                service_account_name,
                ..
            }
            | TokenOrigin::ProjectServiceAccount {
                service_account_name,
                ..
            } => service_account_name,
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
                TokenOrigin::User { .. }
                | TokenOrigin::GroupServiceAccount { .. }
                | TokenOrigin::ProjectServiceAccount { .. } => unreachable!(),
            },
            Self::PersonalAccessToken { origin, .. } => match origin {
                TokenOrigin::Group { .. } | TokenOrigin::Project { .. } => unreachable!(),
                TokenOrigin::GroupServiceAccount { .. } => "group_service_account_personal_access",
                TokenOrigin::ProjectServiceAccount { .. } => {
                    "project_service_account_personal_access"
                }
                TokenOrigin::User { .. } => "user_personal_access",
            },
            Self::DeployToken { origin, .. } => match origin {
                TokenOrigin::Group { .. } => "group_deploy",
                TokenOrigin::Project { .. } => "project_deploy",
                TokenOrigin::User { .. }
                | TokenOrigin::GroupServiceAccount { .. }
                | TokenOrigin::ProjectServiceAccount { .. } => unreachable!(),
            },
            Self::PipelineTriggerToken { origin, .. } => match origin {
                TokenOrigin::Project { .. } => "project_pipeline_trigger",
                TokenOrigin::Group { .. }
                | TokenOrigin::User { .. }
                | TokenOrigin::GroupServiceAccount { .. }
                | TokenOrigin::ProjectServiceAccount { .. } => unreachable!(),
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

    /// Returns the URL used to retrieve deploy tokens.
    fn deploy_tokens_url(&self) -> String;

    /// Returns the URL used to retrieve trigger tokens, if supported.
    fn pipeline_trigger_tokens_url(&self) -> Option<String>;

    /// Returns the origin associated with tokens belonging to
    /// a service account of this resource.
    async fn service_account_origin(
        &self,
        service_account: &ServiceAccount,
    ) -> Result<TokenOrigin, anyhow::Error>;

    /// Returns the URL used to retrieve personal access tokens
    /// belonging to a service account.
    fn service_account_pat_url(&self, service_account_id: u64) -> String;

    /// Returns the URL used to retrieve service accounts.
    fn service_accounts_url(&self) -> String;

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
pub fn deserialize_optional_date<'de, D>(
    deserializer: D,
) -> Result<Option<chrono::NaiveDate>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{Error, Unexpected};

    let Some(mut date_string) = Option::<String>::deserialize(deserializer)? else {
        return Ok(None);
    };

    // ⚠️ deploy tokens have a different format! ("2020-02-14T00:00:00.000Z")
    // We just discard everything after (and including) 'T'
    if date_string.contains('T') && date_string.ends_with('Z') {
        date_string = date_string
            .split('T')
            .collect::<Vec<&str>>()
            .first()
            .unwrap_or(&"")
            .to_string();
    }

    // `date_string` format *must* be year-month-day. For example : `2025-06-28` or `10000-12-31`
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

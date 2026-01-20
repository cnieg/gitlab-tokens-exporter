//! Defines the 2 kinds of gitlab token we interact with : [`AccessToken`] and [`PersonalAccessToken`]
use chrono::NaiveDate;
use core::fmt::Write as _; // To be able to use the `Write` trait
use core::fmt::{Display, Formatter};
use serde::Deserialize;
use serde_repr::Deserialize_repr;

use crate::gitlab::pagination::OffsetBasedPagination;

/// cf <https://docs.gitlab.com/api/project_access_tokens/#create-a-project-access-token>
#[derive(Debug, Deserialize_repr)]
#[repr(u8)]
pub enum AccessLevel {
    Developer = 30,
    Guest = 10,
    Maintainer = 40,
    Owner = 50,
    Reporter = 20,
}

impl Display for AccessLevel {
    #[expect(clippy::absolute_paths, reason = "Use a specific Result type")]
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{}",
            match *self {
                Self::Guest => "guest",
                Self::Reporter => "reporter",
                Self::Developer => "developer",
                Self::Maintainer => "maintainer",
                Self::Owner => "owner",
            },
        )
    }
}

/// Defines a [gitlab access token](https://docs.gitlab.com/api/project_access_tokens/#list-project-access-tokens)
#[derive(Debug, Deserialize)]
pub struct AccessToken {
    /// Access level
    pub access_level: AccessLevel,
    /// Active
    pub active: bool,
    /// Expiration date
    #[serde(deserialize_with = "deserialize_optional_date")]
    pub expires_at: Option<chrono::NaiveDate>,
    /// Id
    pub id: usize,
    /// Name
    pub name: String,
    /// Revoked
    pub revoked: bool,
    /// [Scopes](https://docs.gitlab.com/user/project/settings/project_access_tokens/#scopes-for-a-project-access-token)
    pub scopes: Vec<AccessTokenScope>,
}

#[expect(clippy::missing_trait_methods, reason = "we don't need it")]
impl OffsetBasedPagination<Self> for AccessToken {}

/// Scopes used by [`AccessToken`] (for [`Project`](crate::gitlab::project::Project) and [`Group`](crate::gitlab::group::Group))
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessTokenScope {
    /// Grants permission to perform API actions for GitLab Duo
    AiFeatures,
    /// Grants complete read and write access to the scoped group and related project API
    Api,
    /// Grants permission to create runners
    CreateRunner,
    /// Granular
    ///
    /// This scope has been introduced in gitlab [18.5.0-ee](https://gitlab.com/gitlab-org/gitlab/-/merge_requests/207412)
    ///
    /// It has been hidden in gitlab [18.6.0-ee](https://gitlab.com/gitlab-org/gitlab/-/issues/578375)
    ///
    /// We still have to support it in case a token has been created with this scope (even if the token has been revoked)
    Granular,
    /// Grants permission to perform Kubernetes API calls using the agent for Kubernetes
    K8sProxy,
    /// Grants permission to manage runners
    ManageRunner,
    /// Mcp
    ///
    /// This scope has been introduced by mistake in gitlab [18.3.0](https://gitlab.com/gitlab-org/gitlab/-/issues/554826)
    ///
    /// It has then been hidden in gitlab [18.7.0](https://gitlab.com/gitlab-org/gitlab/-/issues/581526)
    ///
    /// We still have to support it in case a token has been created with this scope (even if the token has been revoked)
    Mcp,
    /// Grants read access to the scoped group and related project API
    ReadApi,
    /// Grants read access (pull) to observability data
    ReadObservability,
    /// Grants read access (pull) to the container registry images
    ReadRegistry,
    /// Grants read access (pull) to the repository or all repositories within a group
    ReadRepository,
    /// If a project is private and authorization is required, grants read-only (pull) access to container images through the dependency proxy
    ReadVirtualRegistry,
    /// Grants permission to rotate this token
    SelfRotate,
    /// Grants write access (push) to observability data
    WriteObservability,
    /// Grants write access (push) to the container registry
    WriteRegistry,
    /// Grants read and write access (pull and push) to the repository or to all repositories within a group
    WriteRepository,
    /// If a project is private and authorization is required, grants read (pull), write (push), and delete access to container images through the dependency proxy
    WriteVirtualRegistry,
}

#[expect(clippy::absolute_paths, reason = "Specific Trait and Result type")]
impl core::fmt::Display for AccessTokenScope {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match *self {
            Self::AiFeatures => write!(f, "ai_features"),
            Self::Api => write!(f, "api"),
            Self::CreateRunner => write!(f, "create_runner"),
            Self::Granular => write!(f, "granular"),
            Self::K8sProxy => write!(f, "k8s_proxy"),
            Self::ManageRunner => write!(f, "manage_runner"),
            Self::Mcp => write!(f, "mcp"),
            Self::ReadApi => write!(f, "read_api"),
            Self::ReadObservability => write!(f, "read_observability"),
            Self::ReadRegistry => write!(f, "read_registry"),
            Self::ReadRepository => write!(f, "read_repository"),
            Self::ReadVirtualRegistry => write!(f, "read_virtual_repository"),
            Self::SelfRotate => write!(f, "self_rotate"),
            Self::WriteObservability => write!(f, "write_observability"),
            Self::WriteRegistry => write!(f, "write_registry"),
            Self::WriteRepository => write!(f, "write_repository"),
            Self::WriteVirtualRegistry => write!(f, "write_virtual_registry"),
        }
    }
}

/// Defines a [gitlab personal access token](https://docs.gitlab.com/api/personal_access_tokens/#list-personal-access-tokens)
#[derive(Debug, Deserialize)]
pub struct PersonalAccessToken {
    /// Active
    pub active: bool,
    /// Expiration date
    #[serde(deserialize_with = "deserialize_optional_date")]
    pub expires_at: Option<chrono::NaiveDate>,
    /// Id
    pub id: usize,
    /// Name
    pub name: String,
    /// Revoked
    pub revoked: bool,
    /// [Scopes](https://docs.gitlab.com/user/profile/personal_access_tokens/#personal-access-token-scopes)
    pub scopes: Vec<PersonalAccessTokenScope>,
    /// User id
    pub user_id: usize,
}

#[expect(clippy::missing_trait_methods, reason = "we don't need it")]
impl OffsetBasedPagination<Self> for PersonalAccessToken {}

/// Scopes used by [`PersonalAccessToken`] (for [`User`](crate::gitlab::user::User))
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersonalAccessTokenScope {
    /// Grants permission to perform API actions when Admin Mode is enabled
    AdminMode,
    /// Grants permission to perform API actions for features like GitLab Duo, Code Suggestions API and Duo Chat API
    AiFeatures,
    /// Grants complete read/write access to the API
    Api,
    /// Grants permission to create runners
    CreateRunner,
    /// Granular
    ///
    /// This scope has been introduced in gitlab [18.5.0-ee](https://gitlab.com/gitlab-org/gitlab/-/merge_requests/207412)
    ///
    /// It has been hidden in gitlab [18.6.0-ee](https://gitlab.com/gitlab-org/gitlab/-/issues/578375)
    ///
    /// We still have to support it in case a token has been created with this scope (even if the token has been revoked)
    Granular,
    /// Grants permission to perform Kubernetes API calls using the agent for Kubernetes
    K8sProxy,
    /// Grants permission to manage runners
    ManageRunner,
    /// Mcp
    ///
    /// This scope has been introduced by mistake in gitlab [18.3.0](https://gitlab.com/gitlab-org/gitlab/-/issues/554826)
    ///
    /// It has then been hidden in gitlab [18.7.0](https://gitlab.com/gitlab-org/gitlab/-/issues/581526)
    ///
    /// We still have to support it in case a token has been created with this scope (even if the token has been revoked)
    Mcp,
    /// Grants read access to the API
    ReadApi,
    /// Grants read access (pull) to observability data
    ReadObservability,
    /// Grants read-only (pull) access to container registry images if a project is private and authorization is required
    ReadRegistry,
    /// Grants read-only access to repositories on private projects
    ReadRepository,
    /// Grant access to download Service Ping payload through the API when authenticated as an admin use
    ReadServicePing,
    /// Grants read-only access to the authenticated user’s profile through the /user API endpoint
    ReadUser,
    /// If a project is private and authorization is required, grants read-only (pull) access to container images through the dependency proxy
    ReadVirtualRegistry,
    /// Grants permission to rotate this token
    SelfRotate,
    /// Grants permission to perform API actions as any user in the system, when authenticated as an administrator
    Sudo,
    /// Grants write access (push) to observability data
    WriteObservability,
    /// Grants read-write (push) access to container registry images if a project is private and authorization is required
    WriteRegistry,
    /// Grants read-write access to repositories on private projects
    WriteRepository,
    /// If a project is private and authorization is required, grants read (pull), write (push), and delete access to container images through the dependency proxy
    WriteVirtualRegistry,
}

#[expect(clippy::absolute_paths, reason = "Specific Trait and Result type")]
impl core::fmt::Display for PersonalAccessTokenScope {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match *self {
            Self::AdminMode => write!(f, "admin_mode"),
            Self::AiFeatures => write!(f, "ai_features"),
            Self::Api => write!(f, "api"),
            Self::CreateRunner => write!(f, "create_runner"),
            Self::Granular => write!(f, "granular"),
            Self::K8sProxy => write!(f, "k8s_proxy"),
            Self::ManageRunner => write!(f, "manage_runner"),
            Self::Mcp => write!(f, "mcp"),
            Self::ReadApi => write!(f, "read_api"),
            Self::ReadObservability => write!(f, "read_observability"),
            Self::ReadRegistry => write!(f, "read_registry"),
            Self::ReadRepository => write!(f, "read_repository"),
            Self::ReadServicePing => write!(f, "read_service_ping"),
            Self::ReadUser => write!(f, "read_user"),
            Self::ReadVirtualRegistry => write!(f, "read_virtual_registry"),
            Self::SelfRotate => write!(f, "self_rotate"),
            Self::Sudo => write!(f, "sudo"),
            Self::WriteObservability => write!(f, "write_observability"),
            Self::WriteRegistry => write!(f, "write_registry"),
            Self::WriteRepository => write!(f, "write_repository"),
            Self::WriteVirtualRegistry => write!(f, "write_virtual_registry"),
        }
    }
}

#[derive(Debug)]
#[expect(clippy::missing_docs_in_private_items, reason = "self documented ;)")]
/// A common token type
pub enum Token {
    /// Group token
    Group {
        token: AccessToken,
        full_path: String,
        web_url: String,
    },
    /// Project token
    Project {
        token: AccessToken,
        full_path: String,
        web_url: String,
    },
    /// User token
    User {
        token: PersonalAccessToken,
        full_path: String,
    },
}

impl Token {
    /// Convert token scopes ([`AccessTokenScope`] or [`PersonalAccessTokenScope`]) into a String
    #[expect(clippy::absolute_paths, reason = "Using a specific type")]
    pub fn scopes(&self) -> Result<String, core::fmt::Error> {
        let mut res = String::from("[");

        match *self {
            Self::Group { ref token, .. } | Self::Project { ref token, .. } => {
                for scope in &token.scopes {
                    write!(res, "{scope},")?;
                }
            }
            Self::User { ref token, .. } => {
                for scope in &token.scopes {
                    write!(res, "{scope},")?;
                }
            }
        }

        if res.ends_with(',') {
            res.pop();
        }

        res.push(']');
        Ok(res)
    }
}

/// Custom date deserialization function to handle years > 9999
///
/// The default `chrono` deserializer doesn't handle years > 9999, so we have
/// to use `NaiveDate::from_ymd_opt()`
#[expect(
    clippy::indexing_slicing,
    reason = "We check the size of the vec before indexing"
)]
fn deserialize_optional_date<'de, D>(deserializer: D) -> Result<Option<chrono::NaiveDate>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{Error, Unexpected};

    match Option::<String>::deserialize(deserializer)? {
        Some(date_string) => {
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
            match NaiveDate::from_ymd_opt(year, month, day) {
                Some(date) => Ok(Some(date)),
                None => Err(Error::invalid_value(
                    Unexpected::Str(&date_string),
                    &"a valid NaiveDate",
                )),
            }
        }
        None => Ok(None),
    }
}

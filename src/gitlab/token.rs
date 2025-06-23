//! Defines the 2 kinds of gitlab token we interact with : [`AccessToken`] and [`PersonalAccessToken`]
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
    #[expect(clippy::min_ident_chars, reason = "Parameter name from std trait")]
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
    pub expires_at: chrono::NaiveDate,
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
    /// Grants permission to perform Kubernetes API calls using the agent for Kubernetes
    K8sProxy,
    /// Grants permission to manage runners
    ManageRunner,
    /// Grants read access to the scoped group and related project API
    ReadApi,
    /// Grants read access (pull) to the container registry images
    ReadRegistry,
    /// Grants read access (pull) to the repository or all repositories within a group
    ReadRepository,
    /// If a project is private and authorization is required, grants read-only (pull) access to container images through the dependency proxy
    ReadVirtualRegistry,
    /// Grants permission to rotate this token
    SelfRotate,
    /// Grants write access (push) to the container registry
    WriteRegistry,
    /// Grants read and write access (pull and push) to the repository or to all repositories within a group
    WriteRepository,
    /// If a project is private and authorization is required, grants read (pull), write (push), and delete access to container images through the dependency proxy
    WriteVirtualRegistry,
}

#[expect(clippy::absolute_paths, reason = "Specific Trait and Result type")]
impl core::fmt::Display for AccessTokenScope {
    #[expect(
        clippy::min_ident_chars,
        reason = "Using the default function parameter name"
    )]
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match *self {
            Self::AiFeatures => write!(f, "ai_features"),
            Self::Api => write!(f, "api"),
            Self::CreateRunner => write!(f, "create_runner"),
            Self::K8sProxy => write!(f, "k8s_proxy"),
            Self::ManageRunner => write!(f, "manage_runner"),
            Self::ReadApi => write!(f, "read_api"),
            Self::ReadRegistry => write!(f, "read_registry"),
            Self::ReadRepository => write!(f, "read_repository"),
            Self::ReadVirtualRegistry => write!(f, "read_virtual_repository"),
            Self::SelfRotate => write!(f, "self_rotate"),
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
    pub expires_at: chrono::NaiveDate,
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
    /// Grants permission to perform Kubernetes API calls using the agent for Kubernetes
    K8sProxy,
    /// Grants permission to manage runners
    ManageRunner,
    /// Grants read access to the API
    ReadApi,
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
    /// Grants read-write (push) access to container registry images if a project is private and authorization is required
    WriteRegistry,
    /// Grants read-write access to repositories on private projects
    WriteRepository,
    /// If a project is private and authorization is required, grants read (pull), write (push), and delete access to container images through the dependency proxy
    WriteVirtualRegistry,
}

#[expect(clippy::absolute_paths, reason = "Specific Trait and Result type")]
impl core::fmt::Display for PersonalAccessTokenScope {
    #[expect(
        clippy::min_ident_chars,
        reason = "Using the default function parameter name"
    )]
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match *self {
            Self::AdminMode => write!(f, "admin_mode"),
            Self::AiFeatures => write!(f, "ai_features"),
            Self::Api => write!(f, "api"),
            Self::CreateRunner => write!(f, "create_runner"),
            Self::K8sProxy => write!(f, "k8s_proxy"),
            Self::ManageRunner => write!(f, "manage_runner"),
            Self::ReadApi => write!(f, "read_api"),
            Self::ReadRegistry => write!(f, "read_registry"),
            Self::ReadRepository => write!(f, "read_repository"),
            Self::ReadServicePing => write!(f, "read_service_ping"),
            Self::ReadUser => write!(f, "read_user"),
            Self::ReadVirtualRegistry => write!(f, "read_virtual_registry"),
            Self::SelfRotate => write!(f, "self_rotate"),
            Self::Sudo => write!(f, "sudo"),
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

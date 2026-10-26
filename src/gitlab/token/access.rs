//! Defines an [`AccessToken`]

use crate::gitlab::token::deserialize_optional_date;
use core::fmt::{Display, Formatter};
use serde::Deserialize;
use serde_repr::Deserialize_repr;

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
    #[expect(clippy::absolute_paths, reason = "use a specific Result type")]
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

#[expect(clippy::absolute_paths, reason = "specific Trait and Result type")]
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

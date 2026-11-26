//! Defines an [`PersonalAccessToken`]

use crate::config::CONFIG;
use crate::gitlab::root_resource::GitlabRootResource;
use crate::gitlab::token::{HasScopes, IntoToken, TokenExpiration, deserialize_optional_date};
use anyhow::Context as _;
use core::fmt::Formatter;
use core::fmt::Write as _;
use serde::Deserialize;

/// Defines a [gitlab personal access token](https://docs.gitlab.com/api/personal_access_tokens/#list-personal-access-tokens)
#[derive(Debug, Deserialize)]
pub struct PersonalAccessToken {
    /// Active
    pub active: bool,
    /// Expiration date
    #[serde(deserialize_with = "deserialize_optional_date")]
    pub expires_at: Option<chrono::NaiveDate>,
    /// Id
    pub id: u64,
    /// Name
    pub name: String,
    /// Revoked
    pub revoked: bool,
    /// [Scopes](https://docs.gitlab.com/user/profile/personal_access_tokens/#personal-access-token-scopes)
    pub scopes: Vec<PersonalAccessTokenScope>,
    /// User id
    pub user_id: u64,
}

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

#[expect(clippy::absolute_paths, reason = "specific Trait and Result type")]
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

impl HasScopes for PersonalAccessToken {
    fn scopes(&self) -> Result<String, anyhow::Error> {
        let mut res = String::from("[");

        for scope in &self.scopes {
            write!(res, "{scope},").context("failed to write access token scopes")?;
        }

        if res.ends_with(',') {
            res.pop();
        }

        res.push(']');
        Ok(res)
    }
}

impl GitlabRootResource for PersonalAccessToken {
    fn first_url() -> String {
        format!(
            "https://{}/api/v4/personal_access_tokens?per_page=100",
            CONFIG.connection.hostname
        )
    }

    fn resource_type_name() -> &'static str {
        "personal access token"
    }
}

impl IntoToken for PersonalAccessToken {
    fn into_token(self, origin: super::TokenOrigin) -> super::Token {
        super::Token::PersonalAccessToken {
            token: self,
            origin,
        }
    }
}

impl TokenExpiration for PersonalAccessToken {
    fn is_non_expiring(&self) -> bool {
        self.expires_at.is_none()
    }
}

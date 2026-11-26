//! Defines an [`DeployToken`]
//!
use crate::gitlab::token::{HasScopes, SpecificToken, deserialize_optional_date};
use anyhow::Context as _;
use core::fmt::{Formatter, Write as _};
use serde::Deserialize;

/// Defines a [gitlab deploy token](https://docs.gitlab.com/api/deploy_tokens/)
///
/// Deploy tokens have no `access_level` and no `active` field (their state is
/// described by `revoked` and the computed days remaining).
#[derive(Debug, Deserialize)]
pub struct DeployToken {
    /// Expired
    pub expired: bool,
    /// Expiration date
    ///
    /// Deploy token endpoints return this as an ISO 8601 datetime
    /// (e.g. `2026-12-31T00:00:00.000Z`), unlike the date-only format used by
    /// the other token endpoints.
    #[serde(deserialize_with = "deserialize_optional_date")]
    pub expires_at: Option<chrono::NaiveDate>,
    /// Id
    pub id: u64,
    /// Name
    pub name: String,
    /// Revoked
    pub revoked: bool,
    /// [Scopes](https://docs.gitlab.com/user/project/deploy_tokens/#scope)
    pub scopes: Vec<DeployTokenScope>,
    /// Username
    pub username: String,
}

/// Scopes used by [`DeployToken`] (for [`Project`](crate::gitlab::project::Project) and [`Group`](crate::gitlab::group::Group))
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeployTokenScope {
    /// Allows read-only access to the package registry
    ReadPackageRegistry,
    /// Allows read access (pull) to the container registry images
    ReadRegistry,
    /// Allows read access (pull) to the repository
    ReadRepository,
    /// Allows read access to the group's dependency proxy / virtual registry
    ReadVirtualRegistry,
    /// Allows write access to the package registry
    WritePackageRegistry,
    /// Allows write access (push) to the container registry
    WriteRegistry,
    /// Allows write access to the group's dependency proxy / virtual registry
    WriteVirtualRegistry,
}

#[expect(clippy::absolute_paths, reason = "specific Trait and Result type")]
impl core::fmt::Display for DeployTokenScope {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match *self {
            Self::ReadPackageRegistry => write!(f, "read_package_registry"),
            Self::ReadRegistry => write!(f, "read_registry"),
            Self::ReadRepository => write!(f, "read_repository"),
            Self::ReadVirtualRegistry => write!(f, "read_virtual_repository"),
            Self::WritePackageRegistry => write!(f, "write_package_registry"),
            Self::WriteRegistry => write!(f, "write_registry"),
            Self::WriteVirtualRegistry => write!(f, "write_virtual_registry"),
        }
    }
}

impl HasScopes for DeployToken {
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

impl SpecificToken for DeployToken {
    fn into_token(self, origin: super::TokenOrigin) -> super::Token {
        super::Token::DeployToken {
            token: self,
            origin,
        }
    }

    fn is_non_expiring(&self) -> bool {
        self.expires_at.is_none()
    }

    fn type_name() -> &'static str {
        "deploy"
    }
}

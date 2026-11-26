//! Defines an [`PipelineTriggerToken`]

use crate::gitlab::token::{SpecificToken, deserialize_optional_date};
use serde::Deserialize;

/// Defines a [gitlab pipeline trigger token](https://docs.gitlab.com/api/pipeline_triggers/#list-project-trigger-tokens)
#[derive(Debug, Deserialize)]
pub struct PipelineTriggerToken {
    /// Description
    pub description: String,
    /// Expiration date
    #[serde(deserialize_with = "deserialize_optional_date")]
    pub expires_at: Option<chrono::NaiveDate>,
    /// Id
    pub id: u64,
    /// Owner
    pub owner: Owner,
}

/// Describes the owner of a pipeline trigger token
#[derive(Debug, Deserialize)]
pub struct Owner {
    /// Id
    pub id: u64,
    /// Username
    pub username: String,
}

impl SpecificToken for PipelineTriggerToken {
    fn into_token(self, origin: super::TokenOrigin) -> super::Token {
        super::Token::PipelineTriggerToken {
            token: self,
            origin,
        }
    }

    fn is_non_expiring(&self) -> bool {
        self.expires_at.is_none()
    }

    fn type_name() -> &'static str {
        "pipeline trigger"
    }
}

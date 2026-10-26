//! Defines a generic [`Token`]

pub mod access;
pub mod personal;

use anyhow::Context as _;
use chrono::NaiveDate;
use core::fmt::Write as _; // To be able to use the `write` macro
use serde::Deserialize as _;

use crate::gitlab::token::{access::AccessToken, personal::PersonalAccessToken};

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
    pub fn scopes(&self) -> Result<String, anyhow::Error> {
        let mut res = String::from("[");

        match self {
            Self::Group { token, .. } | Self::Project { token, .. } => {
                for scope in &token.scopes {
                    write!(res, "{scope},").context("failed to write projet|group token scopes")?;
                }
            }
            Self::User { token, .. } => {
                for scope in &token.scopes {
                    write!(res, "{scope},").context("failed to write user token scopes")?;
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

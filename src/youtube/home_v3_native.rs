#![allow(dead_code)]

//! Parser for the native Home V3 helper payload.
//!
//! This module accepts the JSON shape emitted by `helpers/nocky_youtube_home_v3.py`
//! and converts it into the neutral `HomeV3SourcePage` contract. Runtime wiring
//! happens in a later stack step through `home_v3_source`.

use serde::Deserialize;
use std::{error::Error, fmt};

use super::home_v3_adapter::HomeV3SourcePage;

const SUPPORTED_HOME_V3_VERSION: u32 = 3;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
struct HomeV3NativeHelperResponse {
    ok: bool,
    result: Option<HomeV3SourcePage>,
    error: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
struct HomeV3NativePayload {
    version: u32,
    selected_chip_params: String,
    chips: Vec<super::home_v3_adapter::HomeV3SourceChip>,
    sections: Vec<super::home_v3_adapter::HomeV3SourceSection>,
    continuation: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum HomeV3NativeParseError {
    Json(String),
    Helper(String),
    MissingResult,
    UnsupportedVersion(u32),
}

impl fmt::Display for HomeV3NativeParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(formatter, "invalid Home V3 payload: {error}"),
            Self::Helper(error) => write!(formatter, "Home V3 helper failed: {error}"),
            Self::MissingResult => write!(formatter, "Home V3 helper returned no result"),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported Home V3 payload version: {version}")
            }
        }
    }
}

impl Error for HomeV3NativeParseError {}

pub(crate) fn parse_native_home_v3_payload(
    payload: &str,
) -> Result<HomeV3SourcePage, HomeV3NativeParseError> {
    let payload = serde_json::from_str::<HomeV3NativePayload>(payload)
        .map_err(|error| HomeV3NativeParseError::Json(error.to_string()))?;

    if payload.version != SUPPORTED_HOME_V3_VERSION {
        return Err(HomeV3NativeParseError::UnsupportedVersion(payload.version));
    }

    Ok(HomeV3SourcePage {
        chips: payload.chips,
        sections: payload.sections,
        continuation: payload.continuation,
        selected_chip_params: payload.selected_chip_params,
    })
}

pub(crate) fn parse_native_home_v3_helper_response(
    output: &[u8],
) -> Result<HomeV3SourcePage, HomeV3NativeParseError> {
    let response = serde_json::from_slice::<HomeV3NativeHelperResponse>(output)
        .map_err(|error| HomeV3NativeParseError::Json(error.to_string()))?;

    if !response.ok {
        return Err(HomeV3NativeParseError::Helper(
            response
                .error
                .unwrap_or_else(|| "unknown Home V3 helper error".to_string()),
        ));
    }

    response.result.ok_or(HomeV3NativeParseError::MissingResult)
}

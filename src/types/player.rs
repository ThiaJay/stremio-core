use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Deserialize, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum VideoScale {
    Contain,
    Cover,
    Fill,
}

#[derive(Clone, Copy, Deserialize, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SubtitleSource {
    Embedded,
    External,
}

/// Audio preference for the current Player session, preserved across Player loads.
#[derive(Clone, Deserialize, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioPreference {
    /// Preferred normalized language code, or `None` when it is unavailable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

/// Subtitle preference for the current Player session.
///
/// It is preserved across Player loads and is intentionally independent from
/// episode-specific subtitle tracks.
#[derive(Clone, Deserialize, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitlePreference {
    pub enabled: bool,
    /// Preferred source, or `None` to keep the client's normal source ordering.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<SubtitleSource>,
    /// Preferred normalized language code, or `None` when it is unavailable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

#[derive(Clone, Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IntroOutro {
    pub intro: Option<IntroData>,
    pub outro: Option<u64>,
}

#[derive(Clone, Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IntroData {
    pub from: u64,
    pub to: u64,
    /// `Some` if the difference between the skip gap data
    /// and stream duration ([`LibraryItem.state.duration`]) > 0!
    pub duration: Option<u64>,
}

/// Portable resolved skip state. Clients perform seek_to through the normal media actuator.
#[derive(Clone, Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkipSegmentState {
    pub kind: crate::types::skip_segments::SkipSegmentKind,
    pub generation: u64,
    pub video_id: String,
    pub from: u64,
    pub to: u64,
    pub duration: u64,
    pub mode: crate::types::profile::SkipIntroMode,
    pub active: bool,
    pub dismissed: bool,
    pub seek_to: Option<u64>,
}

/// Transitional compatibility alias for clients that still consume the intro-only descriptor.
pub type SkipIntroState = SkipSegmentState;

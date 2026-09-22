use serde::{Deserialize, Serialize};

pub mod av_sync_v2;

/// Corrective action requested by Core when persistent A/V drift is detected.
#[derive(Clone, Copy, Deserialize, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AvSyncCorrection {
    /// Prefer an inaudible player-native clock or resampling correction.
    Soft,
    /// Re-anchor the decoders at the current media position.
    Hard,
}

/// A timing sample reported by the active playback backend.
///
/// Positive `offset_ms` means audio is ahead of video. Negative means audio
/// is behind video. Backends that cannot measure separate presentation clocks
/// should not invent an offset and should simply omit reporting samples.
#[derive(Clone, Deserialize, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvSyncObservation {
    pub offset_ms: i64,
    #[serde(default)]
    pub buffering: bool,
    #[serde(default)]
    pub seeking: bool,
    #[serde(default)]
    pub refresh_rate_switching: bool,
    #[serde(default)]
    pub can_soft_correct: bool,
    #[serde(default)]
    pub can_hard_correct: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AvSyncStatus {
    Stable,
    Monitoring,
    Correcting,
    Uncorrectable,
}

/// Cross-platform A/V synchronisation state owned by Core.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvSyncState {
    pub offset_ms: i64,
    pub status: AvSyncStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correction: Option<AvSyncCorrection>,
    /// Monotonic token allowing clients to apply the same correction type more than once.
    pub generation: u64,
}

impl Default for AvSyncState {
    fn default() -> Self {
        Self {
            offset_ms: 0,
            status: AvSyncStatus::Stable,
            correction: None,
            generation: 0,
        }
    }
}

/// Recovery requested by Core after a persistent playback health failure.
#[derive(Clone, Copy, Deserialize, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PlaybackRecoveryAction {
    /// Keep the current stable video path and make the audio compatible locally.
    TranscodeAudio,
    /// Try another playback engine for the same source.
    SwitchPlaybackEngine,
    /// Return to a previously stable video path and make its audio compatible.
    RestoreStableVideoAndTranscodeAudio,
    /// Keep the current title and move to a more compatible source only as a last resort.
    SwitchStream,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PlaybackHealthStatus {
    Healthy,
    Monitoring,
    Recovering,
    Exhausted,
}

/// Health sample reported by a playback backend.
///
/// `audio_present` describes decoded audio activity before user volume or mute is
/// applied. `video_stable` describes sustained presentation rather than a single
/// rendered frame. Clients should set `transient` during startup, buffering,
/// seeking or refresh-rate switching so Core does not react to expected disruption.
#[derive(Clone, Deserialize, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackHealthObservation {
    #[serde(default)]
    pub engine: Option<String>,
    #[serde(default)]
    pub audio_codec: Option<String>,
    #[serde(default)]
    pub audio_expected: bool,
    #[serde(default)]
    pub audio_present: bool,
    #[serde(default = "default_true")]
    pub video_stable: bool,
    #[serde(default)]
    pub transient: bool,
    #[serde(default)]
    pub can_transcode_audio: bool,
    #[serde(default)]
    pub can_switch_engine: bool,
    #[serde(default)]
    pub can_restore_stable_video: bool,
    #[serde(default)]
    pub can_switch_stream: bool,
}

fn default_true() -> bool {
    true
}

/// Cross-platform playback health and recovery state owned by Core.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackHealthState {
    pub status: PlaybackHealthStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery: Option<PlaybackRecoveryAction>,
    pub generation: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_codec: Option<String>,
}

impl Default for PlaybackHealthState {
    fn default() -> Self {
        Self {
            status: PlaybackHealthStatus::Healthy,
            recovery: None,
            generation: 0,
            engine: None,
            audio_codec: None,
        }
    }
}

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

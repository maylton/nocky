use serde::{Deserialize, Serialize};

pub const NOCKY_CONNECT_PROTOCOL_VERSION: u32 = 1;
pub const PLAYBACK_SESSION_SNAPSHOT_SCHEMA: &str =
    "io.github.maylton.nocky.connect.PlaybackSessionSnapshot";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlaybackSessionSnapshot {
    pub schema: String,
    #[serde(rename = "schema_version")]
    pub schema_version: u32,
    #[serde(rename = "session_id")]
    pub session_id: String,
    pub revision: u64,
    #[serde(rename = "origin_device_id")]
    pub origin_device_id: String,
    #[serde(rename = "updated_at_epoch_ms")]
    pub updated_at_epoch_ms: u64,
    #[serde(rename = "updated_at_monotonic_ms")]
    pub updated_at_monotonic_ms: Option<u64>,
    pub source: NockyConnectSource,
    pub playback: PlaybackInfo,
    pub queue: PortableQueue,
}

impl PlaybackSessionSnapshot {
    pub fn new(
        session_id: impl Into<String>,
        revision: u64,
        origin_device_id: impl Into<String>,
        updated_at_epoch_ms: u64,
        source: NockyConnectSource,
        playback: PlaybackInfo,
        queue: PortableQueue,
    ) -> Self {
        Self {
            schema: PLAYBACK_SESSION_SNAPSHOT_SCHEMA.to_string(),
            schema_version: NOCKY_CONNECT_PROTOCOL_VERSION,
            session_id: session_id.into(),
            revision,
            origin_device_id: origin_device_id.into(),
            updated_at_epoch_ms,
            updated_at_monotonic_ms: None,
            source,
            playback,
            queue,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlaybackInfo {
    pub state: NockyPlaybackState,
    #[serde(rename = "position_ms")]
    pub position_ms: u64,
    #[serde(rename = "duration_ms")]
    pub duration_ms: Option<u64>,
    pub rate: f32,
    pub volume: Option<f32>,
    pub muted: bool,
}

impl PlaybackInfo {
    pub fn paused(position_ms: u64, duration_ms: Option<u64>) -> Self {
        Self {
            state: NockyPlaybackState::Paused,
            position_ms,
            duration_ms,
            rate: 1.0,
            volume: None,
            muted: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortableQueue {
    pub title: Option<String>,
    #[serde(rename = "current_index")]
    pub current_index: usize,
    #[serde(rename = "repeat_mode")]
    pub repeat_mode: NockyRepeatMode,
    #[serde(rename = "shuffle_enabled")]
    pub shuffle_enabled: bool,
    #[serde(rename = "shuffle_seed")]
    pub shuffle_seed: Option<u64>,
    pub items: Vec<PortableQueueItem>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortableQueueItem {
    #[serde(rename = "queue_item_id")]
    pub queue_item_id: String,
    pub source: NockyConnectSource,
    pub provider: String,
    #[serde(rename = "playable_id")]
    pub playable_id: String,
    #[serde(rename = "set_video_id")]
    pub set_video_id: Option<String>,
    #[serde(rename = "playlist_id")]
    pub playlist_id: Option<String>,
    #[serde(rename = "browse_id")]
    pub browse_id: Option<String>,
    pub title: String,
    pub artists: Vec<PortableArtist>,
    pub album: Option<PortableAlbum>,
    #[serde(rename = "duration_ms")]
    pub duration_ms: Option<u64>,
    #[serde(rename = "thumbnail_url")]
    pub thumbnail_url: Option<String>,
    pub explicit: bool,
    #[serde(rename = "is_video")]
    pub is_video: bool,
    #[serde(rename = "is_episode")]
    pub is_episode: bool,
    pub local: Option<LocalTrackIdentity>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortableArtist {
    pub id: Option<String>,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortableAlbum {
    pub id: Option<String>,
    pub title: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalTrackIdentity {
    #[serde(rename = "library_id")]
    pub library_id: Option<String>,
    #[serde(rename = "content_hash")]
    pub content_hash: Option<String>,
    #[serde(rename = "relative_path")]
    pub relative_path: Option<String>,
    #[serde(rename = "file_size")]
    pub file_size: Option<u64>,
    #[serde(rename = "modified_at_epoch_ms")]
    pub modified_at_epoch_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NockyConnectSource {
    #[serde(rename = "youtube")]
    YouTube,
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NockyPlaybackState {
    #[serde(rename = "idle")]
    Idle,
    #[serde(rename = "loading")]
    Loading,
    #[serde(rename = "playing")]
    Playing,
    #[serde(rename = "paused")]
    Paused,
    #[serde(rename = "ended")]
    Ended,
    #[serde(rename = "error")]
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NockyRepeatMode {
    #[serde(rename = "off")]
    Off,
    #[serde(rename = "one")]
    One,
    #[serde(rename = "all")]
    All,
}

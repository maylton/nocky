use serde::{Deserialize, Serialize};

pub const NOCKY_CONNECT_PROTOCOL_VERSION: u32 = 1;
pub const PLAYBACK_SESSION_SNAPSHOT_SCHEMA: &str =
    "io.github.maylton.nocky.connect.PlaybackSessionSnapshot";
pub const HANDOFF_MESSAGE_SCHEMA: &str = "io.github.maylton.nocky.connect.HandoffMessage";

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NockyConnectHandoffEnvelope {
    pub schema: String,
    #[serde(rename = "schema_version")]
    pub schema_version: u32,
    #[serde(rename = "message_id")]
    pub message_id: String,
    #[serde(rename = "created_at_epoch_ms")]
    pub created_at_epoch_ms: u64,
    pub kind: NockyConnectHandoffKind,
    pub payload: NockyConnectHandoffPayload,
}

impl NockyConnectHandoffEnvelope {
    pub fn offer(
        message_id: impl Into<String>,
        created_at_epoch_ms: u64,
        offer: NockyConnectHandoffOffer,
    ) -> Self {
        Self {
            schema: HANDOFF_MESSAGE_SCHEMA.to_string(),
            schema_version: NOCKY_CONNECT_PROTOCOL_VERSION,
            message_id: message_id.into(),
            created_at_epoch_ms,
            kind: NockyConnectHandoffKind::Offer,
            payload: NockyConnectHandoffPayload::Offer(offer),
        }
    }

    pub fn accept(
        message_id: impl Into<String>,
        created_at_epoch_ms: u64,
        accept: NockyConnectHandoffAccept,
    ) -> Self {
        Self {
            schema: HANDOFF_MESSAGE_SCHEMA.to_string(),
            schema_version: NOCKY_CONNECT_PROTOCOL_VERSION,
            message_id: message_id.into(),
            created_at_epoch_ms,
            kind: NockyConnectHandoffKind::Accept,
            payload: NockyConnectHandoffPayload::Accept(accept),
        }
    }

    pub fn decline(
        message_id: impl Into<String>,
        created_at_epoch_ms: u64,
        decline: NockyConnectHandoffDecline,
    ) -> Self {
        Self {
            schema: HANDOFF_MESSAGE_SCHEMA.to_string(),
            schema_version: NOCKY_CONNECT_PROTOCOL_VERSION,
            message_id: message_id.into(),
            created_at_epoch_ms,
            kind: NockyConnectHandoffKind::Decline,
            payload: NockyConnectHandoffPayload::Decline(decline),
        }
    }

    pub fn result(
        message_id: impl Into<String>,
        created_at_epoch_ms: u64,
        result: NockyConnectHandoffResult,
    ) -> Self {
        Self {
            schema: HANDOFF_MESSAGE_SCHEMA.to_string(),
            schema_version: NOCKY_CONNECT_PROTOCOL_VERSION,
            message_id: message_id.into(),
            created_at_epoch_ms,
            kind: NockyConnectHandoffKind::Result,
            payload: NockyConnectHandoffPayload::Result(result),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NockyConnectHandoffKind {
    #[serde(rename = "handoff_offer")]
    Offer,
    #[serde(rename = "handoff_accept")]
    Accept,
    #[serde(rename = "handoff_decline")]
    Decline,
    #[serde(rename = "handoff_result")]
    Result,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NockyConnectHandoffPayload {
    Offer(NockyConnectHandoffOffer),
    Accept(NockyConnectHandoffAccept),
    Decline(NockyConnectHandoffDecline),
    Result(NockyConnectHandoffResult),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NockyConnectHandoffOffer {
    #[serde(rename = "offer_id")]
    pub offer_id: String,
    #[serde(rename = "sender_device_id")]
    pub sender_device_id: String,
    #[serde(rename = "sender_device_name")]
    pub sender_device_name: String,
    #[serde(rename = "receiver_device_id")]
    pub receiver_device_id: String,
    #[serde(rename = "snapshot_summary")]
    pub snapshot_summary: NockyConnectSnapshotSummary,
    #[serde(rename = "restore_policy")]
    pub restore_policy: NockyConnectRestorePolicy,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NockyConnectSnapshotSummary {
    pub source: NockyConnectSource,
    #[serde(rename = "current_title")]
    pub current_title: Option<String>,
    #[serde(rename = "current_artist")]
    pub current_artist: Option<String>,
    #[serde(rename = "queue_items")]
    pub queue_items: usize,
    #[serde(rename = "position_ms")]
    pub position_ms: u64,
    #[serde(rename = "duration_ms")]
    pub duration_ms: Option<u64>,
    #[serde(rename = "was_playing")]
    pub was_playing: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NockyConnectRestorePolicy {
    #[serde(rename = "restore_paused")]
    RestorePaused,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NockyConnectHandoffAccept {
    #[serde(rename = "offer_id")]
    pub offer_id: String,
    #[serde(rename = "receiver_device_id")]
    pub receiver_device_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NockyConnectHandoffDecline {
    #[serde(rename = "offer_id")]
    pub offer_id: String,
    #[serde(rename = "receiver_device_id")]
    pub receiver_device_id: String,
    pub reason: NockyConnectHandoffDeclineReason,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NockyConnectHandoffDeclineReason {
    #[serde(rename = "user_declined")]
    UserDeclined,
    #[serde(rename = "busy")]
    Busy,
    #[serde(rename = "unsupported")]
    Unsupported,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NockyConnectHandoffResult {
    #[serde(rename = "offer_id")]
    pub offer_id: String,
    pub status: NockyConnectHandoffResultStatus,
    #[serde(rename = "error_message")]
    pub error_message: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NockyConnectHandoffResultStatus {
    #[serde(rename = "restored_paused")]
    RestoredPaused,
    #[serde(rename = "failed")]
    Failed,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handoff_offer_round_trips_without_snapshot_payload() {
        let envelope = NockyConnectHandoffEnvelope::offer(
            "offer-message-1",
            1_789_000,
            NockyConnectHandoffOffer {
                offer_id: "offer-1".to_string(),
                sender_device_id: "desktop-1".to_string(),
                sender_device_name: "Nocky Desktop".to_string(),
                receiver_device_id: "android-1".to_string(),
                snapshot_summary: NockyConnectSnapshotSummary {
                    source: NockyConnectSource::YouTube,
                    current_title: Some("Juno".to_string()),
                    current_artist: Some("Sabrina Carpenter".to_string()),
                    queue_items: 89,
                    position_ms: 2_267,
                    duration_ms: Some(223_000),
                    was_playing: true,
                },
                restore_policy: NockyConnectRestorePolicy::RestorePaused,
            },
        );

        let encoded = serde_json::to_string(&envelope).expect("offer should encode");
        assert!(!encoded.contains("cookies"));
        assert!(!encoded.contains("headers"));
        assert!(!encoded.contains("stream_url"));

        let decoded: NockyConnectHandoffEnvelope =
            serde_json::from_str(&encoded).expect("offer should decode");

        assert_eq!(decoded, envelope);
        assert_eq!(decoded.schema, HANDOFF_MESSAGE_SCHEMA);
        assert_eq!(decoded.schema_version, NOCKY_CONNECT_PROTOCOL_VERSION);
        assert_eq!(decoded.kind, NockyConnectHandoffKind::Offer);
    }

    #[test]
    fn handoff_accept_decline_and_result_round_trip() {
        let accept = NockyConnectHandoffEnvelope::accept(
            "accept-message-1",
            1_789_001,
            NockyConnectHandoffAccept {
                offer_id: "offer-1".to_string(),
                receiver_device_id: "android-1".to_string(),
            },
        );
        let decline = NockyConnectHandoffEnvelope::decline(
            "decline-message-1",
            1_789_002,
            NockyConnectHandoffDecline {
                offer_id: "offer-2".to_string(),
                receiver_device_id: "android-1".to_string(),
                reason: NockyConnectHandoffDeclineReason::UserDeclined,
            },
        );
        let result = NockyConnectHandoffEnvelope::result(
            "result-message-1",
            1_789_003,
            NockyConnectHandoffResult {
                offer_id: "offer-1".to_string(),
                status: NockyConnectHandoffResultStatus::RestoredPaused,
                error_message: None,
            },
        );

        for envelope in [accept, decline, result] {
            let encoded = serde_json::to_string(&envelope).expect("handoff message should encode");
            let decoded: NockyConnectHandoffEnvelope =
                serde_json::from_str(&encoded).expect("handoff message should decode");
            assert_eq!(decoded, envelope);
        }
    }
}

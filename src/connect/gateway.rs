use std::{
    fmt,
    time::{SystemTime, UNIX_EPOCH},
};

use super::{
    export_desktop_queue_snapshot, restore_desktop_queue_snapshot, DesktopPlaybackState,
    PlaybackSessionSnapshot, RestoredDesktopSnapshot, NOCKY_CONNECT_PROTOCOL_VERSION,
    PLAYBACK_SESSION_SNAPSHOT_SCHEMA,
};
use crate::playback::PlaybackQueue;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NockyConnectError {
    UnsupportedSchema(String),
    UnsupportedSchemaVersion(u32),
    Json(String),
}

impl fmt::Display for NockyConnectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchema(schema) => {
                write!(formatter, "unsupported Nocky Connect schema {schema}")
            }
            Self::UnsupportedSchemaVersion(version) => {
                write!(formatter, "unsupported Nocky Connect schema version {version}")
            }
            Self::Json(error) => write!(formatter, "invalid Nocky Connect JSON: {error}"),
        }
    }
}

impl std::error::Error for NockyConnectError {}

#[derive(Clone, Debug)]
pub struct NockyConnectGateway {
    device_id: String,
}

impl NockyConnectGateway {
    pub fn new(device_id: impl Into<String>) -> Self {
        Self {
            device_id: device_id.into(),
        }
    }

    pub fn export_snapshot(
        &self,
        queue: &PlaybackQueue,
        title: Option<String>,
        playback_state: DesktopPlaybackState,
        session_id: impl Into<String>,
        revision: u64,
    ) -> PlaybackSessionSnapshot {
        export_desktop_queue_snapshot(
            queue,
            title,
            playback_state,
            session_id,
            revision,
            self.device_id.clone(),
            now_epoch_ms(),
        )
    }

    pub fn export_snapshot_json(
        &self,
        queue: &PlaybackQueue,
        title: Option<String>,
        playback_state: DesktopPlaybackState,
        session_id: impl Into<String>,
        revision: u64,
    ) -> Result<String, NockyConnectError> {
        serde_json::to_string_pretty(&self.export_snapshot(
            queue,
            title,
            playback_state,
            session_id,
            revision,
        ))
        .map_err(|error| NockyConnectError::Json(error.to_string()))
    }

    pub fn decode_snapshot(
        &self,
        payload: &str,
    ) -> Result<PlaybackSessionSnapshot, NockyConnectError> {
        let snapshot = serde_json::from_str::<PlaybackSessionSnapshot>(payload)
            .map_err(|error| NockyConnectError::Json(error.to_string()))?;
        self.require_supported(&snapshot)?;
        Ok(snapshot)
    }

    pub fn prepare_restore(
        &self,
        payload: &str,
    ) -> Result<RestoredDesktopSnapshot, NockyConnectError> {
        let snapshot = self.decode_snapshot(payload)?;
        Ok(restore_desktop_queue_snapshot(&snapshot))
    }

    pub fn require_supported(
        &self,
        snapshot: &PlaybackSessionSnapshot,
    ) -> Result<(), NockyConnectError> {
        if snapshot.schema != PLAYBACK_SESSION_SNAPSHOT_SCHEMA {
            return Err(NockyConnectError::UnsupportedSchema(snapshot.schema.clone()));
        }
        if snapshot.schema_version != NOCKY_CONNECT_PROTOCOL_VERSION {
            return Err(NockyConnectError::UnsupportedSchemaVersion(snapshot.schema_version));
        }
        Ok(())
    }
}

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{connect::NockyRepeatMode, playback::QueueMedia};

    #[test]
    fn exports_and_prepares_restore_from_json() {
        let gateway = NockyConnectGateway::new("desktop-device");
        let mut queue = PlaybackQueue::new();
        queue.replace(
            vec![QueueMedia::youtube(
                "video-1", "First", "Artist", "Album", 180, None,
            )],
            Some(0),
        );

        let payload = gateway
            .export_snapshot_json(
                &queue,
                Some("Gateway queue".to_string()),
                DesktopPlaybackState {
                    position_ms: 12_345,
                    repeat_mode: NockyRepeatMode::All,
                    ..Default::default()
                },
                "gateway-session",
                3,
            )
            .expect("snapshot JSON should encode");
        let restored = gateway.prepare_restore(&payload).expect("snapshot should restore");

        assert_eq!(restored.title.as_deref(), Some("Gateway queue"));
        assert_eq!(restored.queue.len(), 1);
        assert_eq!(restored.state.position_ms, 12_345);
        assert_eq!(restored.state.repeat_mode, NockyRepeatMode::All);
    }

    #[test]
    fn decodes_shared_v1_fixture_and_prepares_paused_restore() {
        let gateway = NockyConnectGateway::new("desktop-device");
        let payload = include_str!("../../docs/fixtures/nocky-connect-snapshot-v1.json");

        let snapshot = gateway.decode_snapshot(payload).expect("fixture should decode");
        let restored = gateway.prepare_restore(payload).expect("fixture should restore");

        assert_eq!(snapshot.schema, PLAYBACK_SESSION_SNAPSHOT_SCHEMA);
        assert_eq!(snapshot.schema_version, NOCKY_CONNECT_PROTOCOL_VERSION);
        assert_eq!(snapshot.session_id, "compat-session-v1");
        assert_eq!(snapshot.revision, 7);
        assert_eq!(snapshot.queue.title.as_deref(), Some("Compatibility fixture"));
        assert_eq!(snapshot.queue.current_index, 1);
        assert_eq!(snapshot.queue.items.len(), 2);
        assert_eq!(restored.title.as_deref(), Some("Compatibility fixture"));
        assert_eq!(restored.queue.current_index(), Some(1));
        assert_eq!(restored.queue.len(), 2);
        assert_eq!(restored.state.position_ms, 45_000);
        assert_eq!(restored.state.repeat_mode, NockyRepeatMode::All);
    }

    #[test]
    fn rejects_unsupported_schema_version() {
        let gateway = NockyConnectGateway::new("desktop-device");
        let payload = r#"{
            "schema":"io.github.maylton.nocky.connect.PlaybackSessionSnapshot",
            "schema_version":99,
            "session_id":"future",
            "revision":1,
            "origin_device_id":"android-device",
            "updated_at_epoch_ms":1,
            "updated_at_monotonic_ms":null,
            "source":"youtube",
            "playback":{"state":"paused","position_ms":0,"duration_ms":null,"rate":1.0,"volume":null,"muted":false},
            "queue":{"title":null,"current_index":0,"repeat_mode":"off","shuffle_enabled":false,"shuffle_seed":null,"items":[]}
        }"#;

        let error = gateway.decode_snapshot(payload).expect_err("version should fail");
        assert_eq!(error, NockyConnectError::UnsupportedSchemaVersion(99));
    }
}

use crate::queue_model::{QueueSource, QueueSourceKind, ShuffleSnapshot};
use serde::{Deserialize, Serialize};
use std::{
    env,
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const LEGACY_SESSION_FILE: &str = "playback-session.json";

impl QueueSourceKind {
    const fn session_file_name(self) -> &'static str {
        match self {
            Self::Local => "playback-session-local.json",
            Self::YouTube => "playback-session-youtube.json",
        }
    }
}
pub const SESSION_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlaybackSession {
    pub version: u32,
    pub source_key: String,
    pub position_us: i64,
    pub was_playing: bool,
    pub shuffle_enabled: bool,
    pub repeat_enabled: bool,
    pub shuffle_state: Option<ShuffleSnapshot>,
    pub shuffle_rng_state: u64,
    pub context_kind: String,
    pub context_id: String,
    pub context_title: String,
    pub saved_at_unix: u64,
}

impl PlaybackSession {
    pub fn new(source: &QueueSource) -> Self {
        Self {
            version: SESSION_SCHEMA_VERSION,
            source_key: source.stable_key(),
            ..Self::default()
        }
    }

    pub fn valid(&self) -> bool {
        self.version == SESSION_SCHEMA_VERSION && !self.source_key.trim().is_empty()
    }
}

pub fn load_for(source: QueueSourceKind) -> Option<PlaybackSession> {
    let source_path = session_path_for(source);
    if source_path.is_file() {
        return load_from_path_for_source(&source_path, source);
    }

    migrate_legacy_session(source, &source_path)
}

pub fn save_for(source: QueueSourceKind, session: &PlaybackSession) -> io::Result<()> {
    validate_session_source(session, source)?;
    save_to_path(&session_path_for(source), session)
}

pub fn clear_for(source: QueueSourceKind) -> io::Result<()> {
    let path = session_path_for(source);
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub fn session_path_for(source: QueueSourceKind) -> PathBuf {
    state_directory().join(source.session_file_name())
}

fn legacy_session_path() -> PathBuf {
    state_directory().join(LEGACY_SESSION_FILE)
}

fn state_directory() -> PathBuf {
    if let Some(state_home) = env::var_os("XDG_STATE_HOME").filter(|value| !value.is_empty()) {
        return PathBuf::from(state_home).join("nocky");
    }

    if let Some(home) = env::var_os("HOME").filter(|value| !value.is_empty()) {
        return PathBuf::from(home)
            .join(".local")
            .join("state")
            .join("nocky");
    }

    env::temp_dir().join("nocky")
}

fn load_from_path_for_source(path: &Path, source: QueueSourceKind) -> Option<PlaybackSession> {
    let session = load_from_path(path)?;
    if validate_session_source(&session, source).is_ok() {
        Some(session)
    } else {
        eprintln!(
            "Saved playback session at {} belongs to a different source",
            path.display()
        );
        quarantine_invalid_state(path);
        None
    }
}

fn load_from_path(path: &Path) -> Option<PlaybackSession> {
    let contents = fs::read_to_string(path).ok()?;
    match serde_json::from_str::<PlaybackSession>(&contents) {
        Ok(session) if session.valid() => Some(session),
        Ok(_) => {
            quarantine_invalid_state(path);
            None
        }
        Err(error) => {
            eprintln!(
                "Could not parse saved playback session at {}: {error}",
                path.display()
            );
            quarantine_invalid_state(path);
            None
        }
    }
}

fn validate_session_source(session: &PlaybackSession, source: QueueSourceKind) -> io::Result<()> {
    let expected_prefix = match source {
        QueueSourceKind::Local => "local:",
        QueueSourceKind::YouTube => "youtube:",
    };

    if session.source_key.starts_with(expected_prefix) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "playback session source '{}' does not match {source:?}",
                session.source_key
            ),
        ))
    }
}

fn migrate_legacy_session(source: QueueSourceKind, source_path: &Path) -> Option<PlaybackSession> {
    let legacy_path = legacy_session_path();
    if !legacy_path.is_file() {
        return None;
    }

    let session = load_from_path(&legacy_path)?;
    if validate_session_source(&session, source).is_err() {
        return None;
    }

    if let Err(error) = save_to_path(source_path, &session) {
        eprintln!(
            "Could not migrate legacy playback session to {}: {error}",
            source_path.display()
        );
        return Some(session);
    }

    Some(session)
}

fn save_to_path(path: &Path, session: &PlaybackSession) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let temporary = path.with_extension("json.tmp");
    let contents =
        serde_json::to_vec_pretty(session).map_err(|error| io::Error::other(error.to_string()))?;

    {
        let mut file = File::create(&temporary)?;
        file.write_all(&contents)?;
        file.sync_all()?;
    }

    fs::rename(temporary, path)
}

fn quarantine_invalid_state(path: &Path) {
    if !path.is_file() {
        return;
    }

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("playback-session");
    let quarantine = path.with_file_name(format!("{stem}.invalid-{stamp}.json"));

    if let Err(error) = fs::rename(path, &quarantine) {
        eprintln!(
            "Could not quarantine invalid playback session {}: {error}",
            path.display()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_session_is_invalid() {
        assert!(!PlaybackSession::default().valid());
    }

    #[test]
    fn source_session_has_stable_key() {
        let source = QueueSource::YouTube {
            video_id: "video-1".to_string(),
        };
        let session = PlaybackSession::new(&source);
        assert!(session.valid());
        assert_eq!(session.source_key, "youtube:video-1");
    }

    #[test]
    fn source_validation_rejects_mismatched_session() {
        let session = PlaybackSession::new(&QueueSource::YouTube {
            video_id: "video-1".to_string(),
        });

        assert!(validate_session_source(&session, QueueSourceKind::YouTube).is_ok());
        assert!(validate_session_source(&session, QueueSourceKind::Local).is_err());
    }

    #[test]
    fn shuffle_state_is_backward_compatible_when_missing() {
        let json = r#"{
            "version": 1,
            "source_key": "youtube:video-1",
            "position_us": 1000000,
            "shuffle_enabled": true
        }"#;

        let session: PlaybackSession = serde_json::from_str(json).expect("parse old session");
        assert!(session.valid());
        assert!(session.shuffle_state.is_none());
        assert_eq!(session.shuffle_rng_state, 0);
    }

    #[test]
    fn source_session_files_are_distinct() {
        assert_eq!(
            QueueSourceKind::Local.session_file_name(),
            "playback-session-local.json"
        );
        assert_eq!(
            QueueSourceKind::YouTube.session_file_name(),
            "playback-session-youtube.json"
        );
    }
}

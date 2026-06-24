use crate::queue_model::QueueSource;
use serde::{Deserialize, Serialize};
use std::{
    env,
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const SESSION_FILE: &str = "playback-session.json";
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

pub fn load() -> Option<PlaybackSession> {
    let path = session_path();
    let contents = fs::read_to_string(&path).ok()?;
    match serde_json::from_str::<PlaybackSession>(&contents) {
        Ok(session) if session.valid() => Some(session),
        Ok(_) => {
            quarantine_invalid_state(&path);
            None
        }
        Err(error) => {
            eprintln!(
                "Could not parse saved playback session at {}: {error}",
                path.display()
            );
            quarantine_invalid_state(&path);
            None
        }
    }
}

pub fn save(session: &PlaybackSession) -> io::Result<()> {
    save_to_path(&session_path(), session)
}

pub fn clear() -> io::Result<()> {
    let path = session_path();
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub fn session_path() -> PathBuf {
    if let Some(state_home) = env::var_os("XDG_STATE_HOME").filter(|value| !value.is_empty()) {
        return PathBuf::from(state_home).join("nocky").join(SESSION_FILE);
    }

    if let Some(home) = env::var_os("HOME").filter(|value| !value.is_empty()) {
        return PathBuf::from(home)
            .join(".local")
            .join("state")
            .join("nocky")
            .join(SESSION_FILE);
    }

    env::temp_dir().join("nocky").join(SESSION_FILE)
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
    let quarantine = path.with_file_name(format!("playback-session.invalid-{stamp}.json"));

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
}

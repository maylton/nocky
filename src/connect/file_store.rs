use std::{
    fs, io,
    path::{Path, PathBuf},
};

use super::PlaybackSessionSnapshot;

#[derive(Clone, Debug)]
pub struct NockyConnectFileStore {
    directory: PathBuf,
}

impl NockyConnectFileStore {
    pub fn new(base_dir: impl AsRef<Path>) -> Self {
        Self {
            directory: base_dir.as_ref().join("nocky-connect"),
        }
    }

    pub fn write_snapshot(&self, snapshot: &PlaybackSessionSnapshot) -> io::Result<PathBuf> {
        let payload = serde_json::to_string_pretty(snapshot)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        self.write_snapshot_json(&snapshot.session_id, snapshot.revision, &payload)
    }

    pub fn write_snapshot_json(
        &self,
        session_id: &str,
        revision: u64,
        payload: &str,
    ) -> io::Result<PathBuf> {
        fs::create_dir_all(&self.directory)?;
        let path = self.directory.join(file_name_for(session_id, revision));
        fs::write(&path, payload)?;
        Ok(path)
    }

    pub fn read_snapshot(&self, path: impl AsRef<Path>) -> io::Result<PlaybackSessionSnapshot> {
        let payload = fs::read_to_string(path)?;
        serde_json::from_str(&payload).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }

    pub fn latest_snapshot_file(&self) -> io::Result<Option<PathBuf>> {
        let files = self.list_snapshot_files()?;
        Ok(files.into_iter().next())
    }

    pub fn list_snapshot_files(&self) -> io::Result<Vec<PathBuf>> {
        if !self.directory.exists() {
            return Ok(Vec::new());
        }

        let mut files = fs::read_dir(&self.directory)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.is_file() && path.extension().is_some_and(|extension| extension == "json"))
            .collect::<Vec<_>>();

        files.sort_by_key(|path| {
            fs::metadata(path)
                .and_then(|metadata| metadata.modified())
                .ok()
        });
        files.reverse();
        Ok(files)
    }
}

fn file_name_for(session_id: &str, revision: u64) -> String {
    let safe_session_id = session_id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    let safe_session_id = if safe_session_id.trim().is_empty() {
        "session".to_string()
    } else {
        safe_session_id.chars().take(80).collect()
    };
    format!("snapshot_{safe_session_id}_r{revision}.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connect::{
        NockyConnectSource, NockyPlaybackState, NockyRepeatMode, PlaybackInfo,
        PlaybackSessionSnapshot, PortableQueue,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn writes_reads_and_lists_snapshots() {
        let root = temp_root();
        let store = NockyConnectFileStore::new(&root);
        let snapshot = snapshot("session/with spaces", 5);

        let path = store.write_snapshot(&snapshot).expect("write snapshot");
        let decoded = store.read_snapshot(&path).expect("read snapshot");
        let files = store.list_snapshot_files().expect("list snapshots");

        assert!(path.exists());
        assert!(path.file_name().unwrap().to_string_lossy().starts_with("snapshot_session_with_spaces_r5"));
        assert_eq!(decoded, snapshot);
        assert_eq!(files, vec![path.clone()]);
        assert_eq!(store.latest_snapshot_file().unwrap(), Some(path));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn empty_store_lists_no_snapshots() {
        let root = temp_root();
        let store = NockyConnectFileStore::new(&root);

        assert!(store.list_snapshot_files().unwrap().is_empty());
        assert_eq!(store.latest_snapshot_file().unwrap(), None);

        let _ = fs::remove_dir_all(root);
    }

    fn snapshot(session_id: &str, revision: u64) -> PlaybackSessionSnapshot {
        PlaybackSessionSnapshot::new(
            session_id,
            revision,
            "desktop-device",
            1_700_000_000_000,
            NockyConnectSource::YouTube,
            PlaybackInfo {
                state: NockyPlaybackState::Paused,
                position_ms: 1_000,
                duration_ms: None,
                rate: 1.0,
                volume: None,
                muted: false,
            },
            PortableQueue {
                title: None,
                current_index: 0,
                repeat_mode: NockyRepeatMode::Off,
                shuffle_enabled: false,
                shuffle_seed: None,
                items: Vec::new(),
            },
        )
    }

    fn temp_root() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("nocky-connect-store-test-{suffix}"))
    }
}

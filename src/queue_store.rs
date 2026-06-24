use crate::queue_model::{PlaybackQueue, QueueSnapshot, QueueSource};
use std::{
    env,
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const QUEUE_STATE_FILE: &str = "queue.json";

#[derive(Debug)]
pub struct QueueLoadResult {
    pub queue: PlaybackQueue,
    pub discarded_entries: usize,
}

pub fn load() -> QueueLoadResult {
    load_from_path(&queue_state_path())
}

pub fn save(snapshot: &QueueSnapshot) -> io::Result<()> {
    save_to_path(&queue_state_path(), snapshot)
}

pub fn queue_state_path() -> PathBuf {
    if let Some(state_home) = env::var_os("XDG_STATE_HOME").filter(|value| !value.is_empty()) {
        return PathBuf::from(state_home)
            .join("nocky")
            .join(QUEUE_STATE_FILE);
    }

    if let Some(home) = env::var_os("HOME").filter(|value| !value.is_empty()) {
        return PathBuf::from(home)
            .join(".local")
            .join("state")
            .join("nocky")
            .join(QUEUE_STATE_FILE);
    }

    env::temp_dir().join("nocky").join(QUEUE_STATE_FILE)
}

fn load_from_path(path: &Path) -> QueueLoadResult {
    let Ok(contents) = fs::read_to_string(path) else {
        return QueueLoadResult {
            queue: PlaybackQueue::new(),
            discarded_entries: 0,
        };
    };

    let mut snapshot = match serde_json::from_str::<QueueSnapshot>(&contents) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            eprintln!(
                "Could not parse saved Queue 2.0 state at {}: {error}",
                path.display()
            );
            quarantine_invalid_state(path);
            return QueueLoadResult {
                queue: PlaybackQueue::new(),
                discarded_entries: 0,
            };
        }
    };

    let original_len = snapshot.entries.len();
    snapshot.entries.retain(|entry| match &entry.media.source {
        QueueSource::Local { path } => path.is_file(),
        QueueSource::YouTube { video_id } => !video_id.trim().is_empty(),
    });
    let discarded_entries = original_len.saturating_sub(snapshot.entries.len());

    if snapshot
        .current_id
        .is_some_and(|current_id| !snapshot.entries.iter().any(|entry| entry.id == current_id))
    {
        snapshot.current_id = None;
    }

    let queue = match PlaybackQueue::restore(snapshot) {
        Ok(queue) => queue,
        Err(error) => {
            eprintln!(
                "Could not restore saved Queue 2.0 state at {}: {error}",
                path.display()
            );
            quarantine_invalid_state(path);
            return QueueLoadResult {
                queue: PlaybackQueue::new(),
                discarded_entries,
            };
        }
    };

    if discarded_entries > 0 {
        if let Err(error) = save_to_path(path, &queue.snapshot()) {
            eprintln!(
                "Could not persist cleaned Queue 2.0 state at {}: {error}",
                path.display()
            );
        }
    }

    QueueLoadResult {
        queue,
        discarded_entries,
    }
}

fn save_to_path(path: &Path, snapshot: &QueueSnapshot) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let temporary = path.with_extension("json.tmp");
    let contents =
        serde_json::to_vec_pretty(snapshot).map_err(|error| io::Error::other(error.to_string()))?;

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
    let quarantine = path.with_file_name(format!("queue.invalid-{stamp}.json"));

    if let Err(error) = fs::rename(path, &quarantine) {
        eprintln!(
            "Could not quarantine invalid Queue 2.0 state {}: {error}",
            path.display()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue_model::QueueMedia;
    use std::{
        env, fs, process,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temporary_directory(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let path = env::temp_dir().join(format!(
            "nocky-queue-store-{label}-{}-{nonce}",
            process::id()
        ));
        fs::create_dir_all(&path).expect("create temporary queue directory");
        path
    }

    #[test]
    fn round_trip_preserves_order_ids_and_current_entry() {
        let directory = temporary_directory("round-trip");
        let path = directory.join("queue.json");

        let local_path = directory.join("song.ogg");
        fs::write(&local_path, b"test").expect("create local media fixture");

        let mut queue = PlaybackQueue::new();
        let local_id = queue.append(QueueMedia::local(
            local_path,
            "Local song",
            "Artist",
            "Album",
            180,
            None,
        ));
        queue.append(QueueMedia::youtube(
            "video-123",
            "Online song",
            "Artist",
            "Album",
            200,
            None,
        ));
        queue.select(local_id).expect("select local entry");

        save_to_path(&path, &queue.snapshot()).expect("save queue");
        let restored = load_from_path(&path);

        assert_eq!(restored.discarded_entries, 0);
        assert_eq!(restored.queue.snapshot(), queue.snapshot());

        fs::remove_dir_all(directory).expect("remove temporary queue directory");
    }

    #[test]
    fn missing_local_entries_are_discarded_and_current_is_cleared() {
        let directory = temporary_directory("missing-local");
        let path = directory.join("queue.json");

        let mut queue = PlaybackQueue::new();
        let missing_id = queue.append(QueueMedia::local(
            directory.join("missing.flac"),
            "Missing",
            "Artist",
            "Album",
            120,
            None,
        ));
        queue.append(QueueMedia::youtube(
            "video-valid",
            "Valid",
            "Artist",
            "Album",
            150,
            None,
        ));
        queue
            .select(missing_id)
            .expect("select missing local entry");

        save_to_path(&path, &queue.snapshot()).expect("save queue");
        let restored = load_from_path(&path);

        assert_eq!(restored.discarded_entries, 1);
        assert_eq!(restored.queue.len(), 1);
        assert_eq!(restored.queue.current_id(), None);
        assert!(matches!(
            &restored.queue.entries()[0].media.source,
            QueueSource::YouTube { .. }
        ));

        fs::remove_dir_all(directory).expect("remove temporary queue directory");
    }

    #[test]
    fn invalid_youtube_entries_are_discarded() {
        let directory = temporary_directory("invalid-youtube");
        let path = directory.join("queue.json");

        let mut queue = PlaybackQueue::new();
        queue.append(QueueMedia::new(
            QueueSource::YouTube {
                video_id: "   ".to_string(),
            },
            "Invalid",
            "Artist",
            "Album",
            0,
            None,
        ));

        save_to_path(&path, &queue.snapshot()).expect("save queue");
        let restored = load_from_path(&path);

        assert_eq!(restored.discarded_entries, 1);
        assert!(restored.queue.is_empty());

        fs::remove_dir_all(directory).expect("remove temporary queue directory");
    }

    #[test]
    fn corrupt_state_is_quarantined_instead_of_crashing() {
        let directory = temporary_directory("corrupt");
        let path = directory.join("queue.json");
        fs::write(&path, b"{not-json").expect("write corrupt state");

        let restored = load_from_path(&path);

        assert!(restored.queue.is_empty());
        assert!(!path.exists());
        assert!(fs::read_dir(&directory)
            .expect("read temporary queue directory")
            .flatten()
            .any(|entry| entry
                .file_name()
                .to_string_lossy()
                .starts_with("queue.invalid-")));

        fs::remove_dir_all(directory).expect("remove temporary queue directory");
    }
}

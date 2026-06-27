use crate::youtube::credited_artists;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::{mpsc, Mutex, OnceLock},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_EVENTS: usize = 20_000;
// Disk serialization and atomic writes are kept away from GTK's main loop.
static HISTORY_WRITER: OnceLock<mpsc::Sender<StoredHistory>> = OnceLock::new();
static HISTORY_WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ListeningSource {
    Local,
    YouTube,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayEvent {
    pub track_id: String,
    #[serde(default)]
    pub media_id: String,
    #[serde(default)]
    pub title: String,
    pub artist: String,
    pub album: String,
    pub source: ListeningSource,
    pub played_at: i64,
    pub listened_seconds: u64,
    #[serde(default)]
    pub position_seconds: u64,
    #[serde(default)]
    pub duration_seconds: u64,
    #[serde(default)]
    pub context_kind: String,
    #[serde(default)]
    pub context_id: String,
    #[serde(default)]
    pub context_title: String,
    pub completed: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ListeningStats {
    pub play_count: u64,
    pub last_played_at: i64,
    pub total_listened_seconds: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryTrack {
    pub media_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub source: ListeningSource,
    pub played_at: i64,
    pub position_seconds: u64,
    pub duration_seconds: u64,
    pub completed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryCollection {
    pub kind: String,
    pub id: String,
    pub title: String,
    pub source: ListeningSource,
    pub played_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HistoryActivity {
    Track(HistoryTrack),
    Collection(HistoryCollection),
}

#[derive(Clone, Debug, Default)]
pub struct PlaybackHistoryContext {
    pub kind: String,
    pub id: String,
    pub title: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct StoredHistory {
    #[serde(default)]
    events: Vec<PlayEvent>,
}

#[derive(Clone, Debug)]
pub struct ListeningHistory {
    events: Vec<PlayEvent>,
    recording_enabled: bool,
}

impl Default for ListeningHistory {
    fn default() -> Self {
        Self {
            events: Vec::new(),
            recording_enabled: true,
        }
    }
}

impl ListeningHistory {
    pub fn load() -> Self {
        let Ok(raw) = fs::read_to_string(history_path()) else {
            return Self::default();
        };
        let Ok(stored) = serde_json::from_str::<StoredHistory>(&raw) else {
            return Self::default();
        };
        Self {
            events: stored.events,
            recording_enabled: true,
        }
    }

    #[cfg(test)]
    pub fn record_progress(
        &mut self,
        session_id: String,
        artist: String,
        album: String,
        source: ListeningSource,
        listened_seconds: u64,
        completed: bool,
    ) -> bool {
        self.record_playback_progress(
            session_id,
            String::new(),
            String::new(),
            artist,
            album,
            source,
            listened_seconds,
            listened_seconds,
            0,
            PlaybackHistoryContext::default(),
            completed,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_playback_progress(
        &mut self,
        session_id: String,
        media_id: String,
        title: String,
        artist: String,
        album: String,
        source: ListeningSource,
        listened_seconds: u64,
        position_seconds: u64,
        duration_seconds: u64,
        context: PlaybackHistoryContext,
        completed: bool,
    ) -> bool {
        if !self.recording_enabled {
            return false;
        }

        if listened_seconds < 30 && !completed {
            return false;
        }

        let now = now_unix();
        if let Some(event) = self
            .events
            .iter_mut()
            .rev()
            .find(|event| event.track_id == session_id && event.source == source)
        {
            let next_seconds = event.listened_seconds.max(listened_seconds);
            let next_duration = event.duration_seconds.max(duration_seconds);
            let next_completed = event.completed || completed;
            let changed = next_seconds != event.listened_seconds
                || position_seconds != event.position_seconds
                || next_duration != event.duration_seconds
                || next_completed != event.completed
                || event.media_id != media_id
                || event.title != title
                || event.artist != artist
                || event.album != album
                || event.context_kind != context.kind
                || event.context_id != context.id
                || event.context_title != context.title;
            if !changed {
                return false;
            }

            event.media_id = media_id;
            event.title = title;
            event.artist = artist;
            event.album = album;
            event.listened_seconds = next_seconds;
            event.position_seconds = position_seconds;
            event.duration_seconds = next_duration;
            event.context_kind = context.kind;
            event.context_id = context.id;
            event.context_title = context.title;
            event.completed = next_completed;
            event.played_at = now;
        } else {
            self.events.push(PlayEvent {
                track_id: session_id,
                media_id,
                title,
                artist,
                album,
                source,
                played_at: now,
                listened_seconds,
                position_seconds,
                duration_seconds,
                context_kind: context.kind,
                context_id: context.id,
                context_title: context.title,
                completed,
            });
        }

        if self.events.len() > MAX_EVENTS {
            self.events.drain(..self.events.len() - MAX_EVENTS);
        }
        self.save();
        true
    }

    pub fn set_recording_enabled(&mut self, enabled: bool) {
        self.recording_enabled = enabled;
    }

    pub fn clear(&mut self) -> bool {
        if self.events.is_empty() {
            return false;
        }
        self.events.clear();
        self.save();
        true
    }

    pub fn ranked_artists(
        &self,
        source: ListeningSource,
        limit: usize,
    ) -> Vec<(String, ListeningStats)> {
        rank_artist_credits(
            self.events.iter().filter(|event| event.source == source),
            limit,
        )
    }

    pub fn ranked_albums(
        &self,
        source: ListeningSource,
        limit: usize,
    ) -> Vec<(String, ListeningStats)> {
        rank(
            self.events.iter().filter(|event| event.source == source),
            |event| event.album.trim(),
            limit,
        )
    }

    pub fn recent_albums(&self, source: ListeningSource, limit: usize) -> Vec<String> {
        let mut events = self
            .events
            .iter()
            .filter(|event| event.source == source && !event.album.trim().is_empty())
            .collect::<Vec<_>>();
        events.sort_by_key(|event| std::cmp::Reverse(event.played_at));
        let mut albums = Vec::new();
        for event in events {
            if !albums
                .iter()
                .any(|album: &String| album.eq_ignore_ascii_case(&event.album))
            {
                albums.push(event.album.clone());
            }
            if albums.len() == limit {
                break;
            }
        }
        albums
    }

    pub fn recent_activity(&self, source: ListeningSource, limit: usize) -> Vec<HistoryActivity> {
        let mut events = self
            .events
            .iter()
            .filter(|event| {
                event.source == source
                    && !event.media_id.trim().is_empty()
                    && !event.title.trim().is_empty()
            })
            .collect::<Vec<_>>();
        events.sort_by_key(|event| std::cmp::Reverse(event.played_at));

        let mut seen = std::collections::HashSet::new();
        let mut activity = Vec::new();

        for event in events {
            let kind = event.context_kind.trim();
            let id = event.context_id.trim();
            let title = event.context_title.trim();

            if matches!(kind, "album" | "playlist") && !id.is_empty() && !title.is_empty() {
                let identity = format!("{:?}:{kind}:{}", event.source, id.to_lowercase());
                if !seen.insert(identity) {
                    continue;
                }

                activity.push(HistoryActivity::Collection(HistoryCollection {
                    kind: kind.to_string(),
                    id: id.to_string(),
                    title: title.to_string(),
                    source: event.source,
                    played_at: event.played_at,
                }));
            } else {
                let identity =
                    format!("{:?}:track:{}", event.source, event.media_id.to_lowercase());
                if !seen.insert(identity) {
                    continue;
                }

                activity.push(HistoryActivity::Track(HistoryTrack {
                    media_id: event.media_id.clone(),
                    title: event.title.clone(),
                    artist: event.artist.clone(),
                    album: event.album.clone(),
                    source: event.source,
                    played_at: event.played_at,
                    position_seconds: event.position_seconds,
                    duration_seconds: event.duration_seconds,
                    completed: event.completed,
                }));
            }

            if activity.len() == limit {
                break;
            }
        }

        activity
    }

    #[cfg(test)]
    pub fn recent_tracks(&self, source: ListeningSource, limit: usize) -> Vec<HistoryTrack> {
        self.track_history(source, limit, false)
    }

    #[cfg(test)]
    pub fn continue_listening(&self, source: ListeningSource, limit: usize) -> Vec<HistoryTrack> {
        self.track_history(source, limit, true)
    }

    #[cfg(test)]
    fn track_history(
        &self,
        source: ListeningSource,
        limit: usize,
        resumable_only: bool,
    ) -> Vec<HistoryTrack> {
        let mut events = self
            .events
            .iter()
            .filter(|event| {
                event.source == source
                    && !event.media_id.trim().is_empty()
                    && !event.title.trim().is_empty()
            })
            .collect::<Vec<_>>();
        events.sort_by_key(|event| std::cmp::Reverse(event.played_at));

        let mut seen = std::collections::HashSet::new();
        let mut tracks = Vec::new();

        for event in events {
            if !seen.insert(event.media_id.to_lowercase()) {
                continue;
            }

            if resumable_only {
                if event.completed || event.duration_seconds == 0 || event.position_seconds < 30 {
                    continue;
                }

                let progress = event.position_seconds as f64 / event.duration_seconds as f64;
                if !(0.05..=0.90).contains(&progress) {
                    continue;
                }
            }

            tracks.push(HistoryTrack {
                media_id: event.media_id.clone(),
                title: event.title.clone(),
                artist: event.artist.clone(),
                album: event.album.clone(),
                source: event.source,
                played_at: event.played_at,
                position_seconds: event.position_seconds,
                duration_seconds: event.duration_seconds,
                completed: event.completed,
            });

            if tracks.len() == limit {
                break;
            }
        }

        tracks
    }

    fn save(&self) {
        let stored = StoredHistory {
            events: self.events.clone(),
        };

        // Sending a snapshot is cheap. JSON serialization and filesystem I/O
        // happen in the dedicated writer thread.
        if let Err(error) = history_writer().send(stored) {
            // Thread creation can fail on a severely constrained system.
            // Preserve correctness with a synchronous fallback.
            write_history_snapshot(&error.0);
        }
    }

    pub fn flush(&self) {
        write_history_snapshot(&StoredHistory {
            events: self.events.clone(),
        });
    }
}

fn history_writer() -> &'static mpsc::Sender<StoredHistory> {
    HISTORY_WRITER.get_or_init(|| {
        let (sender, receiver) = mpsc::channel::<StoredHistory>();

        if let Err(error) = thread::Builder::new()
            .name("nocky-history-writer".to_string())
            .spawn(move || {
                while let Ok(mut snapshot) = receiver.recv() {
                    // Coalesce checkpoints that accumulated while the previous
                    // snapshot was being written. The newest snapshot already
                    // contains every earlier event.
                    while let Ok(newer) = receiver.try_recv() {
                        snapshot = newer;
                    }
                    write_history_snapshot(&snapshot);
                }
            })
        {
            eprintln!("Could not start Nocky history writer: {error}");
        }

        sender
    })
}

fn write_history_snapshot(stored: &StoredHistory) {
    let lock = HISTORY_WRITE_LOCK.get_or_init(|| Mutex::new(()));
    let Ok(_guard) = lock.lock() else {
        return;
    };

    let path = history_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let Ok(json) = serde_json::to_string_pretty(stored) else {
        return;
    };

    let temporary = path.with_extension("json.tmp");
    if fs::write(&temporary, json).is_ok() {
        let _ = fs::rename(temporary, path);
    }
}

fn rank_artist_credits<'a, I>(events: I, limit: usize) -> Vec<(String, ListeningStats)>
where
    I: Iterator<Item = &'a PlayEvent>,
{
    let mut grouped = HashMap::<String, (String, ListeningStats)>::new();

    for event in events {
        for artist in credited_artists(&event.artist) {
            let normalized = artist.to_lowercase();
            let (_, stats) = grouped
                .entry(normalized)
                .or_insert_with(|| (artist, ListeningStats::default()));
            stats.play_count += 1;
            stats.last_played_at = stats.last_played_at.max(event.played_at);
            stats.total_listened_seconds += event.listened_seconds;
        }
    }

    let mut ranked = grouped.into_values().collect::<Vec<_>>();
    ranked.sort_by(|(left_name, left), (right_name, right)| {
        right
            .total_listened_seconds
            .cmp(&left.total_listened_seconds)
            .then_with(|| right.play_count.cmp(&left.play_count))
            .then_with(|| right.last_played_at.cmp(&left.last_played_at))
            .then_with(|| left_name.to_lowercase().cmp(&right_name.to_lowercase()))
    });
    ranked.truncate(limit);
    ranked
}

fn rank<'a, I, F>(events: I, key: F, limit: usize) -> Vec<(String, ListeningStats)>
where
    I: Iterator<Item = &'a PlayEvent>,
    F: Fn(&PlayEvent) -> &str,
{
    let mut grouped = HashMap::<String, (String, ListeningStats)>::new();
    for event in events {
        let name = key(event).trim();
        if name.is_empty() {
            continue;
        }

        let normalized = name.to_lowercase();
        let (_, stats) = grouped
            .entry(normalized)
            .or_insert_with(|| (name.to_string(), ListeningStats::default()));
        stats.play_count += 1;
        stats.last_played_at = stats.last_played_at.max(event.played_at);
        stats.total_listened_seconds += event.listened_seconds;
    }

    let mut ranked = grouped.into_values().collect::<Vec<_>>();
    ranked.sort_by(|(left_name, left), (right_name, right)| {
        right
            .total_listened_seconds
            .cmp(&left.total_listened_seconds)
            .then_with(|| right.play_count.cmp(&left.play_count))
            .then_with(|| right.last_played_at.cmp(&left.last_played_at))
            .then_with(|| left_name.to_lowercase().cmp(&right_name.to_lowercase()))
    });
    ranked.truncate(limit);
    ranked
}

fn history_path() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("nocky").join("listening-history.json")
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().min(i64::MAX as u64) as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(
        artist: &str,
        album: &str,
        source: ListeningSource,
        played_at: i64,
        listened_seconds: u64,
    ) -> PlayEvent {
        PlayEvent {
            track_id: format!("{artist}:{album}:{played_at}"),
            artist: artist.to_string(),
            album: album.to_string(),
            source,
            played_at,
            listened_seconds,
            media_id: String::new(),
            title: String::new(),
            position_seconds: listened_seconds,
            duration_seconds: 0,
            context_kind: String::new(),
            context_id: String::new(),
            context_title: String::new(),
            completed: false,
        }
    }

    #[test]
    fn ranking_prefers_total_listening_time() {
        let history = ListeningHistory {
            recording_enabled: true,
            events: vec![
                event("Long Listen", "Album A", ListeningSource::Local, 10, 600),
                event("Recent Plays", "Album B", ListeningSource::Local, 20, 45),
                event("Recent Plays", "Album B", ListeningSource::Local, 21, 45),
                event("Recent Plays", "Album B", ListeningSource::Local, 22, 45),
            ],
        };

        let artists = history.ranked_artists(ListeningSource::Local, 10);
        assert_eq!(artists[0].0, "Long Listen");
        assert_eq!(artists[0].1.total_listened_seconds, 600);
    }

    #[test]
    fn artist_ranking_splits_collaboration_credits() {
        let history = ListeningHistory {
            recording_enabled: true,
            events: vec![event(
                "Anitta feat. Felipe Amorim, HITMAKER",
                "Collaboration",
                ListeningSource::YouTube,
                10,
                180,
            )],
        };

        let artists = history.ranked_artists(ListeningSource::YouTube, 10);
        let names = artists
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(names, vec!["Anitta", "Felipe Amorim", "HITMAKER"]);
        assert!(artists
            .iter()
            .all(|(_, stats)| stats.play_count == 1 && stats.total_listened_seconds == 180));
    }

    #[test]
    fn ranking_merges_case_variants() {
        let history = ListeningHistory {
            recording_enabled: true,
            events: vec![
                event("Björk", "Homogenic", ListeningSource::Local, 10, 120),
                event("BJÖRK", "homogenic", ListeningSource::Local, 11, 180),
            ],
        };

        let artists = history.ranked_artists(ListeningSource::Local, 10);
        let albums = history.ranked_albums(ListeningSource::Local, 10);

        assert_eq!(artists.len(), 1);
        assert_eq!(artists[0].1.play_count, 2);
        assert_eq!(artists[0].1.total_listened_seconds, 300);
        assert_eq!(albums.len(), 1);
        assert_eq!(albums[0].1.play_count, 2);
    }

    #[test]
    fn ranking_keeps_sources_separate() {
        let history = ListeningHistory {
            recording_enabled: true,
            events: vec![
                event(
                    "Local Artist",
                    "Local Album",
                    ListeningSource::Local,
                    10,
                    120,
                ),
                event(
                    "YouTube Artist",
                    "YouTube Album",
                    ListeningSource::YouTube,
                    11,
                    500,
                ),
            ],
        };

        let local = history.ranked_artists(ListeningSource::Local, 10);
        assert_eq!(local.len(), 1);
        assert_eq!(local[0].0, "Local Artist");
    }
}

#[cfg(test)]
mod personalized_home_resume_tests {
    use super::*;

    fn rich_event(
        media_id: &str,
        played_at: i64,
        position_seconds: u64,
        duration_seconds: u64,
        completed: bool,
    ) -> PlayEvent {
        PlayEvent {
            track_id: format!("session:{media_id}:{played_at}"),
            media_id: media_id.to_string(),
            title: format!("Track {media_id}"),
            artist: "Artist".to_string(),
            album: "Album".to_string(),
            source: ListeningSource::Local,
            played_at,
            listened_seconds: position_seconds,
            position_seconds,
            duration_seconds,
            context_kind: String::new(),
            context_id: String::new(),
            context_title: String::new(),
            completed,
        }
    }

    #[test]
    fn recent_tracks_deduplicate_by_stable_media_id() {
        let history = ListeningHistory {
            recording_enabled: true,
            events: vec![
                rich_event("a", 10, 40, 200, false),
                rich_event("b", 20, 50, 200, false),
                rich_event("a", 30, 80, 200, false),
            ],
        };

        let recent = history.recent_tracks(ListeningSource::Local, 10);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].media_id, "a");
        assert_eq!(recent[0].position_seconds, 80);
        assert_eq!(recent[1].media_id, "b");
    }

    #[test]
    fn continue_listening_filters_invalid_progress() {
        let history = ListeningHistory {
            recording_enabled: true,
            events: vec![
                rich_event("too-early", 10, 5, 200, false),
                rich_event("resume", 20, 80, 200, false),
                rich_event("almost-done", 30, 195, 200, false),
                rich_event("completed", 40, 100, 200, true),
            ],
        };

        let resumable = history.continue_listening(ListeningSource::Local, 10);
        assert_eq!(resumable.len(), 1);
        assert_eq!(resumable[0].media_id, "resume");
    }
}

#[cfg(test)]
mod session_progress_tests {
    use super::*;

    #[test]
    fn updates_the_same_playback_session() {
        let mut history = ListeningHistory::default();

        assert!(history.record_progress(
            "youtube:abc:session-1".to_string(),
            "Artist".to_string(),
            "Album".to_string(),
            ListeningSource::YouTube,
            30,
            false,
        ));
        assert!(history.record_progress(
            "youtube:abc:session-1".to_string(),
            "Artist".to_string(),
            "Album".to_string(),
            ListeningSource::YouTube,
            180,
            true,
        ));

        let ranked = history.ranked_artists(ListeningSource::YouTube, 10);
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].1.play_count, 1);
        assert_eq!(ranked[0].1.total_listened_seconds, 180);
    }

    #[test]
    fn separate_sessions_count_as_separate_plays() {
        let mut history = ListeningHistory::default();

        for session in ["session-1", "session-2"] {
            assert!(history.record_progress(
                session.to_string(),
                "Artist".to_string(),
                "Album".to_string(),
                ListeningSource::Local,
                45,
                false,
            ));
        }

        let ranked = history.ranked_artists(ListeningSource::Local, 10);
        assert_eq!(ranked[0].1.play_count, 2);
        assert_eq!(ranked[0].1.total_listened_seconds, 90);
    }
}

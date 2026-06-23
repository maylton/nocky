use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_EVENTS: usize = 20_000;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ListeningSource {
    Local,
    YouTube,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayEvent {
    pub track_id: String,
    pub artist: String,
    pub album: String,
    pub source: ListeningSource,
    pub played_at: i64,
    pub listened_seconds: u64,
    pub completed: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ListeningStats {
    pub play_count: u64,
    pub last_played_at: i64,
    pub total_listened_seconds: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct StoredHistory {
    #[serde(default)]
    events: Vec<PlayEvent>,
}

#[derive(Clone, Debug, Default)]
pub struct ListeningHistory {
    events: Vec<PlayEvent>,
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
        }
    }

    pub fn record_progress(
        &mut self,
        session_id: String,
        artist: String,
        album: String,
        source: ListeningSource,
        listened_seconds: u64,
        completed: bool,
    ) -> bool {
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
            let next_completed = event.completed || completed;
            let changed =
                next_seconds != event.listened_seconds || next_completed != event.completed;
            if !changed {
                return false;
            }

            event.artist = artist;
            event.album = album;
            event.listened_seconds = next_seconds;
            event.completed = next_completed;
            event.played_at = now;
        } else {
            self.events.push(PlayEvent {
                track_id: session_id,
                artist,
                album,
                source,
                played_at: now,
                listened_seconds,
                completed,
            });
        }

        if self.events.len() > MAX_EVENTS {
            self.events.drain(..self.events.len() - MAX_EVENTS);
        }
        self.save();
        true
    }

    pub fn ranked_artists(
        &self,
        source: ListeningSource,
        limit: usize,
    ) -> Vec<(String, ListeningStats)> {
        rank(
            self.events.iter().filter(|event| event.source == source),
            |event| event.artist.trim(),
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

    fn save(&self) {
        let path = history_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let stored = StoredHistory {
            events: self.events.clone(),
        };
        if let Ok(json) = serde_json::to_string_pretty(&stored) {
            let temporary = path.with_extension("json.tmp");
            if fs::write(&temporary, json).is_ok() {
                let _ = fs::rename(temporary, path);
            }
        }
    }
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
            completed: false,
        }
    }

    #[test]
    fn ranking_prefers_total_listening_time() {
        let history = ListeningHistory {
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
    fn ranking_merges_case_variants() {
        let history = ListeningHistory {
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

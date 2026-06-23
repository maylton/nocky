use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    collections::HashMap,
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_EVENTS: usize = 20_000;
const RECENT_WINDOW_SECONDS: i64 = 30 * 24 * 60 * 60;

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

    pub fn record(
        &mut self,
        track_id: String,
        artist: String,
        album: String,
        source: ListeningSource,
        listened_seconds: u64,
        completed: bool,
    ) -> bool {
        if listened_seconds < 30 && !completed {
            return false;
        }
        self.events.push(PlayEvent {
            track_id,
            artist,
            album,
            source,
            played_at: now_unix(),
            listened_seconds,
            completed,
        });
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
    let now = now_unix();
    let mut grouped = HashMap::<String, ListeningStats>::new();
    for event in events {
        let name = key(event);
        if name.is_empty() {
            continue;
        }
        let stats = grouped.entry(name.to_string()).or_default();
        stats.play_count += 1;
        stats.last_played_at = stats.last_played_at.max(event.played_at);
        stats.total_listened_seconds += event.listened_seconds;
    }
    let mut ranked = grouped.into_iter().collect::<Vec<_>>();
    ranked.sort_by(|(_, left), (_, right)| {
        score(right, now)
            .partial_cmp(&score(left, now))
            .unwrap_or(Ordering::Equal)
            .then_with(|| right.last_played_at.cmp(&left.last_played_at))
    });
    ranked.truncate(limit);
    ranked
}

fn score(stats: &ListeningStats, now: i64) -> f64 {
    let age = (now - stats.last_played_at).max(0) as f64;
    let recency = if age <= RECENT_WINDOW_SECONDS as f64 {
        1.0 - age / RECENT_WINDOW_SECONDS as f64
    } else {
        0.0
    };
    stats.play_count as f64
        + (stats.total_listened_seconds as f64 / 60.0).sqrt() * 0.35
        + recency * 3.0
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

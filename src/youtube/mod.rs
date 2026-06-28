mod backend;
mod collections;
pub(crate) mod diagnostics;
pub(crate) mod error;
mod feed;
mod playback;
mod routing;

use crate::search_text::{normalize_search_text, search_matches, search_score};
use crate::ui::widgets::ExpressiveLoadingIndicator;
use gtk::glib;
use gtk::prelude::*;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use std::{
    cell::RefCell,
    collections::{hash_map::DefaultHasher, BTreeMap, BTreeSet, HashMap, HashSet},
    env, fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    rc::Rc,
    sync::{
        atomic::{AtomicU8, Ordering as AtomicOrdering},
        mpsc::{self, Receiver, RecvTimeoutError, Sender},
        Mutex, OnceLock,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub(crate) use collections::{resolve_youtube_collection_item, youtube_home_prefetch_candidates};
pub(crate) use feed::YouTubeHomePage;
pub(crate) use routing::{youtube_item_action, YouTubeItemAction};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct YouTubeItem {
    pub result_type: String,
    pub title: String,
    pub subtitle: String,
    pub video_id: String,
    pub browse_id: String,
    pub album: String,
    pub artist: String,
    pub playlist_kind: String,
    pub params: String,
    pub duration_seconds: u64,
    pub thumbnail_url: String,
    pub cover_path: String,
}

impl YouTubeItem {
    pub fn playable(&self) -> bool {
        !self.video_id.is_empty()
    }

    pub fn cached_cover(&self) -> Option<&Path> {
        let path = Path::new(&self.cover_path);
        (!self.cover_path.is_empty() && path.is_file()).then_some(path)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum YouTubeCacheVisualState {
    #[default]
    Hidden,
    Fresh,
    Stale,
}

static YOUTUBE_CACHE_VISUAL_STATE: AtomicU8 = AtomicU8::new(0);

pub fn youtube_cache_visual_state() -> YouTubeCacheVisualState {
    match YOUTUBE_CACHE_VISUAL_STATE.load(AtomicOrdering::Relaxed) {
        1 => YouTubeCacheVisualState::Fresh,
        2 => YouTubeCacheVisualState::Stale,
        _ => YouTubeCacheVisualState::Hidden,
    }
}

fn set_youtube_cache_visual_state(state: YouTubeCacheVisualState) {
    let value = match state {
        YouTubeCacheVisualState::Hidden => 0,
        YouTubeCacheVisualState::Fresh => 1,
        YouTubeCacheVisualState::Stale => 2,
    };
    YOUTUBE_CACHE_VISUAL_STATE.store(value, AtomicOrdering::Relaxed);
}

pub fn cached_cover_for_item(item: &YouTubeItem) -> Option<PathBuf> {
    if let Some(path) = item.cached_cover() {
        return Some(path.to_path_buf());
    }

    let original = item.thumbnail_url.trim();
    if original.is_empty() {
        return None;
    }

    let cache_root = glib::user_cache_dir()
        .join("nocky")
        .join("youtube")
        .join("covers");

    for size in [PLAYER_COVER_SIZE, BROWSER_COVER_SIZE] {
        let upgraded = upgrade_thumbnail_url(original, size);
        let digest = stable_hash(&upgraded);
        let candidate = cache_root.join(format!("{digest:016x}-{size}.cover"));
        if candidate.is_file()
            && fs::metadata(&candidate)
                .map(|metadata| metadata.len() > 0)
                .unwrap_or(false)
        {
            return Some(candidate);
        }
    }

    None
}

pub fn cacheable_youtube_playlist(item: &YouTubeItem) -> bool {
    item.result_type == "playlist"
        && !item.browse_id.is_empty()
        && (item.playlist_kind.is_empty() || item.playlist_kind == "library")
}

pub fn youtube_like_error_message(error: &str) -> &'static str {
    let normalized = error.to_lowercase();

    if normalized.contains("connect")
        || normalized.contains("network")
        || normalized.contains("offline")
        || normalized.contains("timed out")
        || normalized.contains("timeout")
    {
        "Sem conexão com o YouTube Music. A curtida foi restaurada ao estado anterior."
    } else if normalized.contains("permission")
        || normalized.contains("forbidden")
        || normalized.contains("403")
    {
        "O YouTube Music recusou a alteração. Verifique as permissões da conta."
    } else if normalized.contains("session")
        || normalized.contains("authentication")
        || normalized.contains("unauthorized")
        || normalized.contains("401")
    {
        "A sessão do YouTube Music expirou. Reconecte sua conta para continuar."
    } else {
        "Não foi possível sincronizar a curtida com o YouTube Music."
    }
}

pub fn credited_artists(credit: &str) -> Vec<String> {
    let normalized = credit
        .replace(" featuring ", " feat. ")
        .replace(" Featuring ", " feat. ")
        .replace(" FEATURING ", " feat. ")
        .replace(" feat ", " feat. ")
        .replace(" ft. ", " feat. ")
        .replace(" ft ", " feat. ")
        .replace(" x ", " feat. ")
        .replace(" X ", " feat. ")
        .replace(" with ", " feat. ")
        .replace(" With ", " feat. ")
        .replace(" / ", " feat. ")
        .replace(';', " feat. ")
        .replace(" • ", " feat. ");

    let mut artists = Vec::new();
    for segment in normalized.split(" feat. ") {
        for artist in segment.split(',') {
            let artist = artist.trim();
            if artist.is_empty()
                || artists
                    .iter()
                    .any(|existing: &String| existing.eq_ignore_ascii_case(artist))
            {
                continue;
            }
            artists.push(artist.to_string());
        }
    }

    if artists.is_empty() {
        let fallback = credit.trim();
        if !fallback.is_empty() {
            artists.push(fallback.to_string());
        }
    }

    artists
}

pub fn artist_credit_contains(credit: &str, artist: &str) -> bool {
    credited_artists(credit)
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(artist.trim()))
}

fn normalize_collection_component(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn normalized_collection_kind(kind: &str) -> &'static str {
    if kind.eq_ignore_ascii_case("artist") {
        "artist"
    } else {
        "album"
    }
}

pub fn youtube_collection_key(kind: &str, title: &str) -> String {
    let kind = normalized_collection_kind(kind);
    format!("{kind}:{}", normalize_collection_component(title))
}

fn youtube_collection_legacy_key(item: &YouTubeItem) -> String {
    youtube_collection_key(&item.result_type, &item.title)
}

pub fn youtube_collection_cache_key(item: &YouTubeItem) -> String {
    let kind = normalized_collection_kind(&item.result_type);
    let browse_id = normalize_collection_component(&item.browse_id);
    if !browse_id.is_empty() {
        return format!("{kind}:browse:{browse_id}");
    }

    let title = normalize_collection_component(&item.title);
    if kind == "album" {
        let artist = normalize_collection_component(&item.artist);
        if !artist.is_empty() {
            return format!("album:title:{title}:artist:{artist}");
        }
    }

    youtube_collection_key(kind, &item.title)
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct YouTubeArtistOverview {
    pub profile: YouTubeItem,
    pub albums: Vec<YouTubeItem>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct YouTubeStatus {
    pub connected: bool,
    pub account: String,
    pub storage: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct YouTubeLibrarySnapshot {
    pub library: Vec<YouTubeItem>,
    pub liked: Vec<YouTubeItem>,
    pub playlists: Vec<YouTubeItem>,
    pub suggested_albums: Vec<YouTubeItem>,
    pub suggested_artists: Vec<YouTubeItem>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct YouTubeLibrarySyncChanges {
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
}

impl YouTubeLibrarySyncChanges {
    pub fn changed(self) -> bool {
        self.added > 0 || self.updated > 0 || self.removed > 0
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct YouTubeCollectionEntry {
    pub title: String,
    pub subtitle: String,
    pub detail: String,
    pub cover_path: String,
    pub item_count: usize,
    pub source: YouTubeItem,
}

impl YouTubeCollectionEntry {
    pub fn cached_cover(&self) -> Option<&Path> {
        let path = Path::new(&self.cover_path);
        (!self.cover_path.is_empty() && path.is_file()).then_some(path)
    }
}

#[derive(Clone, Debug, Default)]
pub struct YouTubeLibraryCache {
    pub connected: bool,
    pub syncing: bool,
    pub synced: bool,
    pub library: Vec<YouTubeItem>,
    pub liked: Vec<YouTubeItem>,
    pub recently_played: Vec<YouTubeItem>,
    pub playlists: Vec<YouTubeItem>,
    pub suggested_albums: Vec<YouTubeItem>,
    pub suggested_artists: Vec<YouTubeItem>,
    pub playlist_tracks: HashMap<String, Vec<YouTubeItem>>,
    pub playlist_loading: HashSet<String>,
    pub collection_tracks: HashMap<String, Vec<YouTubeItem>>,
    pub collection_loading: HashSet<String>,
    pub artist_profiles: HashMap<String, YouTubeItem>,
    pub artist_albums: HashMap<String, Vec<YouTubeItem>>,
    pub artist_loading: HashSet<String>,
    pub albums: Vec<YouTubeCollectionEntry>,
    pub artists: Vec<YouTubeCollectionEntry>,
    pub search: YouTubeSearchResults,
}

fn youtube_item_identity(item: &YouTubeItem) -> String {
    if !item.video_id.trim().is_empty() {
        return format!("video:{}", item.video_id.trim());
    }
    if !item.browse_id.trim().is_empty() {
        return format!(
            "browse:{}:{}",
            item.result_type.trim().to_ascii_lowercase(),
            item.browse_id.trim()
        );
    }

    format!(
        "fallback:{}:{}:{}:{}",
        item.result_type.trim().to_ascii_lowercase(),
        item.title.trim().to_ascii_lowercase(),
        item.artist.trim().to_ascii_lowercase(),
        item.album.trim().to_ascii_lowercase()
    )
}

fn preserve_cached_item_fields(new_item: &mut YouTubeItem, previous: &YouTubeItem) {
    if new_item.cover_path.is_empty() {
        new_item.cover_path = previous.cover_path.clone();
    }
    if new_item.thumbnail_url.is_empty() {
        new_item.thumbnail_url = previous.thumbnail_url.clone();
    }
    if new_item.album.is_empty() {
        new_item.album = previous.album.clone();
    }
    if new_item.artist.is_empty() {
        new_item.artist = previous.artist.clone();
    }
    if new_item.subtitle.is_empty() {
        new_item.subtitle = previous.subtitle.clone();
    }
    if new_item.duration_seconds == 0 {
        new_item.duration_seconds = previous.duration_seconds;
    }
}

fn merge_youtube_items(
    previous: &[YouTubeItem],
    incoming: Vec<YouTubeItem>,
) -> (Vec<YouTubeItem>, YouTubeLibrarySyncChanges) {
    let previous_by_id = previous
        .iter()
        .map(|item| (youtube_item_identity(item), item))
        .collect::<HashMap<_, _>>();
    let incoming_ids = incoming
        .iter()
        .map(youtube_item_identity)
        .collect::<HashSet<_>>();

    let mut changes = YouTubeLibrarySyncChanges {
        removed: previous_by_id
            .keys()
            .filter(|identity| !incoming_ids.contains(*identity))
            .count(),
        ..YouTubeLibrarySyncChanges::default()
    };

    let mut merged = Vec::with_capacity(incoming.len());
    let mut seen = HashSet::new();

    for mut item in incoming {
        let identity = youtube_item_identity(&item);
        if !seen.insert(identity.clone()) {
            continue;
        }

        match previous_by_id.get(&identity) {
            Some(previous_item) => {
                preserve_cached_item_fields(&mut item, previous_item);
                if item != **previous_item {
                    changes.updated += 1;
                }
            }
            None => changes.added += 1,
        }

        merged.push(item);
    }

    (merged, changes)
}

fn merge_sync_change_counts(
    total: &mut YouTubeLibrarySyncChanges,
    next: YouTubeLibrarySyncChanges,
) {
    total.added += next.added;
    total.updated += next.updated;
    total.removed += next.removed;
}

impl YouTubeLibraryCache {
    pub fn has_content(&self) -> bool {
        !self.library.is_empty() || !self.liked.is_empty() || !self.playlists.is_empty()
    }

    pub fn clear(&mut self) {
        set_youtube_cache_visual_state(YouTubeCacheVisualState::Hidden);
        self.connected = false;
        self.syncing = false;
        self.synced = false;
        self.library.clear();
        self.liked.clear();
        self.recently_played.clear();
        self.playlists.clear();
        self.suggested_albums.clear();
        self.suggested_artists.clear();
        self.playlist_tracks.clear();
        self.playlist_loading.clear();
        self.collection_tracks.clear();
        self.collection_loading.clear();
        self.artist_profiles.clear();
        self.artist_albums.clear();
        self.artist_loading.clear();
        self.albums.clear();
        self.artists.clear();
        self.search = YouTubeSearchResults::default();
    }

    pub fn apply(&mut self, snapshot: YouTubeLibrarySnapshot) -> YouTubeLibrarySyncChanges {
        set_youtube_cache_visual_state(YouTubeCacheVisualState::Fresh);
        self.syncing = false;
        self.synced = true;

        let mut changes = YouTubeLibrarySyncChanges::default();

        let (library, library_changes) = merge_youtube_items(&self.library, snapshot.library);
        merge_sync_change_counts(&mut changes, library_changes);
        self.library = library;

        let (liked, liked_changes) = merge_youtube_items(&self.liked, snapshot.liked);
        merge_sync_change_counts(&mut changes, liked_changes);
        self.liked = liked;

        let previous_playlist_ids = self
            .playlists
            .iter()
            .filter(|item| cacheable_youtube_playlist(item))
            .map(|item| item.browse_id.clone())
            .collect::<HashSet<_>>();
        let (playlists, playlist_changes) =
            merge_youtube_items(&self.playlists, snapshot.playlists);
        merge_sync_change_counts(&mut changes, playlist_changes);
        self.playlists = playlists;

        let (suggested_albums, album_changes) =
            merge_youtube_items(&self.suggested_albums, snapshot.suggested_albums);
        merge_sync_change_counts(&mut changes, album_changes);
        self.suggested_albums = suggested_albums;

        let (suggested_artists, artist_changes) =
            merge_youtube_items(&self.suggested_artists, snapshot.suggested_artists);
        merge_sync_change_counts(&mut changes, artist_changes);
        self.suggested_artists = suggested_artists;

        let valid_playlists = self
            .playlists
            .iter()
            .filter(|item| cacheable_youtube_playlist(item))
            .map(|item| item.browse_id.clone())
            .collect::<HashSet<_>>();
        self.playlist_tracks.retain(|browse_id, _| {
            valid_playlists.contains(browse_id) && previous_playlist_ids.contains(browse_id)
        });
        self.playlist_loading
            .retain(|browse_id| valid_playlists.contains(browse_id));

        let previous_collection_keys = self
            .albums
            .iter()
            .chain(self.artists.iter())
            .map(|entry| youtube_collection_cache_key(&entry.source))
            .collect::<HashSet<_>>();

        self.rebuild_collections();

        let valid_collections = self
            .albums
            .iter()
            .chain(self.artists.iter())
            .map(|entry| youtube_collection_cache_key(&entry.source))
            .collect::<HashSet<_>>();
        self.collection_tracks.retain(|key, _| {
            valid_collections.contains(key) && previous_collection_keys.contains(key)
        });
        self.collection_loading
            .retain(|key| valid_collections.contains(key));
        self.artist_profiles
            .retain(|key, _| valid_collections.contains(key));
        self.artist_albums
            .retain(|key, _| valid_collections.contains(key));
        self.artist_loading
            .retain(|key| valid_collections.contains(key));

        changes
    }

    pub fn rebuild_collections(&mut self) {
        let catalog = youtube_catalog(&self.library, &self.liked, &self.recently_played);
        self.albums = build_album_cache(&catalog);
        self.artists = build_artist_cache(&catalog);
        merge_suggested_collections(&mut self.albums, &self.suggested_albums, "album");
        merge_suggested_collections(&mut self.artists, &self.suggested_artists, "artist");
    }

    pub fn observe_playback(&mut self, item: &YouTubeItem) -> bool {
        if item.video_id.trim().is_empty() {
            return false;
        }

        let mut observed = item.clone();
        if let Some(existing) = self
            .recently_played
            .iter()
            .find(|candidate| candidate.video_id == observed.video_id)
        {
            if observed.thumbnail_url.is_empty() {
                observed.thumbnail_url = existing.thumbnail_url.clone();
            }
            if observed.cover_path.is_empty() {
                observed.cover_path = existing.cover_path.clone();
            }
        }
        if observed.cover_path.is_empty() {
            if let Some(path) = cached_cover_for_item(&observed) {
                observed.cover_path = path.to_string_lossy().into_owned();
            }
        }
        if observed.result_type.is_empty() {
            observed.result_type = "song".to_string();
        }

        let previous = self
            .recently_played
            .iter()
            .position(|candidate| candidate.video_id == observed.video_id);
        let unchanged = previous
            .and_then(|index| self.recently_played.get(index))
            .is_some_and(|candidate| {
                candidate.title == observed.title
                    && candidate.artist == observed.artist
                    && candidate.album == observed.album
                    && candidate.cover_path == observed.cover_path
                    && candidate.thumbnail_url == observed.thumbnail_url
            });

        if let Some(index) = previous {
            self.recently_played.remove(index);
        }
        self.recently_played.insert(0, observed);
        self.recently_played.truncate(240);
        self.rebuild_collections();

        previous != Some(0) || !unchanged
    }

    pub fn presentation_signature(&self) -> u64 {
        fn hash_item(item: &YouTubeItem, hasher: &mut DefaultHasher) {
            item.result_type.hash(hasher);
            item.title.hash(hasher);
            item.subtitle.hash(hasher);
            item.video_id.hash(hasher);
            item.browse_id.hash(hasher);
            item.album.hash(hasher);
            item.artist.hash(hasher);
            item.playlist_kind.hash(hasher);
            item.params.hash(hasher);
            item.duration_seconds.hash(hasher);
            item.thumbnail_url.hash(hasher);
            item.cover_path.hash(hasher);
        }

        fn hash_entry(entry: &YouTubeCollectionEntry, hasher: &mut DefaultHasher) {
            entry.title.hash(hasher);
            entry.subtitle.hash(hasher);
            entry.detail.hash(hasher);
            entry.cover_path.hash(hasher);
            entry.item_count.hash(hasher);
            hash_item(&entry.source, hasher);
        }

        fn hash_items(label: &str, items: &[YouTubeItem], hasher: &mut DefaultHasher) {
            label.hash(hasher);
            items.len().hash(hasher);
            for item in items {
                hash_item(item, hasher);
            }
        }

        let mut hasher = DefaultHasher::new();
        hash_items("library", &self.library, &mut hasher);
        hash_items("liked", &self.liked, &mut hasher);
        hash_items("recently_played", &self.recently_played, &mut hasher);
        hash_items("playlists", &self.playlists, &mut hasher);
        hash_items("suggested_albums", &self.suggested_albums, &mut hasher);
        hash_items("suggested_artists", &self.suggested_artists, &mut hasher);

        "albums".hash(&mut hasher);
        self.albums.len().hash(&mut hasher);
        for entry in &self.albums {
            hash_entry(entry, &mut hasher);
        }

        "artists".hash(&mut hasher);
        self.artists.len().hash(&mut hasher);
        for entry in &self.artists {
            hash_entry(entry, &mut hasher);
        }

        hasher.finish()
    }

    pub fn repair_recent_cover_paths(&mut self) -> bool {
        let mut changed = false;

        for item in &mut self.recently_played {
            if item.cover_path.is_empty() {
                if let Some(path) = cached_cover_for_item(item) {
                    item.cover_path = path.to_string_lossy().into_owned();
                    changed = true;
                }
            }
        }

        if changed {
            self.rebuild_collections();
        }
        changed
    }
}

const LIBRARY_CACHE_VERSION: u32 = 7;
const LIBRARY_CACHE_COMPAT_VERSION: u32 = 6;
const YOUTUBE_CACHE_DETAIL_TTL_SECS: u64 = 6 * 60 * 60;
const YOUTUBE_CACHE_SUGGESTION_TTL_SECS: u64 = 12 * 60 * 60;
const YOUTUBE_CACHE_REVALIDATE_SECS: u64 = 24 * 60 * 60;
const BROWSER_COVER_SIZE: u32 = 512;
const PLAYER_COVER_SIZE: u32 = 1200;

static COVER_CLIENT: OnceLock<Option<reqwest::blocking::Client>> = OnceLock::new();
static LIBRARY_CACHE_WRITER: OnceLock<Option<Sender<YouTubeLibraryCache>>> = OnceLock::new();
static LIBRARY_CACHE_WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct PersistedYouTubeLibraryCache {
    version: u32,
    saved_at: u64,
    library: Vec<YouTubeItem>,
    liked: Vec<YouTubeItem>,
    #[serde(default)]
    recently_played: Vec<YouTubeItem>,
    playlists: Vec<YouTubeItem>,
    suggested_albums: Vec<YouTubeItem>,
    suggested_artists: Vec<YouTubeItem>,
    playlist_tracks: HashMap<String, Vec<YouTubeItem>>,
    collection_tracks: HashMap<String, Vec<YouTubeItem>>,
    artist_profiles: HashMap<String, YouTubeItem>,
    artist_albums: HashMap<String, Vec<YouTubeItem>>,
    albums: Vec<YouTubeCollectionEntry>,
    artists: Vec<YouTubeCollectionEntry>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct YouTubeStream {
    pub video_id: String,
    pub stream_url: String,
    pub webpage_url: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_seconds: u64,
    pub thumbnail_url: String,
    pub http_headers: HashMap<String, String>,
    pub format_id: String,
    pub protocol: String,
    pub container: String,
    pub audio_codec: String,
    pub stream_client: String,
    pub stream_client_label: String,
    pub attempted_clients: Vec<String>,
    pub fallback_used: bool,
    pub expires_at: f64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct YouTubeHomeSuggestions {
    pub playlists: Vec<YouTubeItem>,
    pub albums: Vec<YouTubeItem>,
    pub artists: Vec<YouTubeItem>,
}

#[derive(Clone, Debug, Default)]
pub struct YouTubeSearchResults {
    pub query: String,
    pub loading: bool,
    pub error: String,
    pub songs: Vec<YouTubeItem>,
    pub albums: Vec<YouTubeItem>,
    pub artists: Vec<YouTubeItem>,
    pub playlists: Vec<YouTubeItem>,
}

fn youtube_search_item_key(item: &YouTubeItem) -> String {
    if !item.video_id.is_empty() {
        return format!("video:{}", item.video_id);
    }
    if !item.browse_id.is_empty() {
        return format!("browse:{}", item.browse_id);
    }

    format!(
        "metadata:{}:{}:{}:{}",
        normalize_search_text(&item.result_type),
        normalize_search_text(&item.title),
        normalize_search_text(&item.artist),
        normalize_search_text(&item.album),
    )
}

fn youtube_search_haystack(item: &YouTubeItem) -> String {
    format!(
        "{} {} {} {} {}",
        item.title, item.subtitle, item.artist, item.album, item.playlist_kind
    )
}

fn append_unique_search_items(
    target: &mut Vec<YouTubeItem>,
    items: impl IntoIterator<Item = YouTubeItem>,
) {
    let mut seen = target
        .iter()
        .map(youtube_search_item_key)
        .collect::<HashSet<_>>();

    for item in items {
        if seen.insert(youtube_search_item_key(&item)) {
            target.push(item);
        }
    }
}

fn cached_search_items<'a>(
    items: impl IntoIterator<Item = &'a YouTubeItem>,
    query: &str,
    playable_only: bool,
) -> Vec<YouTubeItem> {
    let mut matches = items
        .into_iter()
        .filter(|item| !playable_only || item.playable())
        .filter_map(|item| {
            let haystack = youtube_search_haystack(item);
            search_matches(&haystack, query).then(|| (search_score(&haystack, query), item.clone()))
        })
        .collect::<Vec<_>>();

    matches.sort_by_key(|(score, _)| *score);
    let mut unique = Vec::new();
    append_unique_search_items(&mut unique, matches.into_iter().map(|(_, item)| item));
    unique
}

impl YouTubeSearchResults {
    pub(crate) fn merge_cached_results(&mut self, cached: &Self) {
        append_unique_search_items(&mut self.songs, cached.songs.clone());
        append_unique_search_items(&mut self.albums, cached.albums.clone());
        append_unique_search_items(&mut self.artists, cached.artists.clone());
        append_unique_search_items(&mut self.playlists, cached.playlists.clone());
    }
}

impl YouTubeLibraryCache {
    pub(crate) fn cached_search_results(&self, raw_query: &str) -> YouTubeSearchResults {
        let query = normalize_search_text(raw_query);
        let mut results = YouTubeSearchResults {
            query: raw_query.trim().to_string(),
            ..YouTubeSearchResults::default()
        };

        results.songs = cached_search_items(
            self.library
                .iter()
                .chain(self.liked.iter())
                .chain(self.recently_played.iter())
                .chain(self.playlist_tracks.values().flatten())
                .chain(self.collection_tracks.values().flatten()),
            &query,
            true,
        );

        results.albums = cached_search_items(
            self.albums
                .iter()
                .map(|entry| &entry.source)
                .chain(self.suggested_albums.iter())
                .chain(self.artist_albums.values().flatten()),
            &query,
            false,
        );

        results.artists = cached_search_items(
            self.artists
                .iter()
                .map(|entry| &entry.source)
                .chain(self.suggested_artists.iter())
                .chain(self.artist_profiles.values()),
            &query,
            false,
        );

        results.playlists = cached_search_items(self.playlists.iter(), &query, false);
        results
    }
}

#[derive(Debug, Deserialize)]
struct HelperResponse<T> {
    ok: bool,
    result: Option<T>,
    error: Option<String>,
}

#[derive(Clone, Debug)]
pub enum YouTubePageEvent {
    Search {
        query: String,
        filter: String,
    },
    Connect(String),
    Disconnect,
    SyncLibrary,
    LoadLibrary,
    LoadLiked,
    LoadPlaylists,
    LoadHome {
        continuation: String,
    },
    LoadLibraryOverview,
    Activate {
        item: YouTubeItem,
        queue: Vec<YouTubeItem>,
        index: usize,
    },
    OpenPlaylist(YouTubeItem),
    OpenCollection(YouTubeItem),
    UnsupportedItem {
        title: String,
        result_type: String,
    },
}

pub struct YouTubeBridge {
    python: PathBuf,
    helper: PathBuf,
}

impl YouTubeBridge {
    pub fn discover() -> Result<Self, String> {
        let helper = helper_path().ok_or_else(|| {
            "The Nocky YouTube helper was not found. Reinstall Nocky 0.2.4.".to_string()
        })?;
        let python = python_path().ok_or_else(|| {
            "The YouTube Music Python runtime is missing or incomplete. Run ./scripts/setup-youtube-runtime.sh for development, or reinstall with ./install.sh --install-youtube.".to_string()
        })?;
        Ok(Self { python, helper })
    }

    pub fn status(&self) -> Result<YouTubeStatus, String> {
        self.run("status", json!({}))
    }

    pub fn connect(&self, raw: &str) -> Result<YouTubeStatus, String> {
        self.run("connect", json!({ "raw": raw }))
    }

    pub fn disconnect(&self) -> Result<YouTubeStatus, String> {
        self.run("disconnect", json!({}))
    }

    pub fn search(&self, query: &str, filter: &str) -> Result<Vec<YouTubeItem>, String> {
        self.run(
            "search",
            json!({ "query": query, "filter": filter, "limit": 30 }),
        )
    }

    pub fn library(&self) -> Result<Vec<YouTubeItem>, String> {
        self.run("library", json!({ "limit": 200 }))
    }

    pub fn liked(&self) -> Result<Vec<YouTubeItem>, String> {
        self.run("liked", json!({ "limit": 200 }))
    }

    pub fn playlists(&self) -> Result<Vec<YouTubeItem>, String> {
        self.run("playlists", json!({ "limit": 150, "home_limit": 8 }))
    }

    fn library_playlists(&self) -> Result<Vec<YouTubeItem>, String> {
        self.run("playlists", json!({ "limit": 150, "home_limit": 0 }))
    }

    pub fn home(&self) -> Result<YouTubeHomeSuggestions, String> {
        self.run("home", json!({ "limit": 8 }))
    }

    pub fn home_page(&self, continuation: Option<&str>) -> Result<YouTubeHomePage, String> {
        self.run(
            "home_v2",
            json!({
                "continuation": continuation.unwrap_or_default(),
                "section_limit": 6,
            }),
        )
    }

    pub fn library_overview(&self) -> Result<YouTubeHomePage, String> {
        self.run("library_v2", json!({ "limit": 80 }))
    }

    pub fn sync_library(&self) -> Result<YouTubeLibrarySnapshot, String> {
        let home = self.home().unwrap_or_else(|error| {
            eprintln!("Could not load YouTube Music home suggestions: {error}");
            YouTubeHomeSuggestions::default()
        });
        let mut playlists = self.library_playlists()?;
        extend_unique_youtube_items(&mut playlists, home.playlists);
        let mut snapshot = YouTubeLibrarySnapshot {
            library: self.library()?,
            liked: self.liked()?,
            playlists,
            suggested_albums: home.albums,
            suggested_artists: home.artists,
        };
        cache_library_covers(&mut snapshot);
        Ok(snapshot)
    }

    pub fn playlist(&self, playlist: &YouTubeItem) -> Result<Vec<YouTubeItem>, String> {
        self.run(
            "playlist",
            json!({
                "browse_id": playlist.browse_id,
                "video_id": playlist.video_id,
                "playlist_kind": playlist.playlist_kind,
                "params": playlist.params,
                // Loading hundreds of entries before the first paint made
                // cold playlist navigation unnecessarily slow. A larger,
                // paginated model can be introduced later; 120 covers the
                // common case while cutting response and render time sharply.
                "limit": 120,
            }),
        )
    }

    pub fn collection(&self, item: &YouTubeItem) -> Result<Vec<YouTubeItem>, String> {
        self.run(
            "collection",
            json!({
                "result_type": item.result_type,
                "browse_id": item.browse_id,
                "title": item.title,
                "params": item.params,
                "limit": 120,
            }),
        )
    }

    pub fn artist_overview(&self, item: &YouTubeItem) -> Result<YouTubeArtistOverview, String> {
        self.run(
            "artist",
            json!({
                "browse_id": item.browse_id,
                "title": item.title,
                "limit": 160,
            }),
        )
    }

    pub fn rate(&self, video_id: &str, liked: bool) -> Result<bool, String> {
        self.run(
            "rate",
            json!({
                "video_id": video_id,
                "liked": liked,
            }),
        )
    }

    pub fn resolve(&self, video_id: &str, force: bool) -> Result<YouTubeStream, String> {
        self.run("resolve", json!({ "video_id": video_id, "force": force }))
    }

    pub fn preload_streams(&self, queue: &[YouTubeItem], current_index: usize, limit: usize) {
        if queue.is_empty() || limit == 0 {
            return;
        }

        let mut seen = HashSet::new();
        for item in queue
            .iter()
            .skip(current_index.saturating_add(1))
            .take(limit)
        {
            if item.video_id.is_empty() || !seen.insert(item.video_id.clone()) {
                continue;
            }
            let _ = self.resolve(&item.video_id, false);
        }
    }

    fn run<T: DeserializeOwned>(
        &self,
        command: &str,
        payload: serde_json::Value,
    ) -> Result<T, String> {
        let mut child = Command::new(&self.python)
            .arg(&self.helper)
            .arg(command)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("Could not start the YouTube helper: {error}"))?;

        if let Some(mut stdin) = child.stdin.take() {
            serde_json::to_writer(&mut stdin, &payload)
                .map_err(|error| format!("Could not send data to the YouTube helper: {error}"))?;
        }

        let output = child
            .wait_with_output()
            .map_err(|error| format!("The YouTube helper did not finish: {error}"))?;
        let response: HelperResponse<T> =
            serde_json::from_slice(&output.stdout).map_err(|error| {
                let stderr = String::from_utf8_lossy(&output.stderr);
                format!("Invalid response from the YouTube helper: {error}. {stderr}")
            })?;

        if !response.ok {
            return Err(response
                .error
                .unwrap_or_else(|| "The YouTube helper reported an unknown error".to_string()));
        }
        response
            .result
            .ok_or_else(|| "The YouTube helper returned no result".to_string())
    }
}

pub struct YouTubePage {
    root: gtk::Box,
    status: gtk::Label,
    connect_button: gtk::Button,
    disconnect_button: gtk::Button,
    private_actions: gtk::Box,
    auth_revealer: gtk::Revealer,
    auth_buffer: gtk::TextBuffer,
    search_entry: gtk::SearchEntry,
    filter: gtk::DropDown,
    heading: gtk::Label,
    loading: ExpressiveLoadingIndicator,
    results: gtk::ListBox,
    items: RefCell<Vec<YouTubeItem>>,
    structured_page: RefCell<YouTubeHomePage>,
    event_tx: Sender<YouTubePageEvent>,
    event_rx: Receiver<YouTubePageEvent>,
}

impl YouTubePage {
    pub fn new() -> Rc<Self> {
        let (event_tx, event_rx) = mpsc::channel();

        let root = gtk::Box::new(gtk::Orientation::Vertical, 14);
        root.set_margin_top(20);
        root.set_margin_bottom(20);
        root.set_margin_start(24);
        root.set_margin_end(24);
        root.set_vexpand(true);
        root.add_css_class("youtube-page");

        let title = gtk::Label::new(Some("YouTube Music"));
        title.set_xalign(0.0);
        title.add_css_class("title-1");
        let subtitle = gtk::Label::new(Some(
            "Busque no catálogo ou conecte a sessão do navegador para acessar sua biblioteca.",
        ));
        subtitle.set_xalign(0.0);
        subtitle.set_wrap(true);
        subtitle.add_css_class("dim-label");

        let status = gtk::Label::new(Some("Verificando conta..."));
        status.set_xalign(0.0);
        status.set_hexpand(true);
        status.add_css_class("youtube-status");
        let connect_button = gtk::Button::with_label("Conectar conta");
        connect_button.add_css_class("suggested-action");
        let disconnect_button = gtk::Button::with_label("Desconectar");
        disconnect_button.add_css_class("flat");
        disconnect_button.set_visible(false);
        let account_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        account_row.append(&status);
        account_row.append(&connect_button);
        account_row.append(&disconnect_button);

        let auth_text = gtk::Label::new(Some(
            "Abra o YouTube Music no navegador do sistema, entre na conta e copie uma requisição bem-sucedida como cURL ou apenas o cabeçalho Cookie. O Nocky nunca solicita sua senha e guarda somente os cabeçalhos mínimos no Secret Service quando disponível.",
        ));
        auth_text.set_wrap(true);
        auth_text.set_xalign(0.0);
        auth_text.add_css_class("dim-label");
        let auth_buffer = gtk::TextBuffer::new(None::<&gtk::TextTagTable>);
        let auth_view = gtk::TextView::with_buffer(&auth_buffer);
        auth_view.set_wrap_mode(gtk::WrapMode::WordChar);
        auth_view.set_monospace(true);
        auth_view.set_top_margin(8);
        auth_view.set_bottom_margin(8);
        auth_view.set_left_margin(8);
        auth_view.set_right_margin(8);
        let auth_scroll = gtk::ScrolledWindow::new();
        auth_scroll.set_min_content_height(110);
        auth_scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
        auth_scroll.set_child(Some(&auth_view));
        auth_scroll.add_css_class("youtube-auth-input");
        let import_button = gtk::Button::with_label("Importar sessão");
        import_button.add_css_class("suggested-action");
        let open_browser_button = gtk::Button::with_label("Abrir no navegador");
        open_browser_button.add_css_class("flat");
        let cancel_auth = gtk::Button::with_label("Cancelar");
        cancel_auth.add_css_class("flat");
        let auth_buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        auth_buttons.set_halign(gtk::Align::End);
        auth_buttons.append(&open_browser_button);
        auth_buttons.append(&cancel_auth);
        auth_buttons.append(&import_button);
        let auth_box = gtk::Box::new(gtk::Orientation::Vertical, 10);
        auth_box.add_css_class("youtube-auth-card");
        auth_box.append(&auth_text);
        auth_box.append(&auth_scroll);
        auth_box.append(&auth_buttons);
        let auth_revealer = gtk::Revealer::new();
        auth_revealer.set_transition_type(gtk::RevealerTransitionType::SlideDown);
        auth_revealer.set_child(Some(&auth_box));

        let search_entry = gtk::SearchEntry::builder()
            .placeholder_text("Buscar músicas, artistas, álbuns ou playlists")
            .hexpand(true)
            .build();
        let filter = gtk::DropDown::from_strings(&[
            "Músicas",
            "Tudo",
            "Vídeos",
            "Álbuns",
            "Artistas",
            "Playlists",
        ]);
        filter.set_selected(0);
        let search_button = gtk::Button::with_label("Buscar");
        search_button.add_css_class("suggested-action");
        let search_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        search_row.append(&search_entry);
        search_row.append(&filter);
        search_row.append(&search_button);

        let sync_button = gtk::Button::with_label("Sincronizar com Nocky");
        sync_button.add_css_class("suggested-action");
        let home_button = gtk::Button::with_label("Para você");
        let overview_button = gtk::Button::with_label("Visão geral");
        let library_button = gtk::Button::with_label("Biblioteca");
        let liked_button = gtk::Button::with_label("Curtidas");
        let playlists_button = gtk::Button::with_label("Playlists");
        for button in [
            &home_button,
            &overview_button,
            &library_button,
            &liked_button,
            &playlists_button,
        ] {
            button.add_css_class("pill");
        }
        let private_actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        private_actions.append(&sync_button);
        private_actions.append(&home_button);
        private_actions.append(&overview_button);
        private_actions.append(&library_button);
        private_actions.append(&liked_button);
        private_actions.append(&playlists_button);
        private_actions.set_hexpand(true);
        private_actions.set_sensitive(false);

        let private_actions_scroll = gtk::ScrolledWindow::new();
        private_actions_scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
        private_actions_scroll.set_propagate_natural_height(true);
        private_actions_scroll.set_child(Some(&private_actions));
        private_actions_scroll.set_tooltip_text(Some(
            "Deslize horizontalmente para acessar todas as ações do YouTube Music",
        ));

        let heading = gtk::Label::new(Some("Buscar no YouTube Music"));
        heading.set_xalign(0.0);
        heading.set_hexpand(true);
        heading.add_css_class("title-3");
        let loading = ExpressiveLoadingIndicator::new();
        loading.widget().set_visible(false);
        let results_header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        results_header.append(&heading);
        results_header.append(loading.widget());

        let results = gtk::ListBox::new();
        results.set_selection_mode(gtk::SelectionMode::None);
        results.add_css_class("boxed-list");
        results.add_css_class("youtube-results");
        let results_scroll = gtk::ScrolledWindow::new();
        results_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        results_scroll.set_vexpand(true);
        results_scroll.set_child(Some(&results));

        root.append(&title);
        root.append(&subtitle);
        root.append(&account_row);
        root.append(&auth_revealer);
        root.append(&search_row);
        root.append(&private_actions_scroll);
        root.append(&results_header);
        root.append(&results_scroll);

        let page = Rc::new(Self {
            root,
            status,
            connect_button,
            disconnect_button,
            private_actions,
            auth_revealer,
            auth_buffer,
            search_entry,
            filter,
            heading,
            loading,
            results,
            items: RefCell::new(Vec::new()),
            structured_page: RefCell::new(YouTubeHomePage::default()),
            event_tx,
            event_rx,
        });

        {
            let button = page.connect_button.clone();
            let weak = Rc::downgrade(&page);
            button.connect_clicked(move |_| {
                if let Some(page) = weak.upgrade() {
                    page.auth_revealer.set_reveal_child(true);
                }
            });
        }
        {
            let weak = Rc::downgrade(&page);
            cancel_auth.connect_clicked(move |_| {
                if let Some(page) = weak.upgrade() {
                    page.auth_revealer.set_reveal_child(false);
                }
            });
        }
        {
            open_browser_button.connect_clicked(move |_| {
                if let Err(error) = gtk::gio::AppInfo::launch_default_for_uri(
                    "https://music.youtube.com/",
                    None::<&gtk::gio::AppLaunchContext>,
                ) {
                    eprintln!("Could not open YouTube Music in the browser: {error}");
                }
            });
        }
        {
            let weak = Rc::downgrade(&page);
            import_button.connect_clicked(move |_| {
                let Some(page) = weak.upgrade() else {
                    return;
                };
                let raw = page
                    .auth_buffer
                    .text(
                        &page.auth_buffer.start_iter(),
                        &page.auth_buffer.end_iter(),
                        false,
                    )
                    .to_string();
                if !raw.trim().is_empty() {
                    let _ = page.event_tx.send(YouTubePageEvent::Connect(raw));
                }
            });
        }
        {
            let sender = page.event_tx.clone();
            let button = page.disconnect_button.clone();
            button.connect_clicked(move |_| {
                let _ = sender.send(YouTubePageEvent::Disconnect);
            });
        }
        {
            let weak = Rc::downgrade(&page);
            search_button.connect_clicked(move |_| {
                if let Some(page) = weak.upgrade() {
                    page.emit_search();
                }
            });
        }
        {
            let entry = page.search_entry.clone();
            let weak = Rc::downgrade(&page);
            entry.connect_activate(move |_| {
                if let Some(page) = weak.upgrade() {
                    page.emit_search();
                }
            });
        }
        {
            let sender = page.event_tx.clone();
            sync_button.connect_clicked(move |_| {
                let _ = sender.send(YouTubePageEvent::SyncLibrary);
            });
        }
        {
            let sender = page.event_tx.clone();
            home_button.connect_clicked(move |_| {
                let _ = sender.send(YouTubePageEvent::LoadHome {
                    continuation: String::new(),
                });
            });
        }
        {
            let sender = page.event_tx.clone();
            overview_button.connect_clicked(move |_| {
                let _ = sender.send(YouTubePageEvent::LoadLibraryOverview);
            });
        }
        {
            let sender = page.event_tx.clone();
            library_button.connect_clicked(move |_| {
                let _ = sender.send(YouTubePageEvent::LoadLibrary);
            });
        }
        {
            let sender = page.event_tx.clone();
            liked_button.connect_clicked(move |_| {
                let _ = sender.send(YouTubePageEvent::LoadLiked);
            });
        }
        {
            let sender = page.event_tx.clone();
            playlists_button.connect_clicked(move |_| {
                let _ = sender.send(YouTubePageEvent::LoadPlaylists);
            });
        }
        {
            let results = page.results.clone();
            let weak = Rc::downgrade(&page);
            results.connect_row_activated(move |_, row| {
                let Some(page) = weak.upgrade() else {
                    return;
                };
                let index = row.index().max(0) as usize;
                let Some(item) = page.items.borrow().get(index).cloned() else {
                    return;
                };
                match youtube_item_action(&item) {
                    YouTubeItemAction::Continue => {
                        let _ = page.event_tx.send(YouTubePageEvent::LoadHome {
                            continuation: item.params.clone(),
                        });
                    }
                    YouTubeItemAction::Play => {
                        let queue = page
                            .items
                            .borrow()
                            .iter()
                            .filter(|item| item.playable())
                            .cloned()
                            .collect::<Vec<_>>();
                        let selected = queue
                            .iter()
                            .position(|candidate| candidate.video_id == item.video_id)
                            .unwrap_or(0);
                        let _ = page.event_tx.send(YouTubePageEvent::Activate {
                            item,
                            queue,
                            index: selected,
                        });
                    }
                    YouTubeItemAction::OpenPlaylist => {
                        let _ = page.event_tx.send(YouTubePageEvent::OpenPlaylist(item));
                    }
                    YouTubeItemAction::OpenCollection => {
                        let _ = page.event_tx.send(YouTubePageEvent::OpenCollection(item));
                    }
                    YouTubeItemAction::Unsupported => {
                        let _ = page.event_tx.send(YouTubePageEvent::UnsupportedItem {
                            title: item.title,
                            result_type: item.result_type,
                        });
                    }
                    YouTubeItemAction::Ignore => {}
                }
            });
        }

        page.show_empty("Busque uma música ou conecte sua conta.");
        page
    }

    pub fn root(&self) -> &gtk::Box {
        &self.root
    }

    pub fn try_recv(&self) -> Option<YouTubePageEvent> {
        self.event_rx.try_recv().ok()
    }

    pub fn set_status(&self, status: &YouTubeStatus) {
        if status.connected {
            let account = if status.account.trim().is_empty() {
                "Conta conectada"
            } else {
                status.account.as_str()
            };
            self.status.set_text(&format!("Conectado: {account}"));
            self.connect_button.set_visible(false);
            self.disconnect_button.set_visible(true);
            self.private_actions.set_sensitive(true);
            self.auth_revealer.set_reveal_child(false);
            self.auth_buffer.set_text("");
        } else {
            self.status
                .set_text("Não conectado - a busca pública continua disponível");
            self.connect_button.set_visible(true);
            self.disconnect_button.set_visible(false);
            self.private_actions.set_sensitive(false);
        }
    }

    pub fn set_loading(&self, loading: bool, title: &str) {
        self.heading.set_text(title);
        self.loading.widget().set_visible(loading);
    }

    pub fn show_structured_page(&self, title: &str, page: YouTubeHomePage, append: bool) {
        {
            let mut current = self.structured_page.borrow_mut();
            if append {
                current.merge_page(page);
            } else {
                *current = page;
            }
        }
        let snapshot = self.structured_page.borrow().clone();
        clear_list_box(&self.results);
        self.loading.widget().set_visible(false);
        let heading = if snapshot.stale {
            format!("{title} • cache offline")
        } else {
            title.to_string()
        };
        self.heading.set_text(&heading);

        let mut rows = Vec::new();
        if !snapshot.chips.is_empty() {
            let chip_summary = YouTubeItem {
                result_type: "chips".to_string(),
                title: snapshot
                    .chips
                    .iter()
                    .map(|chip| chip.title.as_str())
                    .collect::<Vec<_>>()
                    .join("  •  "),
                ..YouTubeItem::default()
            };
            self.results.append(&youtube_row(&chip_summary));
            rows.push(chip_summary);
        }

        for section in &snapshot.sections {
            let header = YouTubeItem {
                result_type: "section".to_string(),
                title: section.title.clone(),
                subtitle: section.label.clone(),
                params: section.layout.clone(),
                ..YouTubeItem::default()
            };
            self.results.append(&youtube_row(&header));
            rows.push(header);
            for item in &section.items {
                self.results.append(&youtube_row(item));
                rows.push(item.clone());
            }
        }

        if !snapshot.continuation.is_empty() {
            let continuation = YouTubeItem {
                result_type: "continuation".to_string(),
                title: "Carregar mais recomendações".to_string(),
                subtitle: "Continuar o feed do YouTube Music".to_string(),
                params: snapshot.continuation.clone(),
                ..YouTubeItem::default()
            };
            self.results.append(&youtube_row(&continuation));
            rows.push(continuation);
        }

        if rows.is_empty() {
            self.results
                .append(&empty_row("Nenhuma seção foi retornada pelo YouTube Music"));
        }
        self.items.replace(rows);
    }

    pub fn show_items(&self, title: &str, items: Vec<YouTubeItem>) {
        self.structured_page.replace(YouTubeHomePage::default());
        clear_list_box(&self.results);
        self.heading.set_text(title);
        self.loading.widget().set_visible(false);
        for item in &items {
            self.results.append(&youtube_row(item));
        }
        if items.is_empty() {
            self.results
                .append(&empty_row("Nenhum resultado encontrado"));
        }
        self.items.replace(items);
    }

    pub fn show_error(&self, message: &str) {
        self.structured_page.replace(YouTubeHomePage::default());
        self.loading.widget().set_visible(false);
        self.heading.set_text("Erro no YouTube Music");
        clear_list_box(&self.results);
        self.results.append(&empty_row(message));
        self.items.borrow_mut().clear();
    }

    pub fn show_empty(&self, message: &str) {
        self.structured_page.replace(YouTubeHomePage::default());
        clear_list_box(&self.results);
        self.results.append(&empty_row(message));
        self.items.borrow_mut().clear();
    }

    fn emit_search(&self) {
        let query = self.search_entry.text().trim().to_string();
        if query.is_empty() {
            return;
        }
        let filters = ["songs", "all", "videos", "albums", "artists", "playlists"];
        let filter = filters
            .get(self.filter.selected() as usize)
            .copied()
            .unwrap_or("songs")
            .to_string();
        let _ = self
            .event_tx
            .send(YouTubePageEvent::Search { query, filter });
    }
}

fn youtube_row(item: &YouTubeItem) -> gtk::ListBoxRow {
    if item.result_type == "section" {
        return youtube_section_row(item);
    }
    if item.result_type == "chips" {
        return youtube_chip_summary_row(item);
    }
    let icon_name = match item.result_type.as_str() {
        "playlist" => "view-list-symbolic",
        "album" => "media-optical-symbolic",
        "artist" => "avatar-default-symbolic",
        "video" | "episode" => "video-x-generic-symbolic",
        "podcast" => "audio-speakers-symbolic",
        "continuation" => "view-more-symbolic",
        _ => "audio-x-generic-symbolic",
    };
    let icon = gtk::Image::from_icon_name(icon_name);
    icon.set_pixel_size(34);
    icon.add_css_class("youtube-result-icon");

    let title = gtk::Label::new(Some(&item.title));
    title.set_xalign(0.0);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title.add_css_class("heading");
    let subtitle = gtk::Label::new(Some(&item.subtitle));
    subtitle.set_xalign(0.0);
    subtitle.set_ellipsize(gtk::pango::EllipsizeMode::End);
    subtitle.add_css_class("dim-label");
    let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
    text.set_hexpand(true);
    text.append(&title);
    text.append(&subtitle);

    let action = gtk::Image::from_icon_name(if item.playable() {
        "media-playback-start-symbolic"
    } else {
        "go-next-symbolic"
    });
    action.set_opacity(0.72);

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    content.set_margin_top(9);
    content.set_margin_bottom(9);
    content.set_margin_start(12);
    content.set_margin_end(12);
    content.append(&icon);
    content.append(&text);
    content.append(&action);

    let row = gtk::ListBoxRow::new();
    row.set_activatable(!matches!(
        youtube_item_action(item),
        YouTubeItemAction::Ignore
    ));
    row.set_child(Some(&content));
    row
}

fn youtube_section_row(item: &YouTubeItem) -> gtk::ListBoxRow {
    let title = gtk::Label::new(Some(&item.title));
    title.set_xalign(0.0);
    title.add_css_class("title-4");
    let content = gtk::Box::new(gtk::Orientation::Vertical, 2);
    content.set_margin_top(18);
    content.set_margin_bottom(6);
    content.set_margin_start(12);
    content.set_margin_end(12);
    content.append(&title);
    if !item.subtitle.is_empty() {
        let subtitle = gtk::Label::new(Some(&item.subtitle));
        subtitle.set_xalign(0.0);
        subtitle.add_css_class("dim-label");
        content.append(&subtitle);
    }
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    row.set_child(Some(&content));
    row
}

fn youtube_chip_summary_row(item: &YouTubeItem) -> gtk::ListBoxRow {
    let label = gtk::Label::new(Some(&item.title));
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.set_margin_top(8);
    label.set_margin_bottom(8);
    label.set_margin_start(12);
    label.set_margin_end(12);
    label.add_css_class("dim-label");
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    row.set_child(Some(&label));
    row
}

fn empty_row(message: &str) -> gtk::ListBoxRow {
    let label = gtk::Label::new(Some(message));
    label.set_wrap(true);
    label.set_justify(gtk::Justification::Center);
    label.set_margin_top(30);
    label.set_margin_bottom(30);
    label.set_margin_start(16);
    label.set_margin_end(16);
    label.add_css_class("dim-label");
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    row.set_child(Some(&label));
    row
}

fn clear_list_box(list: &gtk::ListBox) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
}

pub fn cache_items_for_browser(items: &mut [YouTubeItem]) {
    for item in items {
        item.thumbnail_url = upgrade_thumbnail_url(&item.thumbnail_url, PLAYER_COVER_SIZE);
        if item.cover_path.is_empty() {
            if let Some(path) = download_cover_sized(item, &item.thumbnail_url, BROWSER_COVER_SIZE)
            {
                item.cover_path = path.to_string_lossy().to_string();
            }
        }
    }
}

pub fn cache_library_covers(snapshot: &mut YouTubeLibrarySnapshot) {
    let mut albums = HashSet::new();
    let mut artists = HashSet::new();

    for item in snapshot.library.iter_mut().chain(snapshot.liked.iter_mut()) {
        item.thumbnail_url = upgrade_thumbnail_url(&item.thumbnail_url, PLAYER_COVER_SIZE);
        let album_key = item.album.trim().to_lowercase();
        let artist_key = item.artist.trim().to_lowercase();
        let needs_album = !album_key.is_empty() && albums.insert(album_key);
        let needs_artist = !artist_key.is_empty() && artists.insert(artist_key);
        if (needs_album || needs_artist) && item.cover_path.is_empty() {
            if let Some(path) = download_cover_sized(item, &item.thumbnail_url, BROWSER_COVER_SIZE)
            {
                item.cover_path = path.to_string_lossy().to_string();
            }
        }
    }

    cache_items_for_browser(&mut snapshot.playlists);
    cache_items_for_browser(&mut snapshot.suggested_albums);
    cache_items_for_browser(&mut snapshot.suggested_artists);
}

fn youtube_catalog(
    library: &[YouTubeItem],
    liked: &[YouTubeItem],
    recently_played: &[YouTubeItem],
) -> Vec<YouTubeItem> {
    let mut seen = HashSet::new();
    recently_played
        .iter()
        .chain(library.iter())
        .chain(liked.iter())
        .filter(|item| item.playable())
        .filter(|item| seen.insert(item.video_id.clone()))
        .cloned()
        .collect()
}

fn build_album_cache(catalog: &[YouTubeItem]) -> Vec<YouTubeCollectionEntry> {
    let mut groups: BTreeMap<String, Vec<&YouTubeItem>> = BTreeMap::new();
    for item in catalog {
        let album = item.album.trim();
        if !album.is_empty() {
            groups.entry(album.to_string()).or_default().push(item);
        }
    }

    groups
        .into_iter()
        .map(|(album, items)| {
            let artists = items
                .iter()
                .map(|item| item.artist.as_str())
                .filter(|artist| !artist.is_empty())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
                .join(", ");
            let cover_path = items
                .iter()
                .find_map(|item| item.cached_cover())
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_default();
            let source = YouTubeItem {
                result_type: "album".to_string(),
                title: album.clone(),
                album: album.clone(),
                artist: artists.clone(),
                ..YouTubeItem::default()
            };
            YouTubeCollectionEntry {
                title: album,
                subtitle: artists,
                detail: format!("YouTube Music • {} faixas", items.len()),
                cover_path,
                item_count: items.len(),
                source,
            }
        })
        .collect()
}

fn merge_suggested_collections(
    collections: &mut Vec<YouTubeCollectionEntry>,
    suggestions: &[YouTubeItem],
    kind: &str,
) {
    let mut seen = collections
        .iter()
        .map(|entry| entry.title.trim().to_lowercase())
        .collect::<HashSet<_>>();
    let mut suggested = Vec::new();
    for item in suggestions {
        let title = item.title.trim();
        if title.is_empty() || !seen.insert(title.to_lowercase()) {
            continue;
        }
        suggested.push(YouTubeCollectionEntry {
            title: title.to_string(),
            subtitle: item.subtitle.clone(),
            detail: match kind {
                "artist" => "YouTube Music • sugerido".to_string(),
                _ => "YouTube Music • álbum sugerido".to_string(),
            },
            cover_path: item
                .cached_cover()
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_default(),
            item_count: 0,
            source: item.clone(),
        });
    }
    suggested.extend(collections.iter().cloned());
    *collections = suggested;
}

fn extend_unique_youtube_items(target: &mut Vec<YouTubeItem>, items: Vec<YouTubeItem>) {
    let mut seen = target
        .iter()
        .map(|item| {
            (
                item.result_type.clone(),
                if item.browse_id.is_empty() {
                    item.video_id.clone()
                } else {
                    item.browse_id.clone()
                },
                item.title.clone(),
            )
        })
        .collect::<HashSet<_>>();
    for item in items {
        let key = (
            item.result_type.clone(),
            if item.browse_id.is_empty() {
                item.video_id.clone()
            } else {
                item.browse_id.clone()
            },
            item.title.clone(),
        );
        if seen.insert(key) {
            target.push(item);
        }
    }
}

fn build_artist_cache(catalog: &[YouTubeItem]) -> Vec<YouTubeCollectionEntry> {
    let mut groups: BTreeMap<String, Vec<&YouTubeItem>> = BTreeMap::new();
    for item in catalog {
        for artist in credited_artists(&item.artist) {
            groups.entry(artist).or_default().push(item);
        }
    }

    groups
        .into_iter()
        .map(|(artist, items)| {
            let albums = items
                .iter()
                .map(|item| item.album.as_str())
                .filter(|album| !album.is_empty())
                .collect::<BTreeSet<_>>()
                .len();
            let cover_path = items
                .iter()
                .find_map(|item| item.cached_cover())
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_default();
            let source = YouTubeItem {
                result_type: "artist".to_string(),
                title: artist.clone(),
                artist: artist.clone(),
                ..YouTubeItem::default()
            };
            YouTubeCollectionEntry {
                title: artist,
                subtitle: format!("{albums} álbuns"),
                detail: format!("YouTube Music • {} faixas", items.len()),
                cover_path,
                item_count: items.len(),
                source,
            }
        })
        .collect()
}

#[cfg(test)]
mod artist_credit_tests {
    use super::{artist_credit_contains, credited_artists};

    #[test]
    fn splits_explicit_collaboration_separators() {
        assert_eq!(
            credited_artists("Artist A feat. Artist B, Artist C"),
            vec!["Artist A", "Artist B", "Artist C"]
        );
        assert_eq!(
            credited_artists("Artist A / Artist B"),
            vec!["Artist A", "Artist B"]
        );
    }

    #[test]
    fn preserves_bare_ampersand_band_names() {
        assert_eq!(
            credited_artists("Simon & Garfunkel"),
            vec!["Simon & Garfunkel"]
        );
    }

    #[test]
    fn matches_one_artist_inside_a_credit() {
        assert!(artist_credit_contains(
            "Artist A feat. Artist B",
            "Artist B"
        ));
        assert!(!artist_credit_contains(
            "Artist A feat. Artist B",
            "Artist C"
        ));
    }
}

pub fn download_cover(item: &YouTubeItem, url: &str) -> Option<PathBuf> {
    download_cover_sized(item, url, PLAYER_COVER_SIZE)
}

fn download_cover_sized(item: &YouTubeItem, url: &str, size: u32) -> Option<PathBuf> {
    let original = if url.is_empty() {
        item.thumbnail_url.as_str()
    } else {
        url
    };
    if original.is_empty() {
        return None;
    }

    let upgraded = upgrade_thumbnail_url(original, size);
    let cache_root = glib::user_cache_dir()
        .join("nocky")
        .join("youtube")
        .join("covers");
    fs::create_dir_all(&cache_root).ok()?;
    let digest = stable_hash(&upgraded);
    let destination = cache_root.join(format!("{digest:016x}-{size}.cover"));
    if destination.is_file() && fs::metadata(&destination).ok()?.len() > 0 {
        return Some(destination);
    }

    let client = cover_client()?;

    let bytes = fetch_cover_bytes(client, &upgraded).or_else(|| {
        (upgraded != original)
            .then(|| fetch_cover_bytes(client, original))
            .flatten()
    })?;
    let temporary = destination.with_extension("tmp");
    fs::write(&temporary, &bytes).ok()?;
    fs::rename(&temporary, &destination).ok()?;
    Some(destination)
}

fn cover_client() -> Option<&'static reqwest::blocking::Client> {
    COVER_CLIENT
        .get_or_init(|| {
            reqwest::blocking::Client::builder()
                .user_agent("Nocky/0.2.4")
                .timeout(std::time::Duration::from_secs(20))
                .build()
                .ok()
        })
        .as_ref()
}

fn fetch_cover_bytes(client: &reqwest::blocking::Client, url: &str) -> Option<Vec<u8>> {
    let response = client.get(url).send().ok()?;
    if !response.status().is_success() {
        return None;
    }
    let bytes = response.bytes().ok()?;
    (!bytes.is_empty()).then(|| bytes.to_vec())
}

pub fn upgrade_thumbnail_url(url: &str, size: u32) -> String {
    let url = url.trim();
    if url.is_empty() {
        return String::new();
    }

    let mut output = url.to_string();
    let path_end = output.find(['?', '#']).unwrap_or(output.len());
    let slash = output[..path_end].rfind('/').unwrap_or(0);
    if let Some(relative_equal) = output[slash..path_end].rfind('=') {
        let equal = slash + relative_equal;
        let suffix = &output[equal + 1..path_end];
        let replacement = if let Some(rest) = suffix.strip_prefix('s') {
            let digits = rest
                .chars()
                .take_while(|character| character.is_ascii_digit())
                .count();
            (digits > 0).then(|| format!("s{}{}", size, &rest[digits..]))
        } else if let Some(rest) = suffix.strip_prefix('w') {
            let width_digits = rest
                .chars()
                .take_while(|character| character.is_ascii_digit())
                .count();
            let after_width = &rest[width_digits..];
            if width_digits > 0 {
                if let Some(height) = after_width.strip_prefix("-h") {
                    let height_digits = height
                        .chars()
                        .take_while(|character| character.is_ascii_digit())
                        .count();
                    (height_digits > 0)
                        .then(|| format!("w{0}-h{0}{1}", size, &height[height_digits..]))
                } else {
                    Some(format!("w{}{}", size, after_width))
                }
            } else {
                None
            }
        } else {
            None
        };

        if let Some(replacement) = replacement {
            output.replace_range(equal + 1..path_end, &replacement);
            return output;
        }
    }

    if output.contains("googleusercontent.com") {
        output.insert_str(path_end, &format!("=s{size}"));
    }
    output
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum YouTubeCacheFreshness {
    Fresh,
    Revalidate,
    StaleDetails,
}

fn unix_now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn cache_age_seconds(saved_at: u64, now: u64) -> u64 {
    now.saturating_sub(saved_at)
}

fn youtube_cache_freshness(saved_at: u64, now: u64) -> YouTubeCacheFreshness {
    let age = cache_age_seconds(saved_at, now);
    if saved_at == 0 || age >= YOUTUBE_CACHE_REVALIDATE_SECS {
        YouTubeCacheFreshness::StaleDetails
    } else if age >= YOUTUBE_CACHE_DETAIL_TTL_SECS {
        YouTubeCacheFreshness::Revalidate
    } else {
        YouTubeCacheFreshness::Fresh
    }
}

fn cache_is_expired(saved_at: u64, now: u64, ttl: u64) -> bool {
    saved_at == 0 || cache_age_seconds(saved_at, now) >= ttl
}

fn rekey_collection_map<'a, T>(
    mut map: HashMap<String, T>,
    entries: impl IntoIterator<Item = &'a YouTubeCollectionEntry>,
) -> HashMap<String, T> {
    for entry in entries {
        let legacy = youtube_collection_legacy_key(&entry.source);
        let stable = youtube_collection_cache_key(&entry.source);
        if legacy == stable || map.contains_key(&stable) {
            continue;
        }
        if let Some(value) = map.remove(&legacy) {
            map.insert(stable, value);
        }
    }
    map
}

pub fn load_library_cache() -> YouTubeLibraryCache {
    let path = youtube_library_cache_path();
    let Ok(raw) = fs::read_to_string(path) else {
        {
            set_youtube_cache_visual_state(YouTubeCacheVisualState::Hidden);
            return YouTubeLibraryCache::default();
        }
    };
    let Ok(mut cache) = serde_json::from_str::<PersistedYouTubeLibraryCache>(&raw) else {
        {
            set_youtube_cache_visual_state(YouTubeCacheVisualState::Hidden);
            return YouTubeLibraryCache::default();
        }
    };
    if !matches!(
        cache.version,
        LIBRARY_CACHE_COMPAT_VERSION | LIBRARY_CACHE_VERSION
    ) {
        eprintln!(
            "Ignoring incompatible YouTube library cache version {} (expected {} or {})",
            cache.version, LIBRARY_CACHE_COMPAT_VERSION, LIBRARY_CACHE_VERSION
        );
        {
            set_youtube_cache_visual_state(YouTubeCacheVisualState::Hidden);
            return YouTubeLibraryCache::default();
        }
    }

    let now = unix_now_seconds();
    let freshness = youtube_cache_freshness(cache.saved_at, now);
    set_youtube_cache_visual_state(match freshness {
        YouTubeCacheFreshness::Fresh => YouTubeCacheVisualState::Fresh,
        YouTubeCacheFreshness::Revalidate | YouTubeCacheFreshness::StaleDetails => {
            YouTubeCacheVisualState::Stale
        }
    });
    let suggestions_expired =
        cache_is_expired(cache.saved_at, now, YOUTUBE_CACHE_SUGGESTION_TTL_SECS);

    if suggestions_expired {
        cache.suggested_albums.clear();
        cache.suggested_artists.clear();
    }

    if freshness != YouTubeCacheFreshness::Fresh {
        cache.playlist_tracks.clear();
        cache.collection_tracks.clear();
        cache.artist_profiles.clear();
        cache.artist_albums.clear();
        cache.albums.clear();
        cache.artists.clear();
    }

    if freshness == YouTubeCacheFreshness::StaleDetails {
        eprintln!(
            "Loaded stale YouTube library metadata as an offline fallback; remote details will be revalidated"
        );
    }

    let album_entries = cache.albums.clone();
    let artist_entries = cache.artists.clone();
    let cacheable_playlists = cache
        .playlists
        .iter()
        .filter(|item| cacheable_youtube_playlist(item))
        .map(|item| item.browse_id.clone())
        .collect::<HashSet<_>>();
    let playlist_tracks = cache
        .playlist_tracks
        .into_iter()
        .filter(|(browse_id, items)| cacheable_playlists.contains(browse_id) && !items.is_empty())
        .collect();
    let collection_tracks = rekey_collection_map(
        cache.collection_tracks,
        album_entries.iter().chain(artist_entries.iter()),
    )
    .into_iter()
    .filter(|(_, items)| !items.is_empty())
    .collect();
    let artist_profiles = rekey_collection_map(cache.artist_profiles, artist_entries.iter());
    let artist_albums = rekey_collection_map(cache.artist_albums, artist_entries.iter());

    let mut library = YouTubeLibraryCache {
        search: Default::default(),
        connected: false,
        syncing: false,
        // Keep this false so a connected account refreshes silently in the background.
        synced: false,
        library: cache.library,
        liked: cache.liked,
        recently_played: cache.recently_played,
        playlists: cache.playlists,
        suggested_albums: cache.suggested_albums,
        suggested_artists: cache.suggested_artists,
        playlist_tracks,
        playlist_loading: HashSet::new(),
        collection_tracks,
        collection_loading: HashSet::new(),
        artist_profiles,
        artist_albums,
        artist_loading: HashSet::new(),
        albums: cache.albums,
        artists: cache.artists,
    };
    let repaired_covers = library.repair_recent_cover_paths();
    if library.albums.is_empty() || library.artists.is_empty() {
        library.rebuild_collections();
    }
    if repaired_covers
        || cache.version < LIBRARY_CACHE_VERSION
        || freshness != YouTubeCacheFreshness::Fresh
        || suggestions_expired
    {
        let _ = save_library_cache(&library);
    }
    library
}

pub fn save_library_cache(cache: &YouTubeLibraryCache) -> Result<(), String> {
    let lock = LIBRARY_CACHE_WRITE_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock
        .lock()
        .map_err(|_| "Could not lock the YouTube library cache writer".to_string())?;
    let path = youtube_library_cache_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create the YouTube cache folder: {error}"))?;
    }
    let saved_at = unix_now_seconds();
    let cacheable_playlists = cache
        .playlists
        .iter()
        .filter(|item| cacheable_youtube_playlist(item))
        .map(|item| item.browse_id.clone())
        .collect::<HashSet<_>>();
    let playlist_tracks = cache
        .playlist_tracks
        .iter()
        .filter(|(browse_id, items)| cacheable_playlists.contains(*browse_id) && !items.is_empty())
        .map(|(browse_id, items)| (browse_id.clone(), items.clone()))
        .collect();
    let collection_tracks = cache
        .collection_tracks
        .iter()
        .filter(|(_, items)| !items.is_empty())
        .map(|(key, items)| (key.clone(), items.clone()))
        .collect();
    let payload = PersistedYouTubeLibraryCache {
        version: LIBRARY_CACHE_VERSION,
        saved_at,
        library: cache.library.clone(),
        liked: cache.liked.clone(),
        recently_played: cache.recently_played.clone(),
        playlists: cache.playlists.clone(),
        suggested_albums: cache.suggested_albums.clone(),
        suggested_artists: cache.suggested_artists.clone(),
        playlist_tracks,
        collection_tracks,
        artist_profiles: cache.artist_profiles.clone(),
        artist_albums: cache.artist_albums.clone(),
        albums: cache.albums.clone(),
        artists: cache.artists.clone(),
    };
    let serialized = serde_json::to_vec(&payload)
        .map_err(|error| format!("Could not serialize the YouTube library cache: {error}"))?;
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, serialized)
        .map_err(|error| format!("Could not write the YouTube library cache: {error}"))?;
    fs::rename(&temporary, &path)
        .map_err(|error| format!("Could not replace the YouTube library cache: {error}"))?;
    Ok(())
}

pub fn queue_library_cache_save(cache: &YouTubeLibraryCache) -> Result<(), String> {
    let snapshot = cache.clone();
    if let Some(sender) = library_cache_writer() {
        if sender.send(snapshot.clone()).is_ok() {
            return Ok(());
        }
    }

    save_library_cache(&snapshot)
}

fn library_cache_writer() -> Option<&'static Sender<YouTubeLibraryCache>> {
    LIBRARY_CACHE_WRITER
        .get_or_init(|| {
            let (sender, receiver) = mpsc::channel::<YouTubeLibraryCache>();
            let worker = thread::Builder::new()
                .name("nocky-youtube-cache-writer".to_string())
                .spawn(move || {
                    while let Ok(mut snapshot) = receiver.recv() {
                        loop {
                            match receiver.recv_timeout(Duration::from_millis(180)) {
                                Ok(newer) => snapshot = newer,
                                Err(RecvTimeoutError::Timeout) => break,
                                Err(RecvTimeoutError::Disconnected) => {
                                    if let Err(error) = save_library_cache(&snapshot) {
                                        eprintln!(
                                            "Could not save the YouTube library cache: {error}"
                                        );
                                    }
                                    return;
                                }
                            }
                        }

                        if let Err(error) = save_library_cache(&snapshot) {
                            eprintln!("Could not save the YouTube library cache: {error}");
                        }
                    }
                });

            match worker {
                Ok(_) => Some(sender),
                Err(error) => {
                    eprintln!("Could not start the YouTube cache writer: {error}");
                    None
                }
            }
        })
        .as_ref()
}

pub fn clear_library_cache() {
    let _ = fs::remove_file(youtube_library_cache_path());
}

fn youtube_library_cache_path() -> PathBuf {
    glib::user_cache_dir()
        .join("nocky")
        .join("youtube")
        .join("library-cache.json")
}

fn stable_hash(value: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn helper_path() -> Option<PathBuf> {
    if let Some(path) = env::var_os("NOCKY_YOUTUBE_HELPER").map(PathBuf::from) {
        if path.is_file() {
            eprintln!(
                "Nocky YouTube helper selected from NOCKY_YOUTUBE_HELPER: {}",
                path.display()
            );
            return Some(path);
        }
    }

    let source_helper = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("helpers/nocky_youtube.py");
    let mut candidates = Vec::new();

    // Development builds must exercise the helper from the current checkout
    // before falling back to installed copies.
    if cfg!(debug_assertions) {
        candidates.push(source_helper.clone());
    }

    if let Ok(executable) = env::current_exe() {
        if let Some(prefix) = executable.parent().and_then(Path::parent) {
            candidates.push(prefix.join("share/nocky/helpers/nocky_youtube.py"));
        }
    }
    candidates.push(
        glib::user_data_dir()
            .join("nocky")
            .join("helpers")
            .join("nocky_youtube.py"),
    );
    candidates.push(PathBuf::from(
        "/usr/local/share/nocky/helpers/nocky_youtube.py",
    ));
    candidates.push(PathBuf::from("/usr/share/nocky/helpers/nocky_youtube.py"));

    if !cfg!(debug_assertions) {
        candidates.push(source_helper);
    }

    let selected = candidates.into_iter().find(|path| path.is_file());
    if let Some(path) = selected.as_ref() {
        eprintln!("Nocky YouTube helper selected: {}", path.display());
    }
    selected
}

fn python_path() -> Option<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(path) = env::var_os("NOCKY_PYTHON").map(PathBuf::from) {
        candidates.push(path);
    }
    if let Ok(executable) = env::current_exe() {
        if let Some(prefix) = executable.parent().and_then(Path::parent) {
            candidates.push(prefix.join("share/nocky/runtime/bin/python3"));
            candidates.push(prefix.join("share/nocky/runtime/bin/python"));
        }
    }

    let user_runtime = glib::user_data_dir()
        .join("nocky")
        .join("runtime")
        .join("bin");
    candidates.push(user_runtime.join("python3"));
    candidates.push(user_runtime.join("python"));
    candidates.push(PathBuf::from("/usr/local/share/nocky/runtime/bin/python3"));
    candidates.push(PathBuf::from("/usr/local/share/nocky/runtime/bin/python"));
    candidates.push(PathBuf::from("/usr/share/nocky/runtime/bin/python3"));
    candidates.push(PathBuf::from("/usr/share/nocky/runtime/bin/python"));

    let development_runtime = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".nocky-runtime/bin");
    candidates.push(development_runtime.join("python3"));
    candidates.push(development_runtime.join("python"));

    if let Some(system_python) = find_in_path("python3") {
        candidates.push(system_python);
    }

    candidates
        .into_iter()
        .filter(|path| path.is_file())
        .find(|path| python_supports_youtube(path))
}

fn python_supports_youtube(path: &Path) -> bool {
    Command::new(path)
        .args(["-c", "import requests, ytmusicapi, yt_dlp"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn find_in_path(name: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
}

#[cfg(test)]
mod youtube_cache_expiration_tests {
    use super::*;

    #[test]
    fn fresh_cache_keeps_remote_details() {
        let now = 100_000;
        assert_eq!(
            youtube_cache_freshness(now - 60, now),
            YouTubeCacheFreshness::Fresh
        );
        assert!(!cache_is_expired(
            now - 60,
            now,
            YOUTUBE_CACHE_SUGGESTION_TTL_SECS
        ));
    }

    #[test]
    fn detail_cache_revalidates_after_six_hours() {
        let now = 100_000;
        assert_eq!(
            youtube_cache_freshness(now - YOUTUBE_CACHE_DETAIL_TTL_SECS, now),
            YouTubeCacheFreshness::Revalidate
        );
    }

    #[test]
    fn old_cache_preserves_core_metadata_but_drops_remote_details() {
        let now = 200_000;
        assert_eq!(
            youtube_cache_freshness(now - YOUTUBE_CACHE_REVALIDATE_SECS, now),
            YouTubeCacheFreshness::StaleDetails
        );
    }

    #[test]
    fn missing_timestamp_is_always_stale() {
        assert_eq!(
            youtube_cache_freshness(0, 200_000),
            YouTubeCacheFreshness::StaleDetails
        );
        assert!(cache_is_expired(
            0,
            200_000,
            YOUTUBE_CACHE_SUGGESTION_TTL_SECS
        ));
    }

    #[test]
    fn future_timestamp_does_not_underflow() {
        let now = 200_000;
        assert_eq!(cache_age_seconds(now + 100, now), 0);
        assert_eq!(
            youtube_cache_freshness(now + 100, now),
            YouTubeCacheFreshness::Fresh
        );
    }

    #[test]
    fn suggestions_expire_after_twelve_hours() {
        let now = 200_000;
        assert!(cache_is_expired(
            now - YOUTUBE_CACHE_SUGGESTION_TTL_SECS,
            now,
            YOUTUBE_CACHE_SUGGESTION_TTL_SECS
        ));
    }
}

#[cfg(test)]
mod youtube_incremental_sync_tests {
    use super::*;

    fn song(video_id: &str, title: &str, cover_path: &str) -> YouTubeItem {
        YouTubeItem {
            result_type: "song".to_string(),
            video_id: video_id.to_string(),
            title: title.to_string(),
            artist: "Artist".to_string(),
            cover_path: cover_path.to_string(),
            ..YouTubeItem::default()
        }
    }

    #[test]
    fn preserves_cached_cover_for_unchanged_identity() {
        let previous = vec![song("video-1", "Old title", "/tmp/cover.jpg")];
        let incoming = vec![song("video-1", "New title", "")];

        let (merged, changes) = merge_youtube_items(&previous, incoming);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].cover_path, "/tmp/cover.jpg");
        assert_eq!(merged[0].title, "New title");
        assert_eq!(changes.updated, 1);
        assert_eq!(changes.added, 0);
        assert_eq!(changes.removed, 0);
    }

    #[test]
    fn detects_added_and_removed_items() {
        let previous = vec![
            song("video-1", "One", "/tmp/one.jpg"),
            song("video-2", "Two", "/tmp/two.jpg"),
        ];
        let incoming = vec![song("video-2", "Two", ""), song("video-3", "Three", "")];

        let (merged, changes) = merge_youtube_items(&previous, incoming);

        assert_eq!(merged.len(), 2);
        assert_eq!(changes.added, 1);
        assert_eq!(changes.removed, 1);
    }

    #[test]
    fn removes_duplicate_items_from_incoming_snapshot() {
        let incoming = vec![song("video-1", "One", ""), song("video-1", "Duplicate", "")];

        let (merged, changes) = merge_youtube_items(&[], incoming);

        assert_eq!(merged.len(), 1);
        assert_eq!(changes.added, 1);
    }

    #[test]
    fn browse_identity_is_used_when_video_id_is_missing() {
        let item = YouTubeItem {
            result_type: "playlist".to_string(),
            browse_id: "VL_test".to_string(),
            title: "Playlist".to_string(),
            ..YouTubeItem::default()
        };

        assert_eq!(youtube_item_identity(&item), "browse:playlist:VL_test");
    }

    #[test]
    fn no_changes_for_identical_snapshot() {
        let previous = vec![song("video-1", "One", "/tmp/one.jpg")];
        let (merged, changes) = merge_youtube_items(&previous, previous.clone());

        assert_eq!(merged, previous);
        assert!(!changes.changed());
    }
}

#[cfg(test)]
mod stable_collection_identity_tests {
    use super::*;

    fn collection(kind: &str, title: &str, artist: &str, browse_id: &str) -> YouTubeItem {
        YouTubeItem {
            result_type: kind.to_string(),
            title: title.to_string(),
            artist: artist.to_string(),
            browse_id: browse_id.to_string(),
            ..YouTubeItem::default()
        }
    }

    #[test]
    fn browse_id_has_priority_over_display_metadata() {
        let first = collection("album", "Old title", "Artist", "MPREb_same");
        let renamed = collection("album", "New title", "Artist", "MPREb_same");
        assert_eq!(
            youtube_collection_cache_key(&first),
            youtube_collection_cache_key(&renamed)
        );
    }

    #[test]
    fn albums_with_the_same_title_keep_separate_fallback_keys() {
        let first = collection("album", "Greatest Hits", "Artist A", "");
        let second = collection("album", "Greatest Hits", "Artist B", "");
        assert_ne!(
            youtube_collection_cache_key(&first),
            youtube_collection_cache_key(&second)
        );
    }

    #[test]
    fn artists_with_different_browse_ids_never_collide() {
        let first = collection("artist", "The Band", "", "UC_artist_a");
        let second = collection("artist", "The Band", "", "UC_artist_b");
        assert_ne!(
            youtube_collection_cache_key(&first),
            youtube_collection_cache_key(&second)
        );
    }
}

#[cfg(test)]
mod youtube_like_error_tests {
    use super::youtube_like_error_message;

    #[test]
    fn classifies_expired_sessions() {
        assert!(youtube_like_error_message("401 unauthorized session").contains("expirou"));
    }

    #[test]
    fn classifies_offline_failures() {
        assert!(youtube_like_error_message("network timeout").contains("Sem conexão"));
    }

    #[test]
    fn classifies_permission_failures() {
        assert!(youtube_like_error_message("403 forbidden").contains("permissões"));
    }

    #[test]
    fn keeps_generic_fallback() {
        assert_eq!(
            youtube_like_error_message("unexpected backend response"),
            "Não foi possível sincronizar a curtida com o YouTube Music."
        );
    }
}

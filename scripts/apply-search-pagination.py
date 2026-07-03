#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path.cwd()
YOUTUBE_MOD = ROOT / "src/youtube/mod.rs"
SEARCH_CACHE = ROOT / "src/youtube/search_cache.rs"
BACKGROUND_MESSAGE = ROOT / "src/background.rs"
YOUTUBE_CONTROLLER = ROOT / "src/app/controller/youtube.rs"
BACKGROUND_CONTROLLER = ROOT / "src/app/controller/background.rs"
NAVIGATION = ROOT / "src/app/controller/navigation.rs"
BROWSER = ROOT / "src/browser.rs"
HELPER = ROOT / "helpers/nocky_youtube.py"
ROADMAP = ROOT / "ROADMAP.md"
CACHE_DOC = ROOT / "docs/SEARCH_CACHE.md"
PAGINATION_DOC = ROOT / "docs/SEARCH_PAGINATION.md"
HELPER_TEST = ROOT / "tests/test_youtube_search_pagination_helper.py"


class PatchError(RuntimeError):
    pass


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count == 0 and new in text:
        print(f"[already applied] {label}")
        return text
    if count != 1:
        raise PatchError(f"{label}: expected one match, found {count}")
    print(f"[changed] {label}")
    return text.replace(old, new, 1)


def replace_between(text: str, start: str, end: str, replacement: str, label: str) -> str:
    start_index = text.find(start)
    if start_index < 0:
        if replacement in text:
            print(f"[already applied] {label}")
            return text
        raise PatchError(f"{label}: start marker not found")
    end_index = text.find(end, start_index)
    if end_index < 0:
        raise PatchError(f"{label}: end marker not found")
    print(f"[changed] {label}")
    return text[:start_index] + replacement + text[end_index:]


def replace_one_of(text: str, candidates: list[str], new: str, label: str) -> str:
    if new in text:
        print(f"[already applied] {label}")
        return text
    matches = [candidate for candidate in candidates if candidate in text]
    if len(matches) != 1:
        raise PatchError(f"{label}: expected one candidate, found {len(matches)}")
    print(f"[changed] {label}")
    return text.replace(matches[0], new, 1)


SEARCH_RESULTS_BLOCK = r'''#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum YouTubeSearchCategory {
    Songs,
    Albums,
    Artists,
    Playlists,
}

impl YouTubeSearchCategory {
    pub(crate) fn filter(self) -> &'static str {
        match self {
            Self::Songs => "songs",
            Self::Albums => "albums",
            Self::Artists => "artists",
            Self::Playlists => "playlists",
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct YouTubeSearchPage {
    pub items: Vec<YouTubeItem>,
    pub continuation: String,
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
    pub songs_continuation: String,
    pub albums_continuation: String,
    pub artists_continuation: String,
    pub playlists_continuation: String,
    pub songs_loading_more: bool,
    pub albums_loading_more: bool,
    pub artists_loading_more: bool,
    pub playlists_loading_more: bool,
}

'''

SEARCH_RESULTS_IMPL = r'''impl YouTubeSearchResults {
    fn items_mut(&mut self, category: YouTubeSearchCategory) -> &mut Vec<YouTubeItem> {
        match category {
            YouTubeSearchCategory::Songs => &mut self.songs,
            YouTubeSearchCategory::Albums => &mut self.albums,
            YouTubeSearchCategory::Artists => &mut self.artists,
            YouTubeSearchCategory::Playlists => &mut self.playlists,
        }
    }

    fn continuation_mut(&mut self, category: YouTubeSearchCategory) -> &mut String {
        match category {
            YouTubeSearchCategory::Songs => &mut self.songs_continuation,
            YouTubeSearchCategory::Albums => &mut self.albums_continuation,
            YouTubeSearchCategory::Artists => &mut self.artists_continuation,
            YouTubeSearchCategory::Playlists => &mut self.playlists_continuation,
        }
    }

    pub(crate) fn continuation(&self, category: YouTubeSearchCategory) -> &str {
        match category {
            YouTubeSearchCategory::Songs => &self.songs_continuation,
            YouTubeSearchCategory::Albums => &self.albums_continuation,
            YouTubeSearchCategory::Artists => &self.artists_continuation,
            YouTubeSearchCategory::Playlists => &self.playlists_continuation,
        }
    }

    pub(crate) fn loading_more(&self, category: YouTubeSearchCategory) -> bool {
        match category {
            YouTubeSearchCategory::Songs => self.songs_loading_more,
            YouTubeSearchCategory::Albums => self.albums_loading_more,
            YouTubeSearchCategory::Artists => self.artists_loading_more,
            YouTubeSearchCategory::Playlists => self.playlists_loading_more,
        }
    }

    pub(crate) fn set_loading_more(
        &mut self,
        category: YouTubeSearchCategory,
        loading: bool,
    ) {
        match category {
            YouTubeSearchCategory::Songs => self.songs_loading_more = loading,
            YouTubeSearchCategory::Albums => self.albums_loading_more = loading,
            YouTubeSearchCategory::Artists => self.artists_loading_more = loading,
            YouTubeSearchCategory::Playlists => self.playlists_loading_more = loading,
        }
    }

    pub(crate) fn replace_page(
        &mut self,
        category: YouTubeSearchCategory,
        page: YouTubeSearchPage,
    ) {
        let YouTubeSearchPage {
            items,
            continuation,
        } = page;
        *self.items_mut(category) = items;
        *self.continuation_mut(category) = continuation;
        self.set_loading_more(category, false);
    }

    pub(crate) fn append_page(
        &mut self,
        category: YouTubeSearchCategory,
        page: YouTubeSearchPage,
    ) -> usize {
        let YouTubeSearchPage {
            items,
            continuation,
        } = page;
        let added = {
            let target = self.items_mut(category);
            let before = target.len();
            append_unique_search_items(target, items);
            target.len().saturating_sub(before)
        };
        *self.continuation_mut(category) = continuation;
        self.set_loading_more(category, false);
        added
    }

    pub(crate) fn clear_transient_state(&mut self) {
        self.loading = false;
        self.error.clear();
        self.songs_loading_more = false;
        self.albums_loading_more = false;
        self.artists_loading_more = false;
        self.playlists_loading_more = false;
    }

    pub(crate) fn merge_cached_results(&mut self, cached: &Self) {
        append_unique_search_items(&mut self.songs, cached.songs.clone());
        append_unique_search_items(&mut self.albums, cached.albums.clone());
        append_unique_search_items(&mut self.artists, cached.artists.clone());
        append_unique_search_items(&mut self.playlists, cached.playlists.clone());
    }
}

'''

SEARCH_CACHE_SOURCE = r'''use super::{YouTubeSearchCategory, YouTubeSearchPage, YouTubeSearchResults};
use crate::search_text::normalize_search_text;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

const SEARCH_CACHE_FRESH_TTL: Duration = Duration::from_secs(10 * 60);
const SEARCH_CACHE_STALE_TTL: Duration = Duration::from_secs(60 * 60);
const SEARCH_CACHE_CAPACITY: usize = 32;

#[derive(Clone, Debug)]
struct YouTubeSearchCacheEntry {
    results: YouTubeSearchResults,
    stored_at: Instant,
    last_accessed: Instant,
}

#[derive(Clone, Debug)]
pub(crate) enum YouTubeSearchCacheLookup {
    Miss,
    Fresh(YouTubeSearchResults),
    Stale(YouTubeSearchResults),
}

#[derive(Debug)]
pub(crate) struct YouTubeSearchCache {
    entries: HashMap<String, YouTubeSearchCacheEntry>,
    fresh_ttl: Duration,
    stale_ttl: Duration,
    capacity: usize,
}

impl Default for YouTubeSearchCache {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            fresh_ttl: SEARCH_CACHE_FRESH_TTL,
            stale_ttl: SEARCH_CACHE_STALE_TTL,
            capacity: SEARCH_CACHE_CAPACITY,
        }
    }
}

impl YouTubeSearchCache {
    pub(crate) fn lookup(&mut self, raw_query: &str) -> YouTubeSearchCacheLookup {
        self.lookup_at(raw_query, Instant::now())
    }

    pub(crate) fn insert(&mut self, raw_query: &str, results: YouTubeSearchResults) {
        self.insert_at(raw_query, results, Instant::now());
    }

    pub(crate) fn append_page(
        &mut self,
        raw_query: &str,
        category: YouTubeSearchCategory,
        page: YouTubeSearchPage,
    ) -> bool {
        self.append_page_at(raw_query, category, page, Instant::now())
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }

    fn lookup_at(&mut self, raw_query: &str, now: Instant) -> YouTubeSearchCacheLookup {
        let key = normalize_search_text(raw_query);
        if key.is_empty() {
            return YouTubeSearchCacheLookup::Miss;
        }

        self.prune_expired(now);
        let Some(age) = self
            .entries
            .get(&key)
            .map(|entry| now.saturating_duration_since(entry.stored_at))
        else {
            return YouTubeSearchCacheLookup::Miss;
        };

        if age > self.stale_ttl {
            self.entries.remove(&key);
            return YouTubeSearchCacheLookup::Miss;
        }

        let entry = self
            .entries
            .get_mut(&key)
            .expect("search cache entry should exist after age lookup");
        entry.last_accessed = now;
        let results = entry.results.clone();

        if age <= self.fresh_ttl {
            YouTubeSearchCacheLookup::Fresh(results)
        } else {
            YouTubeSearchCacheLookup::Stale(results)
        }
    }

    fn insert_at(&mut self, raw_query: &str, mut results: YouTubeSearchResults, now: Instant) {
        let key = normalize_search_text(raw_query);
        if key.is_empty() {
            return;
        }

        results.query = raw_query.trim().to_string();
        results.clear_transient_state();
        self.prune_expired(now);
        self.entries.insert(
            key,
            YouTubeSearchCacheEntry {
                results,
                stored_at: now,
                last_accessed: now,
            },
        );
        self.enforce_capacity();
    }

    fn append_page_at(
        &mut self,
        raw_query: &str,
        category: YouTubeSearchCategory,
        page: YouTubeSearchPage,
        now: Instant,
    ) -> bool {
        let key = normalize_search_text(raw_query);
        if key.is_empty() {
            return false;
        }

        self.prune_expired(now);
        let Some(entry) = self.entries.get_mut(&key) else {
            return false;
        };
        entry.results.append_page(category, page);
        entry.results.clear_transient_state();
        entry.stored_at = now;
        entry.last_accessed = now;
        true
    }

    fn prune_expired(&mut self, now: Instant) {
        let stale_ttl = self.stale_ttl;
        self.entries.retain(|_, entry| {
            now.saturating_duration_since(entry.stored_at) <= stale_ttl
        });
    }

    fn enforce_capacity(&mut self) {
        while self.entries.len() > self.capacity {
            let Some(oldest_key) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_accessed)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            self.entries.remove(&oldest_key);
        }
    }

    #[cfg(test)]
    fn with_policy(fresh_ttl: Duration, stale_ttl: Duration, capacity: usize) -> Self {
        Self {
            entries: HashMap::new(),
            fresh_ttl,
            stale_ttl,
            capacity,
        }
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::youtube::YouTubeItem;

    fn results(query: &str, title: &str) -> YouTubeSearchResults {
        YouTubeSearchResults {
            query: query.to_string(),
            loading: true,
            error: "temporary error".to_string(),
            songs: vec![YouTubeItem {
                result_type: "song".to_string(),
                title: title.to_string(),
                video_id: format!("video-{title}"),
                ..YouTubeItem::default()
            }],
            songs_continuation: "page-2".to_string(),
            ..YouTubeSearchResults::default()
        }
    }

    #[test]
    fn normalized_queries_move_from_fresh_to_stale_then_expire() {
        let start = Instant::now();
        let mut cache = YouTubeSearchCache::with_policy(
            Duration::from_secs(10),
            Duration::from_secs(30),
            4,
        );
        cache.insert_at("  Daft   Punk ", results("Daft Punk", "One More Time"), start);

        let fresh = cache.lookup_at("daft punk", start + Duration::from_secs(5));
        assert!(matches!(fresh, YouTubeSearchCacheLookup::Fresh(_)));

        let stale = cache.lookup_at("DAFT PUNK", start + Duration::from_secs(15));
        assert!(matches!(stale, YouTubeSearchCacheLookup::Stale(_)));

        let expired = cache.lookup_at("daft punk", start + Duration::from_secs(31));
        assert!(matches!(expired, YouTubeSearchCacheLookup::Miss));
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn cache_strips_transient_loading_and_error_state() {
        let start = Instant::now();
        let mut cache = YouTubeSearchCache::with_policy(
            Duration::from_secs(10),
            Duration::from_secs(30),
            4,
        );
        let mut pending = results("Muse", "Hysteria");
        pending.songs_loading_more = true;
        cache.insert_at("Muse", pending, start);

        let YouTubeSearchCacheLookup::Fresh(cached) = cache.lookup_at("muse", start) else {
            panic!("expected a fresh cache hit");
        };
        assert!(!cached.loading);
        assert!(cached.error.is_empty());
        assert!(!cached.songs_loading_more);
    }

    #[test]
    fn least_recently_used_entry_is_evicted_at_capacity() {
        let start = Instant::now();
        let mut cache = YouTubeSearchCache::with_policy(
            Duration::from_secs(60),
            Duration::from_secs(120),
            2,
        );
        cache.insert_at("first", results("first", "First"), start);
        cache.insert_at(
            "second",
            results("second", "Second"),
            start + Duration::from_secs(1),
        );
        let _ = cache.lookup_at("first", start + Duration::from_secs(2));
        cache.insert_at(
            "third",
            results("third", "Third"),
            start + Duration::from_secs(3),
        );

        assert!(matches!(
            cache.lookup_at("second", start + Duration::from_secs(4)),
            YouTubeSearchCacheLookup::Miss
        ));
        assert!(matches!(
            cache.lookup_at("first", start + Duration::from_secs(4)),
            YouTubeSearchCacheLookup::Fresh(_)
        ));
        assert!(matches!(
            cache.lookup_at("third", start + Duration::from_secs(4)),
            YouTubeSearchCacheLookup::Fresh(_)
        ));
    }

    #[test]
    fn appending_a_remote_page_deduplicates_and_refreshes_the_continuation() {
        let start = Instant::now();
        let mut cache = YouTubeSearchCache::with_policy(
            Duration::from_secs(60),
            Duration::from_secs(120),
            4,
        );
        cache.insert_at("Muse", results("Muse", "Hysteria"), start);
        assert!(cache.append_page_at(
            "muse",
            YouTubeSearchCategory::Songs,
            YouTubeSearchPage {
                items: vec![
                    YouTubeItem {
                        result_type: "song".to_string(),
                        title: "Hysteria".to_string(),
                        video_id: "video-Hysteria".to_string(),
                        ..YouTubeItem::default()
                    },
                    YouTubeItem {
                        result_type: "song".to_string(),
                        title: "Starlight".to_string(),
                        video_id: "video-Starlight".to_string(),
                        ..YouTubeItem::default()
                    },
                ],
                continuation: "page-3".to_string(),
            },
            start + Duration::from_secs(5),
        ));

        let YouTubeSearchCacheLookup::Fresh(cached) =
            cache.lookup_at("muse", start + Duration::from_secs(6))
        else {
            panic!("expected a fresh cache hit");
        };
        assert_eq!(cached.songs.len(), 2);
        assert_eq!(cached.songs_continuation, "page-3");
    }
}
'''

REQUEST_GLOBAL_SEARCH = r'''    pub(crate) fn request_global_youtube_search(&self, query: String) {
        let query = query.trim().to_string();
        if query.is_empty()
            || self.config.borrow().startup_source != Some(StartupSource::YouTube)
            || self.search_query.borrow().trim() != query.as_str()
        {
            return;
        }

        // Increment before consulting the cache so a fresh cache hit also
        // invalidates any older in-flight response for another query.
        let request_id = self.youtube_search_request_id.get().wrapping_add(1);
        self.youtube_search_request_id.set(request_id);

        let local_results = self.youtube_library.borrow().cached_search_results(&query);
        match self.youtube_search_cache.borrow_mut().lookup(&query) {
            YouTubeSearchCacheLookup::Fresh(mut cached) => {
                cached.query = query;
                cached.loading = false;
                cached.error.clear();
                cached.merge_cached_results(&local_results);
                self.youtube_library.borrow_mut().search = cached;
                self.refresh_browser();
                return;
            }
            YouTubeSearchCacheLookup::Stale(mut cached) => {
                cached.query = query.clone();
                cached.loading = true;
                cached.error.clear();
                cached.merge_cached_results(&local_results);
                self.youtube_library.borrow_mut().search = cached;
            }
            YouTubeSearchCacheLookup::Miss => {
                let mut cached = local_results;
                cached.query = query.clone();
                cached.loading = true;
                cached.error.clear();
                self.youtube_library.borrow_mut().search = cached;
            }
        }
        self.refresh_browser();

        let Some(bridge) = self.youtube_bridge.clone() else {
            let mut library = self.youtube_library.borrow_mut();
            let mut visible = library.search.clone();
            visible.loading = false;
            visible.error = "As dependências do YouTube Music não estão instaladas".to_string();
            library.search = visible;
            drop(library);
            self.refresh_browser();
            return;
        };

        let sender = self.background.sender();
        thread::spawn(move || {
            let categories = [
                YouTubeSearchCategory::Songs,
                YouTubeSearchCategory::Albums,
                YouTubeSearchCategory::Artists,
                YouTubeSearchCategory::Playlists,
            ];
            let expected = categories.len();
            let (result_tx, result_rx) = mpsc::channel();
            let mut workers = Vec::with_capacity(expected);

            for category in categories {
                let bridge = bridge.clone();
                let result_tx = result_tx.clone();
                let worker_query = query.clone();
                workers.push(thread::spawn(move || {
                    let result = bridge.search_page(&worker_query, category, "");
                    let _ = result_tx.send((category, result));
                }));
            }
            drop(result_tx);

            let mut categorized = YouTubeSearchResults {
                query: query.clone(),
                ..YouTubeSearchResults::default()
            };
            let mut errors = Vec::new();

            for (category, result) in result_rx {
                match result {
                    Ok(mut page) => {
                        if category == YouTubeSearchCategory::Songs {
                            page.items.retain(YouTubeItem::playable);
                        }
                        categorized.replace_page(category, page);
                    }
                    Err(error) => errors.push(format!("{}: {error}", category.filter())),
                }
            }

            for worker in workers {
                let _ = worker.join();
            }

            let result = if errors.len() == expected {
                Err(errors.join(" | "))
            } else {
                if !errors.is_empty() {
                    categorized.error = errors.join(" | ");
                }
                Ok(categorized)
            };

            let _ = sender.send(BackgroundMessage::YouTubeGlobalSearch {
                request_id,
                query,
                result,
            });
        });
    }

    pub(crate) fn load_more_youtube_search(&self, category: YouTubeSearchCategory) {
        let query = self.search_query.borrow().trim().to_string();
        if query.is_empty() || self.config.borrow().startup_source != Some(StartupSource::YouTube) {
            return;
        }

        let continuation = {
            let mut library = self.youtube_library.borrow_mut();
            if !library.search.query.eq_ignore_ascii_case(&query)
                || library.search.loading
                || library.search.loading_more(category)
            {
                return;
            }
            let continuation = library.search.continuation(category).trim().to_string();
            if continuation.is_empty() {
                return;
            }
            library.search.set_loading_more(category, true);
            continuation
        };
        self.refresh_browser();

        let Some(bridge) = self.youtube_bridge.clone() else {
            let mut library = self.youtube_library.borrow_mut();
            library.search.set_loading_more(category, false);
            library.search.error =
                "As dependências do YouTube Music não estão instaladas".to_string();
            drop(library);
            self.refresh_browser();
            return;
        };

        let request_id = self.youtube_search_request_id.get();
        let sender = self.background.sender();
        thread::spawn(move || {
            let result = bridge.search_page(&query, category, &continuation);
            let _ = sender.send(BackgroundMessage::YouTubeSearchPageLoaded {
                request_id,
                query,
                category,
                result,
            });
        });
    }

'''

GLOBAL_SEARCH_HANDLERS = r'''                BackgroundMessage::YouTubeGlobalSearch {
                    request_id,
                    query,
                    result,
                } => {
                    if request_id != self.youtube_search_request_id.get()
                        || self.search_query.borrow().trim() != query.as_str()
                        || self.config.borrow().startup_source != Some(StartupSource::YouTube)
                    {
                        continue;
                    }

                    let mut cache_snapshot = None;
                    let mut library = self.youtube_library.borrow_mut();
                    match result {
                        Ok(mut categorized) => {
                            // Cache only the remote response. Current library-derived
                            // matches are merged at read time so removed local data
                            // cannot linger inside the query cache.
                            cache_snapshot = Some(categorized.clone());
                            categorized.merge_cached_results(&library.search);
                            categorized.loading = false;
                            library.search = categorized;
                        }
                        Err(error) => {
                            let mut cached = library.search.clone();
                            cached.query = query.clone();
                            cached.loading = false;
                            cached.error = error;
                            library.search = cached;
                        }
                    }
                    drop(library);

                    if let Some(results) = cache_snapshot {
                        self.youtube_search_cache
                            .borrow_mut()
                            .insert(&query, results);
                    }
                    self.refresh_browser();
                }
                BackgroundMessage::YouTubeSearchPageLoaded {
                    request_id,
                    query,
                    category,
                    result,
                } => {
                    if request_id != self.youtube_search_request_id.get()
                        || self.search_query.borrow().trim() != query.as_str()
                        || self.config.borrow().startup_source != Some(StartupSource::YouTube)
                    {
                        continue;
                    }

                    let mut cache_page = None;
                    let mut library = self.youtube_library.borrow_mut();
                    match result {
                        Ok(mut page) => {
                            if category == YouTubeSearchCategory::Songs {
                                page.items.retain(YouTubeItem::playable);
                            }
                            cache_page = Some(page.clone());
                            library.search.append_page(category, page);
                        }
                        Err(error) => {
                            library.search.set_loading_more(category, false);
                            library.search.error =
                                format!("{}: {error}", category.filter());
                        }
                    }
                    drop(library);

                    if let Some(page) = cache_page {
                        if !self.youtube_search_cache.borrow_mut().append_page(
                            &query,
                            category,
                            page,
                        ) {
                            eprintln!(
                                "Search pagination cache entry expired before append: {query}"
                            );
                        }
                    }
                    self.refresh_browser();
                }
'''

SEARCH_LIST_SECTION = r'''#[expect(
    clippy::too_many_arguments,
    reason = "Search sections keep paging, rendering and playback context explicit"
)]
fn search_list_section(
    title: &str,
    empty_message: &str,
    cards: Vec<HomeCard>,
    limit: Rc<Cell<usize>>,
    event_tx: &Sender<BrowserEvent>,
    loading: bool,
    copy: SearchCopy,
    config: &AppConfig,
    playback: &BrowserPlaybackState,
    category: YouTubeSearchCategory,
    continuation: &str,
    loading_more: bool,
) -> gtk::Box {
    let total = cards.len();
    let visible = total.min(limit.get());
    let section = gtk::Box::new(gtk::Orientation::Vertical, 10);
    section.add_css_class("home-section");
    section.add_css_class("search-section-card");
    section.append(&search_section_heading(
        title, visible, total, loading, copy,
    ));

    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::Single);
    list.set_activate_on_single_click(true);
    list.add_css_class("boxed-list");
    list.add_css_class("search-results-list");
    list.add_css_class("search-results-surface");
    list.add_css_class("search-keyboard-list");

    let visible_cards = Rc::new(RefCell::new(Vec::<HomeCard>::new()));
    {
        let sender = event_tx.clone();
        let cards = visible_cards.clone();
        list.connect_row_activated(move |_, row| {
            let Some(card) = cards.borrow().get(row.index() as usize).cloned() else {
                return;
            };
            let _ = sender.send(card.open_event());
        });
    }

    if total == 0 {
        list.append(&empty_row(if loading {
            copy.searching
        } else {
            empty_message
        }));
    } else {
        for card in cards.into_iter().take(visible) {
            let row = search_collection_row(&card, event_tx, config, playback);
            list.append(&row);
            visible_cards.borrow_mut().push(card);
        }
    }
    section.append(&list);

    if total > visible {
        section.append(&search_more_button(
            title,
            total - visible,
            limit,
            event_tx,
            copy,
        ));
    } else if !loading && (!continuation.is_empty() || loading_more) {
        section.append(&search_remote_more_button(
            title,
            category,
            loading_more,
            event_tx,
            copy,
        ));
    }
    section
}

'''

REMOTE_MORE_BUTTON = r'''fn search_remote_more_button(
    category_label: &str,
    category: YouTubeSearchCategory,
    loading: bool,
    event_tx: &Sender<BrowserEvent>,
    copy: SearchCopy,
) -> gtk::Button {
    let label = if loading {
        copy.searching.to_string()
    } else {
        format!("{} {category_label}", copy.load_more)
    };
    let button = gtk::Button::with_label(&label);
    button.set_halign(gtk::Align::Start);
    button.set_sensitive(!loading);
    button.add_css_class("search-remote-more");
    apply_material_button(
        &button,
        MaterialButtonSpec::new(
            MaterialButtonVariant::FilledTonal,
            MaterialButtonSize::Compact,
        ),
    );
    let sender = event_tx.clone();
    button.connect_clicked(move |_| {
        let _ = sender.send(BrowserEvent::LoadMoreSearch(category));
    });
    button
}

'''

HELPER_PAGINATION = r'''def _find_renderer(node: Any, key: str) -> dict[str, Any] | None:
    if isinstance(node, dict):
        renderer = node.get(key)
        if isinstance(renderer, dict):
            return renderer
        for value in node.values():
            found = _find_renderer(value, key)
            if found is not None:
                return found
    elif isinstance(node, list):
        for value in node:
            found = _find_renderer(value, key)
            if found is not None:
                return found
    return None


def _search_page_renderer(
    response: dict[str, Any],
    continuation: str,
) -> dict[str, Any] | None:
    if continuation:
        continuation_contents = response.get("continuationContents") or {}
        renderer = continuation_contents.get("musicShelfContinuation")
        if isinstance(renderer, dict):
            return renderer

        append_action = _find_renderer(response, "appendContinuationItemsAction")
        if isinstance(append_action, dict):
            items = append_action.get("continuationItems") or []
            if isinstance(items, list):
                return {"contents": items}
        return None

    return _find_renderer(response.get("contents"), "musicShelfRenderer")


def _search_page_continuation(renderer: dict[str, Any]) -> str:
    if renderer.get("continuations") and ytmusic_get_continuation_params is not None:
        try:
            return str(ytmusic_get_continuation_params(renderer) or "")
        except Exception:
            pass

    contents = renderer.get("contents") or renderer.get("items") or []
    if not isinstance(contents, list) or ytmusic_get_continuation_token is None:
        return ""
    try:
        token = ytmusic_get_continuation_token(contents)
    except Exception:
        token = None
    if not token:
        return ""
    if ytmusic_get_continuation_string is not None:
        return str(ytmusic_get_continuation_string(token) or "")
    return f"&ctoken={token}&continuation={token}"


def _search_page_items(
    renderer: dict[str, Any],
    result_type: str,
) -> list[dict[str, Any]]:
    if ytmusic_parse_search_results is None:
        raise RuntimeError("The installed ytmusicapi search parser is unavailable")

    contents = renderer.get("contents") or renderer.get("items") or []
    if not isinstance(contents, list):
        return []
    list_items = [
        item
        for item in contents
        if isinstance(item, dict) and "musicResponsiveListItemRenderer" in item
    ]
    if not list_items:
        return []

    parsed = ytmusic_parse_search_results(list_items, result_type, None)
    return _dedupe(
        [
            item
            for result in parsed
            if isinstance(result, dict)
            if (item := _search_item(result))
        ]
    )


def command_search_page(payload: dict[str, Any]) -> dict[str, Any]:
    query = str(payload.get("query") or "").strip()
    if not query:
        return {"items": [], "continuation": ""}

    filter_name = str(payload.get("filter") or "songs").strip().lower()
    result_types = {
        "songs": "song",
        "albums": "album",
        "artists": "artist",
        "playlists": "playlist",
    }
    if filter_name not in result_types:
        raise RuntimeError(f"Unsupported paginated search filter: {filter_name}")
    if ytmusic_get_search_params is None:
        raise RuntimeError("The installed ytmusicapi search parameter helper is unavailable")

    continuation = str(payload.get("continuation") or "").strip()
    body: dict[str, Any] = {"query": query}
    params = ytmusic_get_search_params(filter_name, None, False)
    if params:
        body["params"] = params

    client = _create_client(authenticated=True)
    request = getattr(client, "_send_request", None)
    if not callable(request):
        raise RuntimeError("The installed ytmusicapi search transport is unavailable")
    response = request("search", body, continuation) if continuation else request("search", body)
    if not isinstance(response, dict):
        return {"items": [], "continuation": ""}

    renderer = _search_page_renderer(response, continuation)
    if renderer is None:
        return {"items": [], "continuation": ""}
    return {
        "items": _search_page_items(renderer, result_types[filter_name]),
        "continuation": _search_page_continuation(renderer),
    }


'''

PAGINATION_DOC_CONTENT = r'''# Real remote search pagination

## Scope

Nocky now requests one real YouTube Music search page per category and keeps the
opaque continuation returned by the remote service. Songs, albums, artists and
playlists paginate independently.

## Request flow

1. The initial categorized search requests the first remote page for each
   category in parallel.
2. The helper parses the initial `musicShelfRenderer` and returns both its items
   and the next continuation.
3. After all fetched items in a category are visible, **Load more** sends only
   that category's continuation.
4. The response is appended with stable identity deduplication and its next
   continuation replaces the previous token.
5. An empty continuation removes the remote load-more control for that category.

The helper supports both the classic `continuationContents` response and the
newer append-action response shape. Continuations remain opaque outside the
helper.

## Safety and state

- one pagination request per category can be active at a time;
- changing the query invalidates late page responses through the existing
  request generation;
- a pagination failure preserves all previously displayed results;
- successful pages update the expiring query cache without mixing synchronized
  local-library matches into the remote snapshot;
- account transitions continue to clear the complete query cache.

## Deferred

Search history, recent queries, mixed local/remote ranking, route-aware request
cancellation and optional accessibility announcements remain separate
checkpoints.
'''

HELPER_TEST_CONTENT = r'''#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
HELPERS = ROOT / "helpers"
sys.path.insert(0, str(HELPERS))

SPEC = importlib.util.spec_from_file_location("nocky_youtube", HELPERS / "nocky_youtube.py")
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class SearchPaginationHelperTests(unittest.TestCase):
    def test_finds_initial_and_append_action_renderers(self) -> None:
        initial = {
            "contents": {
                "tabbedSearchResultsRenderer": {
                    "tabs": [
                        {
                            "tabRenderer": {
                                "content": {
                                    "sectionListRenderer": {
                                        "contents": [
                                            {
                                                "musicShelfRenderer": {
                                                    "contents": [{"first": True}]
                                                }
                                            }
                                        ]
                                    }
                                }
                            }
                        }
                    ]
                }
            }
        }
        self.assertEqual(
            MODULE._search_page_renderer(initial, ""),
            {"contents": [{"first": True}]},
        )

        continuation = {
            "onResponseReceivedActions": [
                {
                    "appendContinuationItemsAction": {
                        "continuationItems": [{"next": True}]
                    }
                }
            ]
        }
        self.assertEqual(
            MODULE._search_page_renderer(continuation, "opaque"),
            {"contents": [{"next": True}]},
        )

    def test_builds_next_continuation_from_classic_renderer(self) -> None:
        original = MODULE.ytmusic_get_continuation_params
        MODULE.ytmusic_get_continuation_params = lambda _renderer: (
            "&ctoken=next&continuation=next"
        )
        try:
            value = MODULE._search_page_continuation(
                {"continuations": [{"nextContinuationData": {}}]}
            )
        finally:
            MODULE.ytmusic_get_continuation_params = original
        self.assertEqual(value, "&ctoken=next&continuation=next")

    def test_parses_only_responsive_search_rows(self) -> None:
        original = MODULE.ytmusic_parse_search_results
        MODULE.ytmusic_parse_search_results = lambda rows, result_type, _category: [
            {
                "resultType": result_type,
                "title": "Starlight",
                "videoId": "abcdefghijk",
                "artists": [{"name": "Muse"}],
            }
            for _row in rows
        ]
        try:
            items = MODULE._search_page_items(
                {
                    "contents": [
                        {"musicResponsiveListItemRenderer": {}},
                        {"continuationItemRenderer": {}},
                    ]
                },
                "song",
            )
        finally:
            MODULE.ytmusic_parse_search_results = original
        self.assertEqual(len(items), 1)
        self.assertEqual(items[0]["title"], "Starlight")


if __name__ == "__main__":
    unittest.main()
'''


def patch_youtube_mod(text: str) -> str:
    old_results = '''#[derive(Clone, Debug, Default)]
pub struct YouTubeSearchResults {
    pub query: String,
    pub loading: bool,
    pub error: String,
    pub songs: Vec<YouTubeItem>,
    pub albums: Vec<YouTubeItem>,
    pub artists: Vec<YouTubeItem>,
    pub playlists: Vec<YouTubeItem>,
}

'''
    text = replace_once(text, old_results, SEARCH_RESULTS_BLOCK, "Add paginated search model")
    text = replace_between(
        text,
        "impl YouTubeSearchResults {\n",
        "impl YouTubeLibraryCache {\n",
        SEARCH_RESULTS_IMPL,
        "Add search-page merge contract",
    )
    bridge_anchor = '''    pub fn search(&self, query: &str, filter: &str) -> Result<Vec<YouTubeItem>, String> {
        self.run(
            "search",
            json!({ "query": query, "filter": filter, "limit": 30 }),
        )
    }

'''
    bridge_new = bridge_anchor + '''    pub fn search_page(
        &self,
        query: &str,
        category: YouTubeSearchCategory,
        continuation: &str,
    ) -> Result<YouTubeSearchPage, String> {
        self.run(
            "search_page",
            json!({
                "query": query,
                "filter": category.filter(),
                "continuation": continuation,
            }),
        )
    }

'''
    return replace_once(text, bridge_anchor, bridge_new, "Add paginated bridge request")


def patch_background_message(text: str) -> str:
    text = replace_once(
        text,
        '''        YouTubeArtistOverview, YouTubeHomePage, YouTubeItem, YouTubeLibrarySnapshot,
        YouTubePlaylistCreation, YouTubeSearchResults, YouTubeStatus, YouTubeStream,
''',
        '''        YouTubeArtistOverview, YouTubeHomePage, YouTubeItem, YouTubeLibrarySnapshot,
        YouTubePlaylistCreation, YouTubeSearchCategory, YouTubeSearchPage, YouTubeSearchResults,
        YouTubeStatus, YouTubeStream,
''',
        "Import paginated search background types",
    )
    return replace_once(
        text,
        '''    YouTubeGlobalSearch {
        request_id: u64,
        query: String,
        result: Result<YouTubeSearchResults, String>,
    },
''',
        '''    YouTubeGlobalSearch {
        request_id: u64,
        query: String,
        result: Result<YouTubeSearchResults, String>,
    },
    YouTubeSearchPageLoaded {
        request_id: u64,
        query: String,
        category: YouTubeSearchCategory,
        result: Result<YouTubeSearchPage, String>,
    },
''',
        "Add background pagination response",
    )


def patch_youtube_controller(text: str) -> str:
    text = replace_once(
        text,
        '''        LikeMutationStartError, YouTubeItem, YouTubePageEvent, YouTubeSearchCacheLookup,
        YouTubeSearchResults, YouTubeStatus,
''',
        '''        LikeMutationStartError, YouTubeItem, YouTubePageEvent, YouTubeSearchCacheLookup,
        YouTubeSearchCategory, YouTubeSearchResults, YouTubeStatus,
''',
        "Import search category in controller",
    )
    return replace_between(
        text,
        "    pub(crate) fn request_global_youtube_search(&self, query: String) {\n",
        "    pub(crate) fn load_youtube_home_page(&self, continuation: String, params: String) {\n",
        REQUEST_GLOBAL_SEARCH,
        "Implement initial and continuation search requests",
    )


def patch_background_controller(text: str) -> str:
    text = replace_once(
        text,
        '''        YouTubeItem,
''',
        '''        YouTubeItem, YouTubeSearchCategory,
''',
        "Import paginated category in background controller",
    )
    return replace_between(
        text,
        "                BackgroundMessage::YouTubeGlobalSearch {\n",
        "                BackgroundMessage::YouTubeItems { title, result } => match result {\n",
        GLOBAL_SEARCH_HANDLERS,
        "Handle and cache paginated search pages",
    )


def patch_navigation(text: str) -> str:
    return replace_once(
        text,
        '''                BrowserEvent::RefreshSearch => self.refresh_browser(),
''',
        '''                BrowserEvent::RefreshSearch => self.refresh_browser(),
                BrowserEvent::LoadMoreSearch(category) => {
                    self.load_more_youtube_search(category);
                }
''',
        "Dispatch search pagination event",
    )


def patch_browser(text: str) -> str:
    text = replace_once(
        text,
        '''        YouTubeHomePage, YouTubeHomeSection, YouTubeItem, YouTubeLibraryCache,
''',
        '''        YouTubeHomePage, YouTubeHomeSection, YouTubeItem, YouTubeLibraryCache,
        YouTubeSearchCategory,
''',
        "Import search category in browser",
    )
    text = replace_once(
        text,
        '''    RefreshSearch,
    Navigate(BrowserRoute),
''',
        '''    RefreshSearch,
    LoadMoreSearch(YouTubeSearchCategory),
    Navigate(BrowserRoute),
''',
        "Add search pagination browser event",
    )
    text = replace_once(
        text,
        "fn search_list_section(\n",
        REMOTE_MORE_BUTTON + "fn search_list_section(\n",
        "Add remote pagination button",
    )
    text = replace_between(
        text,
        "#[expect(\n    clippy::too_many_arguments,\n    reason = \"Search sections keep paging, rendering and playback context explicit\"\n)]\nfn search_list_section(\n",
        "fn search_collection_action_spec(\n",
        SEARCH_LIST_SECTION,
        "Render remote continuation controls",
    )

    old_track_more = '''        if track_matches.len() > track_limit {
            track_section.append(&search_more_button(
                copy.tracks,
                track_matches.len() - track_limit,
                self.search_track_limit.clone(),
                &self.event_tx,
                copy,
            ));
        }
'''
    new_track_more = '''        if track_matches.len() > track_limit {
            track_section.append(&search_more_button(
                copy.tracks,
                track_matches.len() - track_limit,
                self.search_track_limit.clone(),
                &self.event_tx,
                copy,
            ));
        } else if online_state_matches
            && !loading
            && (!youtube
                .search
                .continuation(YouTubeSearchCategory::Songs)
                .is_empty()
                || youtube
                    .search
                    .loading_more(YouTubeSearchCategory::Songs))
        {
            track_section.append(&search_remote_more_button(
                copy.tracks,
                YouTubeSearchCategory::Songs,
                youtube
                    .search
                    .loading_more(YouTubeSearchCategory::Songs),
                &self.event_tx,
                copy,
            ));
        }
'''
    text = replace_once(text, old_track_more, new_track_more, "Paginate track results")

    old_calls = '''        self.search_content.append(&search_list_section(
            copy.albums,
            copy.no_albums,
            search_album_cards(tracks, youtube, &query, online_state_matches),
            self.search_album_limit.clone(),
            &self.event_tx,
            loading,
            copy,
            config,
            playback,
        ));
        self.search_content.append(&search_list_section(
            copy.artists,
            copy.no_artists,
            search_artist_cards(tracks, youtube, &query, online_state_matches),
            self.search_artist_limit.clone(),
            &self.event_tx,
            loading,
            copy,
            config,
            playback,
        ));
        self.search_content.append(&search_list_section(
            copy.playlists,
            copy.no_playlists,
            search_playlist_cards(tracks, config, youtube, &query, online_state_matches),
            self.search_playlist_limit.clone(),
            &self.event_tx,
            loading,
            copy,
            config,
            playback,
        ));
'''
    new_calls = '''        self.search_content.append(&search_list_section(
            copy.albums,
            copy.no_albums,
            search_album_cards(tracks, youtube, &query, online_state_matches),
            self.search_album_limit.clone(),
            &self.event_tx,
            loading,
            copy,
            config,
            playback,
            YouTubeSearchCategory::Albums,
            if online_state_matches {
                youtube
                    .search
                    .continuation(YouTubeSearchCategory::Albums)
            } else {
                ""
            },
            online_state_matches
                && youtube
                    .search
                    .loading_more(YouTubeSearchCategory::Albums),
        ));
        self.search_content.append(&search_list_section(
            copy.artists,
            copy.no_artists,
            search_artist_cards(tracks, youtube, &query, online_state_matches),
            self.search_artist_limit.clone(),
            &self.event_tx,
            loading,
            copy,
            config,
            playback,
            YouTubeSearchCategory::Artists,
            if online_state_matches {
                youtube
                    .search
                    .continuation(YouTubeSearchCategory::Artists)
            } else {
                ""
            },
            online_state_matches
                && youtube
                    .search
                    .loading_more(YouTubeSearchCategory::Artists),
        ));
        self.search_content.append(&search_list_section(
            copy.playlists,
            copy.no_playlists,
            search_playlist_cards(tracks, config, youtube, &query, online_state_matches),
            self.search_playlist_limit.clone(),
            &self.event_tx,
            loading,
            copy,
            config,
            playback,
            YouTubeSearchCategory::Playlists,
            if online_state_matches {
                youtube
                    .search
                    .continuation(YouTubeSearchCategory::Playlists)
            } else {
                ""
            },
            online_state_matches
                && youtube
                    .search
                    .loading_more(YouTubeSearchCategory::Playlists),
        ));
'''
    return replace_once(text, old_calls, new_calls, "Pass category continuation state to search sections")


def patch_helper(text: str) -> str:
    old_import = '''    try:
        from ytmusicapi.continuations import get_continuation_params as ytmusic_get_continuation_params
        from ytmusicapi.parsers.browsing import parse_mixed_content as ytmusic_parse_mixed_content
    except Exception:
        ytmusic_get_continuation_params = None
        ytmusic_parse_mixed_content = None
'''
    new_import = '''    try:
        from ytmusicapi.continuations import (
            get_continuation_params as ytmusic_get_continuation_params,
            get_continuation_string as ytmusic_get_continuation_string,
            get_continuation_token as ytmusic_get_continuation_token,
        )
        from ytmusicapi.parsers.browsing import (
            parse_mixed_content as ytmusic_parse_mixed_content,
        )
        from ytmusicapi.parsers.search import (
            get_search_params as ytmusic_get_search_params,
            parse_search_results as ytmusic_parse_search_results,
        )
    except Exception:
        ytmusic_get_continuation_params = None
        ytmusic_get_continuation_string = None
        ytmusic_get_continuation_token = None
        ytmusic_get_search_params = None
        ytmusic_parse_mixed_content = None
        ytmusic_parse_search_results = None
'''
    text = replace_once(text, old_import, new_import, "Import search continuation helpers")
    text = replace_once(
        text,
        '''    ytmusic_get_continuation_params = None
    ytmusic_parse_mixed_content = None
''',
        '''    ytmusic_get_continuation_params = None
    ytmusic_get_continuation_string = None
    ytmusic_get_continuation_token = None
    ytmusic_get_search_params = None
    ytmusic_parse_mixed_content = None
    ytmusic_parse_search_results = None
''',
        "Initialize unavailable search helpers",
    )
    text = replace_once(
        text,
        "def command_library(payload: dict[str, Any]) -> list[dict[str, Any]]:\n",
        HELPER_PAGINATION + "def command_library(payload: dict[str, Any]) -> list[dict[str, Any]]:\n",
        "Add helper search-page command",
    )
    text = replace_once(
        text,
        '''    "search": command_search,
    "library": command_library,
''',
        '''    "search": command_search,
    "search_page": command_search_page,
    "library": command_library,
''',
        "Register helper search-page command",
    )
    return replace_once(
        text,
        "<status|stream_clients|connect|disconnect|search|library|library_v2|",
        "<status|stream_clients|connect|disconnect|search|search_page|library|library_v2|",
        "Document helper search-page command",
    )


def patch_roadmap(text: str) -> str:
    text = replace_one_of(
        text,
        [
            "- 🟡 True remote pagination beyond the initial search batches.\n",
            "- 🟡 Remote search pagination and cache expiration.\n",
        ],
        "- 🟡 Search history and recent queries.\n",
        "Advance active search checkpoint",
    )
    anchor = "- ✅ Expiring search cache with a 10-minute fresh TTL, one-hour stale-while-revalidate window and bounded LRU eviction.\n"
    addition = anchor + "- ✅ Real per-category remote pagination backed by opaque YouTube Music continuations.\n"
    text = replace_once(text, anchor, addition, "Document completed remote pagination")
    text = replace_once(
        text,
        "- True remote pagination beyond the initial batches.\n",
        "",
        "Remove completed pagination item",
    )
    return replace_one_of(
        text,
        [
            "8. Finish true remote search pagination.\n",
            "8. Finish search pagination, caching and keyboard navigation.\n",
        ],
        "8. Add search history, mixed-source ranking and route-aware cancellation.\n",
        "Advance recommended search order",
    )


def patch_cache_doc(text: str) -> str:
    old = '''## Deferred

True remote pagination and continuation tokens remain the next search
checkpoint. The current cache stores the initial categorized batches and is
structured so paginated batches can update the same query entry later.
'''
    new = '''## Pagination integration

Successful continuation pages now append directly to the remote-only cache
entry, refresh its age and replace the category continuation. Synchronized local
library matches are still merged only when the cached query is displayed.

## Deferred

Search history, mixed local/remote ranking and route-aware cancellation remain
separate checkpoints.
'''
    return replace_once(text, old, new, "Document pagination cache integration")


def main() -> int:
    required = [
        YOUTUBE_MOD,
        SEARCH_CACHE,
        BACKGROUND_MESSAGE,
        YOUTUBE_CONTROLLER,
        BACKGROUND_CONTROLLER,
        NAVIGATION,
        BROWSER,
        HELPER,
        ROADMAP,
        CACHE_DOC,
    ]
    missing = [path for path in required if not path.is_file()]
    if missing:
        print("Run this script from the Nocky repository root after applying the search-cache patch.", file=sys.stderr)
        for path in missing:
            print(f"missing: {path}", file=sys.stderr)
        return 1

    original = {path: path.read_text(encoding="utf-8") for path in required}
    if "YouTubeSearchCacheLookup" not in original[YOUTUBE_CONTROLLER]:
        print("ERROR: apply and validate the expiring search-cache patch first.", file=sys.stderr)
        return 1
    if "search_result_primary_action" not in original[BROWSER]:
        print("ERROR: apply and validate the keyboard-first search actions patch first.", file=sys.stderr)
        return 1
    if "YouTubeSearchCategory" in original[YOUTUBE_MOD]:
        print("Search pagination appears to be applied already.")
        return 0

    updated = dict(original)
    try:
        updated[YOUTUBE_MOD] = patch_youtube_mod(updated[YOUTUBE_MOD])
        updated[BACKGROUND_MESSAGE] = patch_background_message(updated[BACKGROUND_MESSAGE])
        updated[YOUTUBE_CONTROLLER] = patch_youtube_controller(updated[YOUTUBE_CONTROLLER])
        updated[BACKGROUND_CONTROLLER] = patch_background_controller(updated[BACKGROUND_CONTROLLER])
        updated[NAVIGATION] = patch_navigation(updated[NAVIGATION])
        updated[BROWSER] = patch_browser(updated[BROWSER])
        updated[HELPER] = patch_helper(updated[HELPER])
        updated[ROADMAP] = patch_roadmap(updated[ROADMAP])
        updated[CACHE_DOC] = patch_cache_doc(updated[CACHE_DOC])
    except PatchError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        print("No files were written.", file=sys.stderr)
        return 1

    for path, expected in [
        (PAGINATION_DOC, PAGINATION_DOC_CONTENT),
        (HELPER_TEST, HELPER_TEST_CONTENT),
    ]:
        if path.exists() and path.read_text(encoding="utf-8") != expected:
            print(f"ERROR: {path} already exists with different content.", file=sys.stderr)
            print("No files were written.", file=sys.stderr)
            return 1

    changed: list[Path] = []
    for path in required:
        content = SEARCH_CACHE_SOURCE if path == SEARCH_CACHE else updated[path]
        if content != original[path]:
            path.write_text(content, encoding="utf-8")
            changed.append(path.relative_to(ROOT))

    for path, content in [
        (PAGINATION_DOC, PAGINATION_DOC_CONTENT),
        (HELPER_TEST, HELPER_TEST_CONTENT),
    ]:
        if not path.exists():
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8")
            changed.append(path.relative_to(ROOT))

    print("Real remote YouTube search pagination patch applied successfully.")
    for path in changed:
        print(f"  {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

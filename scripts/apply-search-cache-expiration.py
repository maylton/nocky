#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path.cwd()
YOUTUBE_MOD = ROOT / "src/youtube/mod.rs"
SEARCH_CACHE = ROOT / "src/youtube/search_cache.rs"
CONTROLLER_MOD = ROOT / "src/app/controller/mod.rs"
CONSTRUCTION = ROOT / "src/app/controller/construction.rs"
YOUTUBE_CONTROLLER = ROOT / "src/app/controller/youtube.rs"
BACKGROUND = ROOT / "src/app/controller/background.rs"
ROADMAP = ROOT / "ROADMAP.md"
DOC = ROOT / "docs/SEARCH_CACHE.md"


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


def replace_count(text: str, old: str, new: str, expected: int, label: str) -> str:
    count = text.count(old)
    if count == 0 and text.count(new) >= expected:
        print(f"[already applied] {label}")
        return text
    if count != expected:
        raise PatchError(f"{label}: expected {expected} matches, found {count}")
    print(f"[changed] {label}")
    return text.replace(old, new)


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


SEARCH_CACHE_SOURCE = r'''use super::YouTubeSearchResults;
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
        results.loading = false;
        results.error.clear();
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
        cache.insert_at("Muse", results("Muse", "Hysteria"), start);

        let YouTubeSearchCacheLookup::Fresh(cached) = cache.lookup_at("muse", start) else {
            panic!("expected a fresh cache hit");
        };
        assert!(!cached.loading);
        assert!(cached.error.is_empty());
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
            let filters = ["songs", "albums", "artists", "playlists"];
            let expected = filters.len();
            let (result_tx, result_rx) = mpsc::channel();
            let mut workers = Vec::with_capacity(expected);

            for filter in filters {
                let bridge = bridge.clone();
                let result_tx = result_tx.clone();
                let worker_query = query.clone();
                workers.push(thread::spawn(move || {
                    let result = bridge.search(&worker_query, filter);
                    let _ = result_tx.send((filter, result));
                }));
            }
            drop(result_tx);

            let mut categorized = YouTubeSearchResults {
                query: query.clone(),
                ..YouTubeSearchResults::default()
            };
            let mut errors = Vec::new();

            for (filter, result) in result_rx {
                match result {
                    Ok(items) => match filter {
                        "songs" => {
                            categorized.songs =
                                items.into_iter().filter(YouTubeItem::playable).collect()
                        }
                        "albums" => categorized.albums = items,
                        "artists" => categorized.artists = items,
                        "playlists" => categorized.playlists = items,
                        _ => {}
                    },
                    Err(error) => errors.push(format!("{filter}: {error}")),
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

'''

GLOBAL_SEARCH_HANDLER = r'''                BackgroundMessage::YouTubeGlobalSearch {
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
'''

DOC_CONTENT = r'''# Expiring YouTube Search Cache

## Scope

This checkpoint adds a bounded, session-scoped cache for categorized YouTube
Music search results. It deliberately does not persist account search history to
disk.

## Policy

- cache key: normalized query text;
- fresh lifetime: 10 minutes;
- stale-while-revalidate window: up to 60 minutes;
- capacity: 32 queries;
- eviction: least recently used entry;
- account disconnect or reconnect: immediate cache clear.

## Behavior

A fresh hit paints immediately and skips the remote request. A stale hit paints
immediately with the existing searching banner while the four remote categories
are refreshed in parallel. Entries older than the stale window are discarded.

Only remote results are stored. Current synchronized library matches are merged
when a cache entry is displayed, preventing removed local or synchronized items
from becoming permanently embedded in a query snapshot.

Request generation remains authoritative: using a fresh cache entry increments
the request ID and invalidates older in-flight responses, so a late response can
never replace the active query.

## Deferred

True remote pagination and continuation tokens remain the next search
checkpoint. The current cache stores the initial categorized batches and is
structured so paginated batches can update the same query entry later.
'''


def patch_youtube_mod(text: str) -> str:
    text = replace_once(
        text,
        "mod routing;\nmod structured_cards;\n",
        "mod routing;\nmod search_cache;\nmod structured_cards;\n",
        "Register search cache module",
    )
    return replace_once(
        text,
        "pub(crate) use routing::{youtube_item_action, YouTubeItemAction};\n",
        "pub(crate) use routing::{youtube_item_action, YouTubeItemAction};\n"
        "pub(crate) use search_cache::{YouTubeSearchCache, YouTubeSearchCacheLookup};\n",
        "Export search cache contract",
    )


def patch_controller_mod(text: str) -> str:
    text = replace_once(
        text,
        """        LikeMutationRegistry, YouTubeBridge, YouTubeHomePage, YouTubeItem, YouTubeLibraryCache,
        YouTubePage,
""",
        """        LikeMutationRegistry, YouTubeBridge, YouTubeHomePage, YouTubeItem, YouTubeLibraryCache,
        YouTubePage, YouTubeSearchCache,
""",
        "Import controller search cache",
    )
    return replace_once(
        text,
        """    pub(crate) youtube_search_request_id: Cell<u64>,
    pub(crate) youtube_home_request_id: Cell<u64>,
""",
        """    pub(crate) youtube_search_request_id: Cell<u64>,
    pub(crate) youtube_search_cache: RefCell<YouTubeSearchCache>,
    pub(crate) youtube_home_request_id: Cell<u64>,
""",
        "Add controller search cache state",
    )


def patch_construction(text: str) -> str:
    text = replace_once(
        text,
        """        YouTubeBridge, YouTubeHomePage, YouTubePage, YouTubeSearchResults,
""",
        """        YouTubeBridge, YouTubeHomePage, YouTubePage, YouTubeSearchCache,
        YouTubeSearchResults,
""",
        "Import search cache during construction",
    )
    return replace_once(
        text,
        """                youtube_search_request_id: Cell::new(0),
                youtube_home_request_id: Cell::new(0),
""",
        """                youtube_search_request_id: Cell::new(0),
                youtube_search_cache: RefCell::new(YouTubeSearchCache::default()),
                youtube_home_request_id: Cell::new(0),
""",
        "Initialize search cache",
    )


def patch_youtube_controller(text: str) -> str:
    text = replace_once(
        text,
        """        LikeMutationStartError, YouTubeItem, YouTubePageEvent, YouTubeSearchResults, YouTubeStatus,
""",
        """        LikeMutationStartError, YouTubeItem, YouTubePageEvent, YouTubeSearchCacheLookup,
        YouTubeSearchResults, YouTubeStatus,
""",
        "Import cache lookup state",
    )
    return replace_between(
        text,
        "    pub(crate) fn request_global_youtube_search(&self, query: String) {\n",
        "    pub(crate) fn load_youtube_home_page(&self, continuation: String, params: String) {\n",
        REQUEST_GLOBAL_SEARCH,
        "Use expiring search cache",
    )


def patch_background(text: str) -> str:
    text = replace_count(
        text,
        """                            self.youtube_library.borrow_mut().clear();
                            clear_library_cache();
""",
        """                            self.youtube_search_cache.borrow_mut().clear();
                            self.youtube_library.borrow_mut().clear();
                            clear_library_cache();
""",
        2,
        "Clear search cache on disconnected account",
    )
    text = replace_once(
        text,
        """                        self.youtube_page
                            .set_loading(false, "YouTube Music connected");
                        {
""",
        """                        self.youtube_page
                            .set_loading(false, "YouTube Music connected");
                        self.youtube_search_cache.borrow_mut().clear();
                        {
""",
        "Clear search cache on account reconnect",
    )
    return replace_between(
        text,
        "                BackgroundMessage::YouTubeGlobalSearch {\n",
        "                BackgroundMessage::YouTubeItems { title, result } => match result {\n",
        GLOBAL_SEARCH_HANDLER,
        "Store successful remote search responses",
    )


def patch_roadmap(text: str) -> str:
    text = replace_one_of(
        text,
        [
            "- 🟡 Compact search-result actions and keyboard-first result navigation.\n",
            "- 🟡 Remote search pagination and cache expiration.\n",
        ],
        "- 🟡 True remote pagination beyond the initial search batches.\n",
        "Advance active search checkpoint",
    )

    anchor = "- Local and YouTube results remain source-aware.\n"
    addition = (
        anchor
        + "- ✅ Expiring search cache with a 10-minute fresh TTL, one-hour "
        + "stale-while-revalidate window and bounded LRU eviction.\n"
    )
    text = replace_once(text, anchor, addition, "Document completed search cache")

    text = replace_once(
        text,
        "- Search-result cache with expiration.\n",
        "",
        "Remove completed search cache item",
    )

    old_order = "8. Finish search pagination, caching and keyboard navigation.\n"
    new_order = "8. Finish true remote search pagination.\n"
    if old_order in text:
        text = replace_once(text, old_order, new_order, "Update recommended search order")
    elif new_order in text:
        print("[already applied] Update recommended search order")
    else:
        raise PatchError("Update recommended search order: marker not found")
    return text


def main() -> int:
    required = [
        YOUTUBE_MOD,
        CONTROLLER_MOD,
        CONSTRUCTION,
        YOUTUBE_CONTROLLER,
        BACKGROUND,
        ROADMAP,
    ]
    missing = [path for path in required if not path.is_file()]
    if missing:
        print("Run this script from the Nocky repository root.", file=sys.stderr)
        for path in missing:
            print(f"missing: {path}", file=sys.stderr)
        return 1

    for path, expected in [(SEARCH_CACHE, SEARCH_CACHE_SOURCE), (DOC, DOC_CONTENT)]:
        if path.exists() and path.read_text(encoding="utf-8") != expected:
            print(f"ERROR: {path} already exists with different content.", file=sys.stderr)
            return 1

    original = {path: path.read_text(encoding="utf-8") for path in required}
    updated = dict(original)

    try:
        updated[YOUTUBE_MOD] = patch_youtube_mod(updated[YOUTUBE_MOD])
        updated[CONTROLLER_MOD] = patch_controller_mod(updated[CONTROLLER_MOD])
        updated[CONSTRUCTION] = patch_construction(updated[CONSTRUCTION])
        updated[YOUTUBE_CONTROLLER] = patch_youtube_controller(updated[YOUTUBE_CONTROLLER])
        updated[BACKGROUND] = patch_background(updated[BACKGROUND])
        updated[ROADMAP] = patch_roadmap(updated[ROADMAP])
    except PatchError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        print("No files were written.", file=sys.stderr)
        return 1

    changed: list[Path] = []
    for path in required:
        if updated[path] != original[path]:
            path.write_text(updated[path], encoding="utf-8")
            changed.append(path.relative_to(ROOT))

    for path, content in [(SEARCH_CACHE, SEARCH_CACHE_SOURCE), (DOC, DOC_CONTENT)]:
        if not path.exists():
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8")
            changed.append(path.relative_to(ROOT))

    print("Expiring YouTube search cache patch applied successfully.")
    for path in changed:
        print(f"  {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

use super::{YouTubeSearchCategory, YouTubeSearchPage, YouTubeSearchResults};
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
        self.entries
            .retain(|_, entry| now.saturating_duration_since(entry.stored_at) <= stale_ttl);
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
        let mut cache =
            YouTubeSearchCache::with_policy(Duration::from_secs(10), Duration::from_secs(30), 4);
        cache.insert_at(
            "  Daft   Punk ",
            results("Daft Punk", "One More Time"),
            start,
        );

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
        let mut cache =
            YouTubeSearchCache::with_policy(Duration::from_secs(10), Duration::from_secs(30), 4);
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
        let mut cache =
            YouTubeSearchCache::with_policy(Duration::from_secs(60), Duration::from_secs(120), 2);
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
        let mut cache =
            YouTubeSearchCache::with_policy(Duration::from_secs(60), Duration::from_secs(120), 4);
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

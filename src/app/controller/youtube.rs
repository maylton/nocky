//! YouTube controller methods for `AppController`.

use super::{
    youtube_playlist_revalidation_can_start, youtube_playlist_revalidation_delay, AppController,
    PlaylistRevalidationState,
};
use crate::{
    background::BackgroundMessage,
    browser::{BrowserRoute, YouTubeCollectionRoute},
    config::StartupSource,
    listening_history,
    youtube::{
        self as youtube_domain, cache_first_items_for_browser, cache_home_page_covers,
        cache_items_for_browser, repair_home_page_cover_paths, resolve_youtube_collection_item,
        youtube_collection_cache_key, youtube_collection_key, youtube_home_prefetch_candidates,
        LikeMutationStartError, YouTubeItem, YouTubePageEvent, YouTubeSearchCacheLookup,
        YouTubeSearchCategory, YouTubeSearchResults, YouTubeStatus,
    },
};
use gtk::prelude::*;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Instant,
};

const YOUTUBE_HOME_STREAM_PRELOAD_LIMIT: usize = 6;

fn youtube_home_stream_preload_items(
    page: &youtube_domain::YouTubeHomePage,
    limit: usize,
) -> Vec<YouTubeItem> {
    let mut items = Vec::new();
    let mut seen = HashSet::new();
    for section in &page.sections {
        for item in &section.items {
            if item.playable() && seen.insert(item.video_id.clone()) {
                items.push(item.clone());
                if items.len() == limit {
                    return items;
                }
            }
        }
    }
    items
}

impl AppController {
    pub(crate) fn present_assisted_youtube_login(&self) {
        let page = self.youtube_page.clone();
        let language = self.config.borrow().language;
        if let Err(error) =
            youtube_domain::present_assisted_login(&self.window, language, move |raw| {
                page.submit_assisted_session(raw)
            })
        {
            self.youtube_page.show_manual_import();
            self.show_toast(&error);
        }
    }

    pub(crate) fn refresh_youtube_status(&self) {
        let Some(bridge) = self.youtube_bridge.clone() else {
            self.youtube_page.set_status(&YouTubeStatus::default());
            self.youtube_page.show_error(
                "YouTube Music runtime is missing. Run ./scripts/setup-youtube-runtime.sh for cargo run, or reinstall with ./install.sh --install-youtube.",
            );
            return;
        };
        let sender = self.background.sender();
        thread::spawn(move || {
            let _ = sender.send(BackgroundMessage::YouTubeStatus(
                bridge.status_with_profile(),
            ));
        });
    }

    pub(crate) fn sync_youtube_library(&self, force: bool, notify: bool) -> bool {
        let Some(bridge) = self.youtube_bridge.clone() else {
            return false;
        };
        {
            let mut library = self.youtube_library.borrow_mut();
            if !library.connected || library.syncing || (library.synced && !force) {
                return false;
            }
            library.syncing = true;
        }
        let sender = self.background.sender();
        thread::spawn(move || {
            let _ = sender.send(BackgroundMessage::YouTubeLibrarySynced {
                notify,
                result: bridge.sync_library(),
            });
        });
        true
    }

    pub(crate) fn prefetch_youtube_playlist_cache(&self) {
        let Some(bridge) = self.youtube_bridge.clone() else {
            return;
        };
        if self.youtube_playlist_prefetching.get() {
            return;
        }
        let playlists = {
            let library = self.youtube_library.borrow();
            youtube_home_prefetch_candidates(&library)
        };
        if playlists.is_empty() {
            return;
        }

        self.youtube_playlist_prefetching.set(true);
        let sender = self.background.sender();
        thread::spawn(move || {
            // Playlist requests are independent. A small worker pool prevents the
            // previous sequential 10s + 10s + 10s startup behavior without
            // flooding YouTube or spawning an unbounded number of helpers.
            let worker_count = playlists.len().min(3);
            let work = Arc::new(Mutex::new(playlists.into_iter().collect::<VecDeque<_>>()));
            let (result_tx, result_rx) = mpsc::channel();
            let mut workers = Vec::with_capacity(worker_count);

            for _ in 0..worker_count {
                let bridge = bridge.clone();
                let work = work.clone();
                let result_tx = result_tx.clone();
                workers.push(thread::spawn(move || loop {
                    let playlist = match work.lock() {
                        Ok(mut queue) => queue.pop_front(),
                        Err(_) => None,
                    };
                    let Some(playlist) = playlist else {
                        break;
                    };

                    let browse_id = playlist.browse_id.clone();
                    let result = bridge.playlist(&playlist).map(|mut items| {
                        cache_items_for_browser(&mut items);
                        items
                    });
                    let _ = result_tx.send((playlist, browse_id, result));
                }));
            }
            drop(result_tx);

            let mut cached = HashMap::new();
            for (playlist, browse_id, result) in result_rx {
                match result {
                    Ok(items) if !items.is_empty() => {
                        cached.insert(browse_id, items);
                    }
                    Ok(_) => {}
                    Err(error)
                        if error.contains(
                            "No playable tracks were returned for this YouTube Music playlist",
                        ) => {}
                    Err(error) => {
                        eprintln!(
                            "Could not pre-cache YouTube playlist '{}': {error}",
                            playlist.title
                        );
                    }
                }
            }
            for worker in workers {
                let _ = worker.join();
            }

            let _ = sender.send(BackgroundMessage::YouTubePlaylistsCached(Ok(cached)));
        });
    }

    pub(crate) fn load_youtube_playlist_for_browser(&self, playlist: YouTubeItem) {
        let Some(bridge) = self.youtube_bridge.clone() else {
            self.show_toast("As dependências do YouTube Music não estão instaladas");
            return;
        };
        let browse_id = playlist.browse_id.clone();
        if browse_id.is_empty() {
            return;
        }

        let route = BrowserRoute::YouTubePlaylist {
            title: playlist.title.clone(),
            browse_id: browse_id.clone(),
        };
        let cached = self
            .youtube_library
            .borrow()
            .playlist_tracks
            .get(&browse_id)
            .map(|items| !items.is_empty())
            .unwrap_or(false);
        if cached {
            self.navigate_browser(route);
            return;
        }

        {
            let mut library = self.youtube_library.borrow_mut();
            library.playlist_tracks.remove(&browse_id);
            library.playlist_loading.insert(browse_id.clone());
        }
        // Change pages before starting or queueing the network request. The user
        // immediately sees the playlist title and a loading row instead of
        // remaining on the previous page for several seconds.
        self.navigate_browser(route);

        if self.youtube_playlist_loading.get() {
            self.youtube_pending_playlist.replace(Some(playlist));
            return;
        }

        let request_id = self.youtube_playlist_request_id.get().wrapping_add(1);
        self.youtube_playlist_request_id.set(request_id);
        self.youtube_playlist_loading.set(true);
        let sender = self.background.sender();
        thread::spawn(move || match bridge.playlist(&playlist) {
            Ok(mut items) => {
                cache_first_items_for_browser(&mut items, 24);
                let initial_items = items.clone();
                let initial_playlist = playlist.clone();
                let _ = sender.send(BackgroundMessage::YouTubeBrowserPlaylist {
                    request_id,
                    playlist: initial_playlist,
                    result: Ok(initial_items),
                });

                cache_items_for_browser(&mut items);
                let _ = sender.send(BackgroundMessage::YouTubeBrowserPlaylistCoversCached {
                    request_id,
                    playlist,
                    items,
                });
            }
            Err(error) => {
                let _ = sender.send(BackgroundMessage::YouTubeBrowserPlaylist {
                    request_id,
                    playlist,
                    result: Err(error),
                });
            }
        });
    }

    pub(crate) fn revalidate_youtube_playlist_for_browser(&self, playlist: YouTubeItem) {
        let Some(bridge) = self.youtube_bridge.clone() else {
            return;
        };
        let browse_id = playlist.browse_id.clone();
        if browse_id.trim().is_empty() {
            return;
        }

        let state = self
            .youtube_playlist_revalidation
            .borrow()
            .get(&browse_id)
            .cloned();
        if !youtube_playlist_revalidation_can_start(state.as_ref(), Instant::now()) {
            return;
        }

        let attempt = match state {
            Some(PlaylistRevalidationState::RetryAt { attempt, .. })
            | Some(PlaylistRevalidationState::Loading { attempt }) => attempt,
            _ => 0,
        };

        self.youtube_playlist_revalidation.borrow_mut().insert(
            browse_id.clone(),
            PlaylistRevalidationState::Loading { attempt },
        );
        self.youtube_library
            .borrow_mut()
            .playlist_loading
            .insert(browse_id);

        let sender = self.background.sender();
        thread::spawn(move || {
            let result = bridge.playlist(&playlist).map(|mut items| {
                cache_items_for_browser(&mut items);
                items
            });
            let _ = sender
                .send(BackgroundMessage::YouTubeBrowserPlaylistRevalidated { playlist, result });
        });
    }

    pub(crate) fn mark_youtube_playlist_revalidation_succeeded(&self, browse_id: &str) {
        self.youtube_playlist_revalidation
            .borrow_mut()
            .insert(browse_id.to_string(), PlaylistRevalidationState::Succeeded);
    }

    pub(crate) fn schedule_youtube_playlist_revalidation_retry(&self, browse_id: &str) {
        let current_attempt = match self
            .youtube_playlist_revalidation
            .borrow()
            .get(browse_id)
            .cloned()
        {
            Some(PlaylistRevalidationState::Loading { attempt })
            | Some(PlaylistRevalidationState::RetryAt { attempt, .. }) => attempt,
            _ => 0,
        };
        let next_attempt = current_attempt.saturating_add(1);
        let delay = youtube_playlist_revalidation_delay(next_attempt);
        self.youtube_playlist_revalidation.borrow_mut().insert(
            browse_id.to_string(),
            PlaylistRevalidationState::RetryAt {
                when: Instant::now() + delay,
                attempt: next_attempt,
            },
        );
    }

    pub(crate) fn is_open_youtube_playlist(&self, browse_id: &str) -> bool {
        matches!(
            self.browser.route(),
            BrowserRoute::YouTubePlaylist {
                browse_id: current,
                ..
            } if current == browse_id
        )
    }

    pub(crate) fn load_youtube_collection_for_browser(&self, item: YouTubeItem) {
        let collection = YouTubeCollectionRoute::from_item(&item);
        let key = collection.key.clone();
        let route = if item.result_type == "artist" {
            BrowserRoute::YouTubeArtist(collection)
        } else {
            BrowserRoute::YouTubeAlbum(collection)
        };
        self.youtube_library
            .borrow_mut()
            .remember_collection_reference(item.clone());

        if item.result_type == "artist" {
            self.navigate_browser(route);

            let already_loading = self.youtube_library.borrow().artist_loading.contains(&key);
            if already_loading {
                return;
            }

            let Some(bridge) = self.youtube_bridge.clone() else {
                self.show_toast("As dependências do YouTube Music não estão instaladas");
                return;
            };

            self.youtube_library
                .borrow_mut()
                .artist_loading
                .insert(key.clone());

            let sender = self.background.sender();
            thread::spawn(move || {
                let result = resolve_youtube_collection_item(&bridge, &item, "artists")
                    .and_then(|resolved| bridge.artist_overview(&resolved))
                    .map(|mut overview| {
                        cache_items_for_browser(std::slice::from_mut(&mut overview.profile));
                        cache_items_for_browser(&mut overview.albums);
                        overview
                    });
                let _ = sender.send(BackgroundMessage::YouTubeArtistOverview { key, result });
            });
            return;
        }

        let cached = self
            .youtube_library
            .borrow()
            .collection_tracks
            .get(&key)
            .map(|items| !items.is_empty())
            .unwrap_or(false);
        if cached {
            self.navigate_browser(route);
            return;
        }

        let Some(bridge) = self.youtube_bridge.clone() else {
            self.show_toast("As dependências do YouTube Music não estão instaladas");
            return;
        };

        self.youtube_library
            .borrow_mut()
            .collection_loading
            .insert(key.clone());
        self.navigate_browser(route);

        let sender = self.background.sender();
        thread::spawn(move || {
            let result = resolve_youtube_collection_item(&bridge, &item, "albums")
                .and_then(|resolved| bridge.collection(&resolved))
                .map(|mut items| {
                    cache_items_for_browser(&mut items);
                    items
                });
            let _ = sender.send(BackgroundMessage::YouTubeBrowserCollection { item, key, result });
        });
    }

    pub(crate) fn is_open_youtube_collection(&self, key: &str) -> bool {
        match self.browser.route() {
            BrowserRoute::YouTubeAlbum(collection) | BrowserRoute::YouTubeArtist(collection) => {
                collection.key == key
            }
            _ => false,
        }
    }

    pub(crate) fn prefetch_youtube_collection_cache(&self) {
        let Some(bridge) = self.youtube_bridge.clone() else {
            return;
        };
        if self.youtube_collection_prefetching.get() {
            return;
        }

        let collections = {
            let library = self.youtube_library.borrow();
            let mut seen = HashSet::new();
            library
                .suggested_albums
                .iter()
                .take(6)
                .chain(library.suggested_artists.iter().take(6))
                .filter(|item| !item.browse_id.is_empty())
                .filter(|item| {
                    let key = youtube_collection_cache_key(item);
                    seen.insert(key.clone()) && !library.collection_tracks.contains_key(&key)
                })
                .cloned()
                .collect::<Vec<_>>()
        };
        if collections.is_empty() {
            return;
        }

        self.youtube_collection_prefetching.set(true);
        let sender = self.background.sender();
        thread::spawn(move || {
            let worker_count = collections.len().min(3);
            let work = Arc::new(Mutex::new(collections.into_iter().collect::<VecDeque<_>>()));
            let (result_tx, result_rx) = mpsc::channel();
            let mut workers = Vec::with_capacity(worker_count);

            for _ in 0..worker_count {
                let bridge = bridge.clone();
                let work = work.clone();
                let result_tx = result_tx.clone();
                workers.push(thread::spawn(move || loop {
                    let item = match work.lock() {
                        Ok(mut queue) => queue.pop_front(),
                        Err(_) => None,
                    };
                    let Some(item) = item else {
                        break;
                    };

                    let key = youtube_collection_cache_key(&item);
                    let result = bridge.collection(&item).map(|mut items| {
                        cache_items_for_browser(&mut items);
                        items
                    });
                    let _ = result_tx.send((item, key, result));
                }));
            }
            drop(result_tx);

            let mut cached = HashMap::new();
            for (item, key, result) in result_rx {
                match result {
                    Ok(items) if !items.is_empty() => {
                        cached.insert(key, items);
                    }
                    Ok(_) => {}
                    Err(error) => {
                        eprintln!(
                            "Could not pre-cache YouTube {} '{}': {error}",
                            item.result_type, item.title
                        );
                    }
                }
            }
            for worker in workers {
                let _ = worker.join();
            }

            let _ = sender.send(BackgroundMessage::YouTubeCollectionsCached(Ok(cached)));
        });
    }

    pub(crate) fn prefetch_home_artist_profiles(&self, force: bool) {
        let Some(bridge) = self.youtube_bridge.clone() else {
            return;
        };

        let limit = if force {
            self.browser.artist_display_limit()
        } else {
            12
        };

        let artists = {
            let mut library = self.youtube_library.borrow_mut();
            let mut entries = library.artists.iter().collect::<Vec<_>>();
            if force {
                entries.sort_by(|left, right| {
                    left.title.to_lowercase().cmp(&right.title.to_lowercase())
                });
            }

            let candidates = entries
                .into_iter()
                .take(limit)
                .filter_map(|entry| {
                    let key = youtube_collection_cache_key(&entry.source);
                    let missing = !library.artist_profiles.contains_key(&key);
                    let idle = !library.artist_loading.contains(&key);

                    ((force || missing) && idle).then(|| (key, entry.source.clone()))
                })
                .collect::<Vec<_>>();

            for (key, _) in &candidates {
                library.artist_loading.insert(key.clone());
            }

            candidates
        };

        if artists.is_empty() {
            return;
        }

        let sender = self.background.sender();
        thread::spawn(move || {
            let worker_count = artists.len().min(3);
            let work = Arc::new(Mutex::new(artists.into_iter().collect::<VecDeque<_>>()));
            let mut workers = Vec::with_capacity(worker_count);

            for _ in 0..worker_count {
                let bridge = bridge.clone();
                let work = work.clone();
                let sender = sender.clone();

                workers.push(thread::spawn(move || loop {
                    let next = match work.lock() {
                        Ok(mut queue) => queue.pop_front(),
                        Err(_) => None,
                    };
                    let Some((key, item)) = next else {
                        break;
                    };

                    let result = resolve_youtube_collection_item(&bridge, &item, "artists")
                        .and_then(|resolved| bridge.artist_overview(&resolved))
                        .map(|mut overview| {
                            cache_items_for_browser(std::slice::from_mut(&mut overview.profile));
                            cache_items_for_browser(&mut overview.albums);
                            overview
                        });

                    let _ = sender.send(BackgroundMessage::YouTubeArtistOverview { key, result });
                }));
            }

            for worker in workers {
                let _ = worker.join();
            }
        });
    }

    pub(crate) fn request_global_youtube_search(&self, query: String) {
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

    pub(crate) fn load_youtube_home_page(&self, continuation: String, params: String) {
        let Some(bridge) = self.youtube_bridge.clone() else {
            self.youtube_page
                .show_error("YouTube Music runtime is missing. Reinstall with --install-youtube.");
            return;
        };
        let append = !continuation.is_empty();
        let filtered = !params.is_empty();
        if append {
            let current = self.youtube_home_page.borrow();
            if !current.can_request_continuation(
                &continuation,
                &params,
                self.youtube_home_continuation_loading.get(),
            ) {
                return;
            }
        } else {
            let current = self.youtube_home_page.borrow();
            if !current.sections.is_empty()
                && current.selected_chip_params == params
                && !self.youtube_home_loading.get()
            {
                return;
            }
        }

        let request_id = self.youtube_home_request_id.get().wrapping_add(1);
        self.youtube_home_request_id.set(request_id);
        if !append {
            self.youtube_home_continuation_loading.set(false);
            let previous = self.youtube_home_page.borrow().selected_chip_params.clone();
            self.youtube_home_previous_params.replace(previous);
            self.youtube_home_page.borrow_mut().selected_chip_params = params.clone();
            self.youtube_home_loading.set(true);
        } else {
            self.youtube_home_continuation_loading.set(true);
        }
        let youtube_active = self.config.borrow().startup_source == Some(StartupSource::YouTube);
        if youtube_active && !append {
            self.refresh_browser();
        }

        if !append {
            self.youtube_page.set_loading(
                true,
                if filtered {
                    "Carregando seleção do YouTube Music..."
                } else {
                    "Carregando seu feed do YouTube Music..."
                },
            );
        }
        let sender = self.background.sender();
        thread::spawn(move || {
            match bridge.home_page(
                (!continuation.is_empty()).then_some(continuation.as_str()),
                (!params.is_empty()).then_some(params.as_str()),
            ) {
                Ok(mut page) => {
                    repair_home_page_cover_paths(&mut page);
                    let title = "Para você".to_string();
                    let initial_page = page.clone();
                    let _ = sender.send(BackgroundMessage::YouTubeStructuredPage {
                        request_id,
                        title: title.clone(),
                        home: true,
                        append,
                        result: Ok(initial_page),
                    });

                    if !append {
                        let preload_items = youtube_home_stream_preload_items(
                            &page,
                            YOUTUBE_HOME_STREAM_PRELOAD_LIMIT,
                        );
                        if !preload_items.is_empty() {
                            let preload_bridge = bridge.clone();
                            thread::spawn(move || {
                                preload_bridge.preload_items(
                                    &preload_items,
                                    YOUTUBE_HOME_STREAM_PRELOAD_LIMIT,
                                );
                            });
                        }
                    }

                    cache_home_page_covers(&mut page);
                    let _ = sender.send(BackgroundMessage::YouTubeStructuredPageCoversCached {
                        request_id,
                        title,
                        home: true,
                        append,
                        page,
                    });
                }
                Err(error) => {
                    let _ = sender.send(BackgroundMessage::YouTubeStructuredPage {
                        request_id,
                        title: "Para você".to_string(),
                        home: true,
                        append,
                        result: Err(error),
                    });
                }
            }
        });
    }

    pub(crate) fn load_youtube_library_overview(&self) {
        let Some(bridge) = self.youtube_bridge.clone() else {
            self.youtube_page
                .show_error("YouTube Music runtime is missing. Reinstall with --install-youtube.");
            return;
        };
        self.youtube_page
            .set_loading(true, "Carregando a visão geral da biblioteca...");
        let sender = self.background.sender();
        thread::spawn(move || {
            let _ = sender.send(BackgroundMessage::YouTubeStructuredPage {
                request_id: 0,
                title: "Sua biblioteca do YouTube Music".to_string(),
                home: false,
                append: false,
                result: bridge.library_overview().map(|mut page| {
                    cache_home_page_covers(&mut page);
                    page
                }),
            });
        });
    }

    fn prepare_native_youtube_route(&self) {
        self.youtube_page.close_host_dialog();
        self.close_settings_page();
        self.views.set_visible_child_name("music");
        self.music_stack.set_visible_child_name("library");
    }

    pub(crate) fn handle_youtube_events(&self) {
        while let Some(event) = self.youtube_page.try_recv() {
            let Some(bridge) = self.youtube_bridge.clone() else {
                self.youtube_page.show_error(
                    "YouTube Music runtime is missing. Run ./scripts/setup-youtube-runtime.sh for cargo run, or reinstall with ./install.sh --install-youtube.",
                );
                continue;
            };

            match event {
                YouTubePageEvent::AssistedLogin => {
                    self.present_assisted_youtube_login();
                }
                YouTubePageEvent::LoadHome {
                    continuation,
                    params,
                } => {
                    self.load_youtube_home_page(continuation, params);
                }
                YouTubePageEvent::LoadLibraryOverview => {
                    self.load_youtube_library_overview();
                }
                YouTubePageEvent::SyncLibrary => {
                    if self.sync_youtube_library(true, true) {
                        self.youtube_page
                            .set_loading(true, "Sincronizando com o Nocky...");
                    } else {
                        self.show_toast("A biblioteca já está sendo sincronizada");
                    }
                }
                YouTubePageEvent::Search { query, filter } => {
                    self.youtube_page
                        .set_loading(true, "Buscando no YouTube Music...");
                    let sender = self.background.sender();
                    thread::spawn(move || {
                        let result = bridge.search(&query, &filter);
                        let _ = sender.send(BackgroundMessage::YouTubeItems {
                            title: format!("Resultados para \"{query}\""),
                            result,
                        });
                    });
                }
                YouTubePageEvent::Connect(raw) => {
                    self.youtube_page
                        .set_loading(true, "Validando sessão do navegador...");
                    let sender = self.background.sender();
                    thread::spawn(move || {
                        let _ = sender.send(BackgroundMessage::YouTubeConnected(
                            bridge.connect_with_profile(&raw),
                        ));
                    });
                }
                YouTubePageEvent::Disconnect => {
                    self.youtube_page
                        .set_loading(true, "Desconectando conta...");
                    let sender = self.background.sender();
                    thread::spawn(move || {
                        let _ = sender.send(BackgroundMessage::YouTubeDisconnected(
                            bridge.disconnect_with_profile(),
                        ));
                    });
                }
                YouTubePageEvent::LoadLibrary => {
                    self.youtube_page
                        .set_loading(true, "Montando sua biblioteca...");
                    let sender = self.background.sender();
                    thread::spawn(move || {
                        let _ = sender.send(BackgroundMessage::YouTubeStructuredPage {
                            request_id: 0,
                            title: "Sua biblioteca do YouTube Music".to_string(),
                            home: false,
                            append: false,
                            result: bridge.library_page().map(|mut page| {
                                cache_home_page_covers(&mut page);
                                page
                            }),
                        });
                    });
                }
                YouTubePageEvent::LoadLiked => {
                    self.youtube_page
                        .set_loading(true, "Montando suas curtidas...");
                    let sender = self.background.sender();
                    thread::spawn(move || {
                        let _ = sender.send(BackgroundMessage::YouTubeStructuredPage {
                            request_id: 0,
                            title: "Suas curtidas no YouTube Music".to_string(),
                            home: false,
                            append: false,
                            result: bridge.liked_page().map(|mut page| {
                                cache_home_page_covers(&mut page);
                                page
                            }),
                        });
                    });
                }
                YouTubePageEvent::LoadPlaylists => {
                    self.youtube_page
                        .set_loading(true, "Carregando playlists...");
                    let sender = self.background.sender();
                    thread::spawn(move || {
                        let _ = sender.send(BackgroundMessage::YouTubeItems {
                            title: "Suas playlists".to_string(),
                            result: bridge.playlists(),
                        });
                    });
                }
                YouTubePageEvent::CreatePlaylist {
                    title,
                    description,
                    privacy,
                } => {
                    if self.youtube_playlist_create_pending.replace(true) {
                        self.show_toast("A criação anterior ainda está em andamento");
                        continue;
                    }

                    self.youtube_page
                        .set_loading(true, "Criando playlist no YouTube Music...");
                    let sender = self.background.sender();
                    thread::spawn(move || {
                        let result = bridge.create_empty_playlist(&title, &description, &privacy);
                        let _ = sender.send(BackgroundMessage::YouTubePlaylistCreated { result });
                    });
                }
                YouTubePageEvent::OpenPlaylist(item) => {
                    self.prepare_native_youtube_route();
                    self.load_youtube_playlist_for_browser(item);
                }
                YouTubePageEvent::OpenCollection(item) => {
                    self.prepare_native_youtube_route();
                    self.load_youtube_collection_for_browser(item);
                }
                YouTubePageEvent::UnsupportedItem { title, result_type } => {
                    let kind = match result_type.as_str() {
                        "podcast" => "Podcast",
                        "audiobook" => "Audiolivro",
                        "channel" => "Canal",
                        _ => "Item",
                    };
                    let detail = if title.trim().is_empty() {
                        kind.to_string()
                    } else {
                        format!("{kind} “{title}”")
                    };
                    self.show_toast(&format!(
                        "{detail} ainda não possui uma visualização compatível no Nocky"
                    ));
                }
                YouTubePageEvent::Activate { item, queue, index } => {
                    self.resolve_youtube_track(item, queue, index, false)
                }
            }
        }
    }

    pub(crate) fn load_youtube_collection_for_playback(&self, item: YouTubeItem, playlist: bool) {
        let Some(bridge) = self.youtube_bridge.clone() else {
            self.show_toast("As dependências do YouTube Music não estão instaladas");
            return;
        };

        let request_id = self
            .youtube_collection_play_request_id
            .get()
            .wrapping_add(1);
        self.youtube_collection_play_request_id.set(request_id);

        if playlist {
            if !item.browse_id.trim().is_empty() {
                self.youtube_library
                    .borrow_mut()
                    .playlist_loading
                    .insert(item.browse_id.clone());
            }
        } else {
            self.youtube_library
                .borrow_mut()
                .collection_loading
                .insert(youtube_collection_key("album", &item.title));
        }

        self.show_toast(if playlist {
            "Carregando playlist do YouTube Music…"
        } else {
            "Carregando álbum do YouTube Music…"
        });

        let sender = self.background.sender();
        thread::spawn(move || {
            let result = if playlist {
                bridge.playlist(&item)
            } else {
                bridge.collection(&item)
            }
            .map(|mut items| {
                cache_items_for_browser(&mut items);
                items
            });

            let _ = sender.send(BackgroundMessage::YouTubeCollectionPlaybackLoaded {
                request_id,
                item,
                playlist,
                result,
            });
        });
    }

    pub(crate) fn play_youtube_collection(&self, item: YouTubeItem, playlist: bool) {
        let kind = if playlist { "playlist" } else { "album" };
        let id = if item.browse_id.trim().is_empty() {
            item.title.to_lowercase()
        } else {
            item.browse_id.clone()
        };

        let items = {
            let library = self.youtube_library.borrow();
            if playlist {
                library
                    .playlist_tracks
                    .get(&item.browse_id)
                    .cloned()
                    .unwrap_or_default()
            } else {
                let key = youtube_collection_key("album", &item.title);
                library
                    .collection_tracks
                    .get(&key)
                    .cloned()
                    .unwrap_or_default()
            }
        };

        if items.is_empty() {
            self.load_youtube_collection_for_playback(item, playlist);
            return;
        }

        self.listening_history_context
            .replace(listening_history::PlaybackHistoryContext {
                kind: kind.to_string(),
                id,
                title: item.title.clone(),
            });
        self.pending_resume_position_us.set(None);
        self.resolve_youtube_track(items[0].clone(), items, 0, false);
    }

    pub(crate) fn set_youtube_favorite_visual_state(&self, active: bool) {
        self.favorite_icon
            .set_icon_name(Some("emblem-favorite-symbolic"));
        self.favorite_icon
            .set_opacity(if active { 0.98 } else { 0.28 });
        self.footer_favorite_icon
            .set_icon_name(Some("emblem-favorite-symbolic"));
        self.footer_favorite_icon
            .set_opacity(if active { 0.98 } else { 0.28 });

        for button in [&self.favorite_button, &self.footer_favorite_button] {
            if active {
                button.add_css_class("active");
            } else {
                button.remove_css_class("active");
            }
        }
    }

    pub(crate) fn current_youtube_item(&self) -> Option<YouTubeItem> {
        self.youtube_state
            .borrow()
            .as_ref()
            .map(|state| state.item.clone())
    }

    pub(crate) fn youtube_item_is_liked(&self, video_id: &str) -> bool {
        self.youtube_library
            .borrow()
            .liked
            .iter()
            .any(|item| item.video_id == video_id)
    }

    pub(crate) fn apply_youtube_like_cache(&self, item: &YouTubeItem, liked: bool) {
        let mut library = self.youtube_library.borrow_mut();
        library
            .liked
            .retain(|candidate| candidate.video_id != item.video_id);

        if liked {
            let mut stored = item.clone();
            if stored.result_type.is_empty() {
                stored.result_type = "song".to_string();
            }
            library.liked.insert(0, stored);
        }

        library.rebuild_collections();
        if let Err(error) = youtube_domain::save_library_cache(&library) {
            eprintln!("Could not persist YouTube liked songs: {error}");
        }
    }

    pub(crate) fn toggle_youtube_favorite(&self) {
        let Some(item) = self.current_youtube_item() else {
            self.show_toast("Nenhuma música do YouTube Music está selecionada");
            return;
        };
        self.toggle_youtube_item_favorite(item);
    }

    pub(crate) fn toggle_youtube_item_favorite(&self, item: YouTubeItem) {
        if item.video_id.trim().is_empty() {
            self.show_toast("Esta música não possui um identificador válido do YouTube");
            return;
        }

        if !self.youtube_library.borrow().connected {
            self.show_toast("Conecte sua conta do YouTube Music para curtir músicas");
            return;
        }

        if self
            .youtube_like_pending
            .borrow()
            .contains_key(&item.video_id)
        {
            self.show_toast("Aguarde a confirmação da curtida anterior");
            return;
        }

        let Some(bridge) = self.youtube_bridge.clone() else {
            self.show_toast("As dependências do YouTube Music não estão instaladas");
            return;
        };

        let previous_liked = self.youtube_item_is_liked(&item.video_id);
        let liked = !previous_liked;
        match self
            .youtube_like_mutations
            .borrow_mut()
            .begin(&item.video_id, previous_liked, liked)
        {
            Ok(_) => {}
            Err(LikeMutationStartError::AlreadyPending) => {
                self.show_toast("Aguarde a confirmação da curtida anterior");
                return;
            }
            Err(LikeMutationStartError::MissingId) => {
                self.show_toast("Esta música não possui um identificador válido do YouTube");
                return;
            }
            Err(LikeMutationStartError::Unchanged) => return,
        }

        let request_id = self.youtube_like_request_id.get().wrapping_add(1);
        self.youtube_like_request_id.set(request_id);
        self.youtube_like_pending
            .borrow_mut()
            .insert(item.video_id.clone(), request_id);

        self.apply_youtube_like_cache(&item, liked);

        if self
            .current_youtube_item()
            .is_some_and(|current| current.video_id == item.video_id)
        {
            self.set_youtube_favorite_visual_state(liked);
        }
        self.refresh_browser();

        let sender = self.background.sender();
        thread::spawn(move || {
            let result = bridge.rate(&item.video_id, liked);
            let _ = sender.send(BackgroundMessage::YouTubeRatingChanged {
                request_id,
                item,
                liked,
                result,
            });
        });
    }
}

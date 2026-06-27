//! YouTube controller methods for `AppController`.

use super::*;

impl AppController {
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
            let _ = sender.send(BackgroundMessage::YouTubeStatus(bridge.status()));
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
        thread::spawn(move || {
            let result = bridge.playlist(&playlist).map(|mut items| {
                cache_items_for_browser(&mut items);
                items
            });
            let _ = sender.send(BackgroundMessage::YouTubeBrowserPlaylist {
                request_id,
                playlist,
                result,
            });
        });
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
        if query.trim().is_empty()
            || self.config.borrow().startup_source != Some(StartupSource::YouTube)
            || self.search_query.borrow().trim() != query.as_str()
        {
            return;
        }

        let Some(bridge) = self.youtube_bridge.clone() else {
            self.youtube_library.borrow_mut().search = YouTubeSearchResults {
                query,
                error: "As dependências do YouTube Music não estão instaladas".to_string(),
                ..YouTubeSearchResults::default()
            };
            self.refresh_browser();
            return;
        };

        let request_id = self.youtube_search_request_id.get().wrapping_add(1);
        self.youtube_search_request_id.set(request_id);
        let mut cached = self.youtube_library.borrow().cached_search_results(&query);
        cached.loading = true;
        self.youtube_library.borrow_mut().search = cached;
        self.refresh_browser();

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

    pub(crate) fn handle_youtube_events(&self) {
        while let Some(event) = self.youtube_page.try_recv() {
            let Some(bridge) = self.youtube_bridge.clone() else {
                self.youtube_page.show_error(
                    "YouTube Music runtime is missing. Run ./scripts/setup-youtube-runtime.sh for cargo run, or reinstall with ./install.sh --install-youtube.",
                );
                continue;
            };

            match event {
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
                        let _ =
                            sender.send(BackgroundMessage::YouTubeConnected(bridge.connect(&raw)));
                    });
                }
                YouTubePageEvent::Disconnect => {
                    self.youtube_page
                        .set_loading(true, "Desconectando conta...");
                    let sender = self.background.sender();
                    thread::spawn(move || {
                        let _ = sender
                            .send(BackgroundMessage::YouTubeDisconnected(bridge.disconnect()));
                    });
                }
                YouTubePageEvent::LoadLibrary => {
                    self.youtube_page
                        .set_loading(true, "Carregando sua biblioteca...");
                    let sender = self.background.sender();
                    thread::spawn(move || {
                        let _ = sender.send(BackgroundMessage::YouTubeItems {
                            title: "Sua biblioteca do YouTube Music".to_string(),
                            result: bridge.library(),
                        });
                    });
                }
                YouTubePageEvent::LoadLiked => {
                    self.youtube_page
                        .set_loading(true, "Carregando músicas curtidas...");
                    let sender = self.background.sender();
                    thread::spawn(move || {
                        let _ = sender.send(BackgroundMessage::YouTubeItems {
                            title: "Músicas curtidas".to_string(),
                            result: bridge.liked(),
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
                YouTubePageEvent::OpenPlaylist(item) => {
                    let title = item.title.clone();
                    self.youtube_page
                        .set_loading(true, &format!("Carregando {title}..."));
                    let sender = self.background.sender();
                    thread::spawn(move || {
                        let _ = sender.send(BackgroundMessage::YouTubeItems {
                            title,
                            result: bridge.playlist(&item),
                        });
                    });
                }
                YouTubePageEvent::Activate { item, queue, index } => {
                    self.resolve_youtube_track(item, queue, index, false)
                }
            }
        }
    }
}

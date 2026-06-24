// youtube_collection_background_playback_v1
// collection_card_loading_spinner_v3\n// youtube_collection_queue_background_load_v1
// youtube_playlist_background_autoplay_v1
use crate::{
    background::BackgroundMessage,
    config::StartupSource,
    youtube::{
        cacheable_youtube_playlist, clear_library_cache, save_library_cache,
        youtube_collection_key, YouTubeSearchResults,
    },
    AppController,
};

impl AppController {
    pub(super) fn handle_background_messages(&self) {
        while let Ok(message) = self.background.try_recv() {
            match message {
                BackgroundMessage::LibraryScanned { root, result } => {
                    self.scanning.set(false);
                    if self.config.borrow().music_directory.as_ref() != Some(&root) {
                        continue;
                    }
                    match result {
                        Ok(paths) => self.apply_scanned_library(paths),
                        Err(error) => self.show_error(&error),
                    }
                }
                BackgroundMessage::LyricsDownloaded {
                    path,
                    result,
                    notify,
                } => {
                    self.lyrics_pending.borrow_mut().remove(&path);
                    match result {
                        Ok(()) => {
                            let current_track = {
                                let mut state = self.state.borrow_mut();
                                let current = state.current;
                                let mut changed = None;
                                if let Some((index, track)) = state
                                    .tracks
                                    .iter_mut()
                                    .enumerate()
                                    .find(|(_, track)| track.path == path)
                                {
                                    track.reload_lyrics();
                                    changed = Some((index, track.clone()));
                                }
                                changed.filter(|(index, _)| Some(*index) == current)
                            };

                            if let Some((_, track)) = current_track {
                                self.rebuild_lyrics(&track);
                            }
                            self.refresh_browser();
                            if notify {
                                self.show_toast("Letras sincronizadas baixadas");
                            }
                        }
                        Err(error) => {
                            if notify {
                                self.show_toast(&error);
                            }
                        }
                    }
                }
                BackgroundMessage::YouTubeLyricsDownloaded {
                    video_id,
                    notify,
                    result,
                } => {
                    let current = self.youtube_state.borrow().as_ref().map(|state| {
                        (
                            state.item.video_id.clone(),
                            state.item.title.clone(),
                            state.item.artist.clone(),
                        )
                    });
                    if current
                        .as_ref()
                        .map(|(current_id, _, _)| current_id.as_str())
                        != Some(video_id.as_str())
                    {
                        continue;
                    }

                    match result {
                        Ok(lyrics) => {
                            if let Some(state) = self.youtube_state.borrow_mut().as_mut() {
                                state.lyrics = lyrics.clone();
                            }
                            self.rebuild_youtube_lyrics(&lyrics);
                            if notify {
                                self.show_toast("Letras sincronizadas do YouTube carregadas");
                            }
                        }
                        Err(error) => {
                            let title = current
                                .as_ref()
                                .map(|(_, title, _)| title.as_str())
                                .unwrap_or("esta música");
                            self.set_lyrics_message(&format!(
                                "No synchronized lyrics were found for {title}. {error}"
                            ));
                        }
                    }
                }
                BackgroundMessage::YouTubeStatus(result) => match result {
                    Ok(status) => {
                        self.youtube_page.set_status(&status);
                        if status.connected {
                            self.youtube_library.borrow_mut().connected = true;
                            let syncing = self.config.borrow().youtube_auto_sync
                                && self.sync_youtube_library(true, false);
                            if syncing {
                                self.youtube_page.set_loading(
                                    true,
                                    "Sincronizando biblioteca do YouTube Music…",
                                );
                            } else {
                                self.prefetch_youtube_playlist_cache();
                                self.prefetch_home_artist_profiles();
                            }
                        } else {
                            self.youtube_library.borrow_mut().clear();
                            clear_library_cache();
                            self.refresh_browser();
                        }
                    }
                    Err(error) => self.youtube_page.show_error(&error),
                },
                BackgroundMessage::YouTubeConnected(result) => match result {
                    Ok(status) => {
                        self.youtube_page.set_status(&status);
                        self.youtube_page
                            .set_loading(false, "YouTube Music connected");
                        {
                            let mut library = self.youtube_library.borrow_mut();
                            library.connected = true;
                            library.synced = false;
                        }
                        let _ = self.sync_youtube_library(true, false);
                        self.show_toast("Conta do YouTube Music conectada");
                    }
                    Err(error) => {
                        self.youtube_page.show_error(&error);
                        self.show_toast("Não foi possível conectar o YouTube Music");
                    }
                },
                BackgroundMessage::YouTubeDisconnected(result) => match result {
                    Ok(status) => {
                        self.youtube_page.set_status(&status);
                        self.youtube_page.set_loading(false, "YouTube Music");
                        self.youtube_page
                            .show_empty("Search for music or connect your account.");
                        self.youtube_library.borrow_mut().clear();
                        clear_library_cache();
                        self.refresh_browser();
                        self.show_toast("Conta do YouTube Music desconectada");
                    }
                    Err(error) => self.youtube_page.show_error(&error),
                },
                BackgroundMessage::YouTubeLibrarySynced { notify, result } => match result {
                    Ok(snapshot) => {
                        let counts = (
                            snapshot.library.len(),
                            snapshot.liked.len(),
                            snapshot.playlists.len(),
                        );
                        let previous_signature =
                            self.youtube_library.borrow().presentation_signature();
                        self.youtube_library.borrow_mut().apply(snapshot);
                        let content_changed =
                            self.youtube_library.borrow().presentation_signature()
                                != previous_signature;
                        if let Err(error) = save_library_cache(&self.youtube_library.borrow()) {
                            eprintln!("Could not save the YouTube library cache: {error}");
                        }
                        self.youtube_page
                            .set_loading(false, "Library synchronized with Nocky");
                        if content_changed {
                            self.refresh_browser();
                        }
                        self.prefetch_youtube_playlist_cache();
                        self.prefetch_youtube_collection_cache();
                        if notify {
                            self.show_toast(&format!(
                                "YouTube Music sincronizado: {} faixas, {} curtidas e {} playlists",
                                counts.0, counts.1, counts.2
                            ));
                        }
                    }
                    Err(error) => {
                        self.youtube_library.borrow_mut().syncing = false;
                        self.youtube_page.set_loading(false, "YouTube Music");
                        self.show_toast(&format!(
                            "Não foi possível sincronizar a biblioteca: {error}"
                        ));
                    }
                },
                BackgroundMessage::YouTubeCollectionQueueLoaded {
                    request_id,
                    item,
                    playlist,
                    play_next,
                    result,
                } => {
                    if playlist {
                        if !item.browse_id.trim().is_empty() {
                            self.youtube_library
                                .borrow_mut()
                                .playlist_loading
                                .remove(&item.browse_id);
                        }
                    } else {
                        self.youtube_library
                            .borrow_mut()
                            .collection_loading
                            .remove(&youtube_collection_key("album", &item.title));
                    }

                    self.refresh_browser();

                    if request_id != self.youtube_collection_queue_request_id.get() {
                        continue;
                    }

                    match result {
                        Ok(items) if !items.is_empty() => {
                            if playlist {
                                self.youtube_library
                                    .borrow_mut()
                                    .playlist_tracks
                                    .insert(item.browse_id.clone(), items);
                            } else {
                                self.youtube_library
                                    .borrow_mut()
                                    .collection_tracks
                                    .insert(youtube_collection_key("album", &item.title), items);
                            }

                            if let Err(error) = save_library_cache(&self.youtube_library.borrow()) {
                                eprintln!("Could not save the YouTube collection cache: {error}");
                            }

                            self.enqueue_youtube_collection(&item, playlist, play_next);
                        }
                        Ok(_) => {
                            self.show_toast(if playlist {
                                "Esta playlist não retornou faixas reproduzíveis agora"
                            } else {
                                "Este álbum não retornou faixas reproduzíveis agora"
                            });
                        }
                        Err(error) => {
                            self.show_toast(&format!(
                                "Não foi possível carregar {}: {error}",
                                if playlist { "a playlist" } else { "o álbum" }
                            ));
                        }
                    }

                    self.refresh_browser();
                }
                BackgroundMessage::YouTubeCollectionPlaybackLoaded {
                    request_id,
                    item,
                    playlist,
                    result,
                } => {
                    let cache_key = if playlist {
                        item.browse_id.clone()
                    } else {
                        youtube_collection_key("album", &item.title)
                    };

                    if playlist {
                        if !cache_key.is_empty() {
                            self.youtube_library
                                .borrow_mut()
                                .playlist_loading
                                .remove(&cache_key);
                        }
                    } else {
                        self.youtube_library
                            .borrow_mut()
                            .collection_loading
                            .remove(&cache_key);
                    }

                    self.refresh_browser();

                    if request_id != self.youtube_collection_play_request_id.get() {
                        continue;
                    }

                    match result {
                        Ok(items) if !items.is_empty() => {
                            if playlist {
                                self.youtube_library
                                    .borrow_mut()
                                    .playlist_tracks
                                    .insert(cache_key.clone(), items);
                            } else {
                                self.youtube_library
                                    .borrow_mut()
                                    .collection_tracks
                                    .insert(cache_key.clone(), items);
                            }

                            let should_save = !playlist || cacheable_youtube_playlist(&item);
                            if should_save {
                                if let Err(error) =
                                    save_library_cache(&self.youtube_library.borrow())
                                {
                                    eprintln!(
                                        "Could not save the YouTube collection cache: {error}"
                                    );
                                }
                            }

                            self.show_toast(if playlist {
                                "Playlist carregada. Iniciando reprodução…"
                            } else {
                                "Álbum carregado. Iniciando reprodução…"
                            });
                            self.play_youtube_collection(item, playlist);
                        }
                        Ok(_) => {
                            if playlist {
                                self.youtube_library
                                    .borrow_mut()
                                    .playlist_tracks
                                    .remove(&cache_key);
                            } else {
                                self.youtube_library
                                    .borrow_mut()
                                    .collection_tracks
                                    .remove(&cache_key);
                            }

                            self.show_toast(if playlist {
                                "Esta playlist não retornou faixas reproduzíveis agora"
                            } else {
                                "Este álbum não retornou faixas reproduzíveis agora"
                            });
                            self.refresh_browser();
                        }
                        Err(error) => {
                            if playlist {
                                self.youtube_library
                                    .borrow_mut()
                                    .playlist_tracks
                                    .remove(&cache_key);
                            } else {
                                self.youtube_library
                                    .borrow_mut()
                                    .collection_tracks
                                    .remove(&cache_key);
                            }

                            self.show_toast(&format!(
                                "Não foi possível carregar {}: {error}",
                                if playlist { "a playlist" } else { "o álbum" }
                            ));
                            self.refresh_browser();
                        }
                    }
                }
                BackgroundMessage::YouTubeBrowserPlaylist {
                    request_id,
                    playlist,
                    result,
                } => match result {
                    Ok(items) => {
                        if request_id != self.youtube_playlist_request_id.get() {
                            continue;
                        }
                        self.youtube_playlist_loading.set(false);
                        let browse_id = playlist.browse_id.clone();
                        self.youtube_library
                            .borrow_mut()
                            .playlist_loading
                            .remove(&browse_id);

                        if items.is_empty() {
                            self.youtube_library
                                .borrow_mut()
                                .playlist_tracks
                                .remove(&browse_id);
                            if self.is_open_youtube_playlist(&browse_id) {
                                self.refresh_browser();
                            }
                            self.show_toast(
                                "Esta playlist não retornou faixas reproduzíveis agora",
                            );
                        } else {
                            self.youtube_library
                                .borrow_mut()
                                .playlist_tracks
                                .insert(browse_id.clone(), items);
                            if cacheable_youtube_playlist(&playlist) {
                                if let Err(error) =
                                    save_library_cache(&self.youtube_library.borrow())
                                {
                                    eprintln!("Could not save the YouTube playlist cache: {error}");
                                }
                            }
                            if self.is_open_youtube_playlist(&browse_id) {
                                self.refresh_browser();
                            }
                        }

                        let pending = self.youtube_pending_playlist.borrow_mut().take();
                        if let Some(pending) = pending {
                            self.load_youtube_playlist_for_browser(pending);
                        }
                    }
                    Err(error) => {
                        if request_id != self.youtube_playlist_request_id.get() {
                            continue;
                        }
                        self.youtube_playlist_loading.set(false);
                        let browse_id = playlist.browse_id.clone();
                        self.youtube_library
                            .borrow_mut()
                            .playlist_loading
                            .remove(&browse_id);
                        if self.is_open_youtube_playlist(&browse_id) {
                            self.refresh_browser();
                        }
                        self.show_toast(&format!("Não foi possível carregar a playlist: {error}"));
                        let pending = self.youtube_pending_playlist.borrow_mut().take();
                        if let Some(pending) = pending {
                            self.load_youtube_playlist_for_browser(pending);
                        }
                    }
                },
                BackgroundMessage::YouTubeArtistOverview { key, result } => {
                    self.youtube_library
                        .borrow_mut()
                        .artist_loading
                        .remove(&key);
                    match result {
                        Ok(overview) => {
                            let mut library = self.youtube_library.borrow_mut();
                            library
                                .artist_profiles
                                .insert(key.clone(), overview.profile);
                            library.artist_albums.insert(key.clone(), overview.albums);
                            drop(library);
                            if let Err(error) = save_library_cache(&self.youtube_library.borrow()) {
                                eprintln!("Could not save YouTube artist details: {error}");
                            }
                        }
                        Err(error) => {
                            if !error.contains("No YouTube Music artist could be resolved") {
                                eprintln!("Could not load YouTube artist details: {error}");
                            }
                            if self.is_open_youtube_collection(&key) {
                                self.show_toast(&format!(
                                    "Não foi possível carregar os álbuns do artista: {error}"
                                ));
                            }
                        }
                    }
                    if self.is_open_youtube_collection(&key) {
                        self.refresh_browser();
                    }
                }
                BackgroundMessage::YouTubeBrowserCollection { item, key, result } => {
                    self.youtube_library
                        .borrow_mut()
                        .collection_loading
                        .remove(&key);
                    match result {
                        Ok(items) if !items.is_empty() => {
                            self.youtube_library
                                .borrow_mut()
                                .collection_tracks
                                .insert(key.clone(), items);
                            if let Err(error) = save_library_cache(&self.youtube_library.borrow()) {
                                eprintln!("Could not save the YouTube collection cache: {error}");
                            }
                        }
                        Ok(_) => {
                            self.youtube_library
                                .borrow_mut()
                                .collection_tracks
                                .remove(&key);
                            self.show_toast(if item.result_type == "artist" {
                                "Este artista não retornou faixas reproduzíveis agora"
                            } else {
                                "Este álbum não retornou faixas reproduzíveis agora"
                            });
                        }
                        Err(error) => {
                            self.youtube_library
                                .borrow_mut()
                                .collection_tracks
                                .remove(&key);
                            self.show_toast(&format!(
                                "Não foi possível carregar {}: {error}",
                                if item.result_type == "artist" {
                                    "o artista"
                                } else {
                                    "o álbum"
                                }
                            ));
                        }
                    }
                    if self.is_open_youtube_collection(&key) {
                        self.refresh_browser();
                    }
                }
                BackgroundMessage::YouTubeCollectionsCached(result) => match result {
                    Ok(cached) => {
                        self.youtube_collection_prefetching.set(false);
                        if cached.is_empty() {
                            continue;
                        }
                        self.youtube_library
                            .borrow_mut()
                            .collection_tracks
                            .extend(cached);
                        if let Err(error) = save_library_cache(&self.youtube_library.borrow()) {
                            eprintln!("Could not save the YouTube collection cache: {error}");
                        }
                    }
                    Err(error) => {
                        self.youtube_collection_prefetching.set(false);
                        eprintln!("Could not pre-cache YouTube collections: {error}");
                    }
                },
                BackgroundMessage::YouTubePlaylistsCached(result) => match result {
                    Ok(cached) => {
                        self.youtube_playlist_prefetching.set(false);
                        if cached.is_empty() {
                            continue;
                        }
                        self.youtube_library
                            .borrow_mut()
                            .playlist_tracks
                            .extend(cached);
                        if let Err(error) = save_library_cache(&self.youtube_library.borrow()) {
                            eprintln!("Could not save the YouTube playlist cache: {error}");
                        }
                    }
                    Err(error) => {
                        self.youtube_playlist_prefetching.set(false);
                        eprintln!("Could not pre-cache YouTube playlists: {error}");
                    }
                },
                BackgroundMessage::YouTubeGlobalSearch {
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

                    let mut library = self.youtube_library.borrow_mut();
                    match result {
                        Ok(mut categorized) => {
                            categorized.loading = false;
                            library.search = categorized;
                        }
                        Err(error) => {
                            library.search = YouTubeSearchResults {
                                query,
                                error,
                                ..YouTubeSearchResults::default()
                            };
                        }
                    }
                    drop(library);
                    self.refresh_browser();
                }
                BackgroundMessage::YouTubeItems { title, result } => match result {
                    Ok(items) => self.youtube_page.show_items(&title, items),
                    Err(error) => self.youtube_page.show_error(&error),
                },
                BackgroundMessage::YouTubeResolved {
                    request_id,
                    queue,
                    index,
                    item,
                    result,
                } => {
                    if request_id != self.youtube_request_id.get() {
                        continue;
                    }
                    match result {
                        Ok((stream, cover)) => {
                            self.apply_youtube_track(queue, index, *item, stream, cover)
                        }
                        Err(error) => {
                            self.show_error(&error);
                            self.youtube_page.show_error(&error);
                        }
                    }
                }
            }
        }
    }
}

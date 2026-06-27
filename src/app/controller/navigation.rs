//! Navigation controller methods for `AppController`.

use super::*;

impl AppController {
    pub(crate) fn browser_playback_state(&self) -> BrowserPlaybackState {
        let context = self.listening_history_context.borrow();
        let youtube = self.youtube_library.borrow();
        let loading_collections = youtube
            .playlist_loading
            .iter()
            .chain(youtube.collection_loading.iter())
            .map(|key| key.trim().to_lowercase())
            .collect::<HashSet<_>>();

        BrowserPlaybackState {
            playing: self.play_icon.icon_name().as_deref() == Some("media-playback-pause-symbolic"),
            collection_kind: context.kind.clone(),
            collection_id: context.id.clone(),
            collection_title: context.title.clone(),
            loading_collections,
        }
    }

    pub(crate) fn refresh_artist_directory(&self) {
        if !matches!(self.browser.route(), BrowserRoute::Artists) {
            return;
        }

        let state = self.state.borrow();
        let config = self.config.borrow();
        let youtube = self.youtube_library.borrow();
        let query = self.search_query.borrow();
        let youtube_only = config.startup_source == Some(StartupSource::YouTube);
        let effective_tracks: &[Track] = if youtube_only {
            &[]
        } else {
            state.tracks.as_slice()
        };

        self.browser
            .refresh_artists_page(effective_tracks, &youtube, &query);
    }

    pub(crate) fn refresh_browser(&self) {
        let home_scroll_positions = self.browser.home_scroll_positions();
        let playback = self.browser_playback_state();
        let state = self.state.borrow();
        let config = self.config.borrow();
        let youtube = self.youtube_library.borrow();
        let query = self.search_query.borrow();
        let youtube_only = config.startup_source == Some(StartupSource::YouTube);
        let effective_tracks: &[Track] = if youtube_only {
            &[]
        } else {
            state.tracks.as_slice()
        };
        let mut effective_config = config.clone();
        if youtube_only {
            effective_config.playlists.clear();
        }
        let has_library = !query.trim().is_empty()
            || !effective_tracks.is_empty()
            || youtube.has_content()
            || youtube.syncing;
        self.music_stack
            .set_visible_child_name(if has_library { "library" } else { "empty" });
        self.browser.refresh(
            effective_tracks,
            &effective_config,
            &youtube,
            &BrowserRenderContext {
                history: &self.listening_history.borrow(),
                playback: &playback,
                offline: &self.offline_store.borrow(),
            },
            &query,
        );
        self.browser
            .restore_home_scroll_positions(home_scroll_positions);
        if !youtube_only {
            if let Some(current) = state.current {
                self.browser.select_track(current);
            }
        }
    }

    pub(crate) fn navigate_browser(&self, route: BrowserRoute) {
        if matches!(&route, BrowserRoute::Artists) {
            self.prefetch_home_artist_profiles(true);
        }
        let playback = self.browser_playback_state();
        let state = self.state.borrow();
        let config = self.config.borrow();
        let youtube = self.youtube_library.borrow();
        let query = self.search_query.borrow();
        let youtube_only = config.startup_source == Some(StartupSource::YouTube);
        let effective_tracks: &[Track] = if youtube_only {
            &[]
        } else {
            state.tracks.as_slice()
        };
        let mut effective_config = config.clone();
        if youtube_only {
            effective_config.playlists.clear();
        }
        self.browser.navigate(
            route.clone(),
            effective_tracks,
            &effective_config,
            &youtube,
            &BrowserRenderContext {
                history: &self.listening_history.borrow(),
                playback: &playback,
                offline: &self.offline_store.borrow(),
            },
            &query,
        );
        drop(query);
        drop(youtube);
        drop(config);
        drop(state);
        self.update_sidebar_active(&route);
        self.apply_footer_mode();
    }

    pub(crate) fn update_sidebar_active(&self, route: &BrowserRoute) {
        for button in [
            &self.sidebar_all,
            &self.sidebar_albums,
            &self.sidebar_artists,
            &self.sidebar_playlists,
            &self.sidebar_liked,
        ] {
            button.remove_css_class("active");
        }
        match route {
            BrowserRoute::All => self.sidebar_all.add_css_class("active"),
            BrowserRoute::Albums | BrowserRoute::Album(_) | BrowserRoute::YouTubeAlbum(_) => {
                self.sidebar_albums.add_css_class("active")
            }
            BrowserRoute::Artists | BrowserRoute::Artist(_) | BrowserRoute::YouTubeArtist(_) => {
                self.sidebar_artists.add_css_class("active")
            }
            BrowserRoute::Playlists
            | BrowserRoute::Playlist(_)
            | BrowserRoute::YouTubePlaylist { .. } => {
                self.sidebar_playlists.add_css_class("active")
            }
            BrowserRoute::Liked => self.sidebar_liked.add_css_class("active"),
        }
    }

    pub(crate) fn update_listening_history_context_from_route(&self) {
        let context = match self.browser.route() {
            BrowserRoute::Album(title) => listening_history::PlaybackHistoryContext {
                kind: "album".to_string(),
                id: title.to_lowercase(),
                title,
            },
            BrowserRoute::Playlist(title) => listening_history::PlaybackHistoryContext {
                kind: "playlist".to_string(),
                id: title.to_lowercase(),
                title,
            },
            BrowserRoute::YouTubeAlbum(collection) => listening_history::PlaybackHistoryContext {
                kind: "album".to_string(),
                id: collection.key,
                title: collection.title,
            },
            BrowserRoute::YouTubePlaylist { title, browse_id } => {
                listening_history::PlaybackHistoryContext {
                    kind: "playlist".to_string(),
                    id: if browse_id.is_empty() {
                        title.to_lowercase()
                    } else {
                        browse_id
                    },
                    title,
                }
            }
            _ => listening_history::PlaybackHistoryContext::default(),
        };
        self.listening_history_context.replace(context);
    }

    pub(crate) fn handle_browser_events(&self) {
        while let Some(event) = self.browser.try_recv() {
            match event {
                BrowserEvent::RefreshSearch => self.refresh_browser(),
                BrowserEvent::TrackActivated(index) => {
                    self.update_listening_history_context_from_route();
                    self.pending_resume_position_us.set(None);
                    self.prepare_playback_queue(index);
                    self.select_track(index, true);
                }
                BrowserEvent::ResumeLocalTrack {
                    index,
                    position_seconds,
                } => {
                    self.prepare_playback_queue(index);
                    self.select_track(index, true);
                    self.pending_resume_position_us.set(Some(
                        position_seconds
                            .saturating_mul(1_000_000)
                            .min(i64::MAX as u64) as i64,
                    ));
                }
                BrowserEvent::ResumeYouTubeTrack {
                    item,
                    position_seconds,
                } => {
                    self.pending_resume_position_us.set(Some(
                        position_seconds
                            .saturating_mul(1_000_000)
                            .min(i64::MAX as u64) as i64,
                    ));
                    self.resolve_youtube_track(item.clone(), vec![item], 0, false);
                }
                BrowserEvent::YouTubeTrackActivated { item, queue, index } => {
                    self.update_listening_history_context_from_route();
                    self.pending_resume_position_us.set(None);
                    self.resolve_youtube_track(item, queue, index, false);
                }
                BrowserEvent::QueueLocalPlayNext(index) => {
                    self.enqueue_local_track(index, true);
                }
                BrowserEvent::QueueLocalAppend(index) => {
                    self.enqueue_local_track(index, false);
                }
                BrowserEvent::QueueYouTubePlayNext(item) => {
                    self.enqueue_youtube_track(&item, true);
                }
                BrowserEvent::QueueYouTubeAppend(item) => {
                    self.enqueue_youtube_track(&item, false);
                }
                BrowserEvent::ToggleLocalTrackFavorite(index) => {
                    let path = self
                        .state
                        .borrow()
                        .tracks
                        .get(index)
                        .map(|track| track.path.clone());
                    if let Some(path) = path {
                        let liked = self.config.borrow_mut().toggle_liked(&path);
                        self.save_config();
                        if self.current_track_path().as_deref() == Some(path.as_path()) {
                            self.update_favorite_icon(&path);
                        }
                        self.refresh_browser();
                        self.show_toast(if liked {
                            self.tr(Message::AddedLiked)
                        } else {
                            self.tr(Message::RemovedLiked)
                        });
                    }
                }
                BrowserEvent::ToggleYouTubeTrackFavorite(item) => {
                    self.toggle_youtube_item_favorite(item);
                }
                BrowserEvent::DownloadYouTubeCollection { item, playlist } => {
                    self.download_youtube_collection(item, playlist);
                }
                BrowserEvent::QueueLocalCollection {
                    kind,
                    title,
                    play_next,
                } => {
                    self.enqueue_local_collection(&kind, &title, play_next);
                }
                BrowserEvent::QueueYouTubeCollection {
                    item,
                    playlist,
                    play_next,
                } => {
                    self.enqueue_youtube_collection(&item, playlist, play_next);
                }
                BrowserEvent::TogglePlayback => {
                    self.toggle_playback();
                }
                BrowserEvent::PlayLocalAlbum(title) => {
                    self.play_local_collection("album", &title);
                }
                BrowserEvent::PlayLocalPlaylist(title) => {
                    self.play_local_collection("playlist", &title);
                }
                BrowserEvent::PlayLocalMix { title, indices } => {
                    if let Some(first) = indices.first().copied() {
                        let artist = self
                            .state
                            .borrow()
                            .tracks
                            .get(first)
                            .map(|track| track.artist.clone())
                            .unwrap_or_default();

                        self.listening_history_context.replace(
                            listening_history::PlaybackHistoryContext {
                                kind: "mix".to_string(),
                                id: artist,
                                title,
                            },
                        );
                        self.pending_resume_position_us.set(None);
                        self.state.borrow_mut().playback_queue = indices;
                        self.select_track(first, true);
                    }
                }
                BrowserEvent::PlayYouTubeAlbum(item) => {
                    self.play_youtube_collection(item, false);
                }
                BrowserEvent::PlayYouTubePlaylist(item) => {
                    self.play_youtube_collection(item, true);
                }
                BrowserEvent::OpenYouTubePlaylist(item) => {
                    self.load_youtube_playlist_for_browser(item);
                }
                BrowserEvent::OpenYouTubeCollection(item) => {
                    self.load_youtube_collection_for_browser(item);
                }
                BrowserEvent::LoadMoreAlbums => {
                    self.browser.show_more_albums();
                    self.refresh_browser();
                }
                BrowserEvent::LoadMoreArtists => {
                    self.browser.show_more_artists();
                    self.prefetch_home_artist_profiles(true);
                    self.refresh_browser();
                }
                BrowserEvent::Navigate(route) => self.navigate_browser(route),
                BrowserEvent::CreatePlaylist(name) => {
                    let created = self.config.borrow_mut().create_playlist(&name);
                    if created {
                        self.save_config();
                        self.refresh_browser();
                        self.show_toast(&format!("Playlist ‘{name}’ criada"));
                    } else {
                        self.show_toast("Use um nome novo para a playlist");
                    }
                }
                BrowserEvent::AddCurrentToPlaylist(name) => {
                    let Some(path) = self.current_track_path() else {
                        self.show_toast("Selecione uma faixa primeiro");
                        continue;
                    };
                    let added = self.config.borrow_mut().add_to_playlist(&name, &path);
                    if added {
                        self.save_config();
                        self.refresh_browser();
                        self.show_toast(&format!("Faixa adicionada a ‘{name}’"));
                    } else {
                        self.show_toast("A faixa já está nessa playlist");
                    }
                }
                BrowserEvent::RemoveCurrentFromPlaylist(name) => {
                    let Some(path) = self.current_track_path() else {
                        self.show_toast("Selecione uma faixa primeiro");
                        continue;
                    };
                    let removed = self.config.borrow_mut().remove_from_playlist(&name, &path);
                    if removed {
                        self.save_config();
                        self.refresh_browser();
                        self.show_toast(&format!("Faixa removida de ‘{name}’"));
                    } else {
                        self.show_toast("A faixa não está nessa playlist");
                    }
                }
                BrowserEvent::DeletePlaylist(name) => {
                    if self.config.borrow_mut().delete_playlist(&name) {
                        self.save_config();
                        self.navigate_browser(BrowserRoute::Playlists);
                        self.show_toast(&format!("Playlist ‘{name}’ excluída"));
                    }
                }
                BrowserEvent::ToggleCollectionFavorite(key) => {
                    let added = self.config.borrow_mut().toggle_collection_favorite(&key);
                    self.save_config();
                    self.refresh_browser();
                    self.show_toast(if added {
                        "Coleção adicionada aos favoritos"
                    } else {
                        "Coleção removida dos favoritos"
                    });
                }
            }
        }
    }

    pub(crate) fn play_local_collection(&self, kind: &str, title: &str) {
        let mut indices = if kind == "playlist" {
            let paths = self
                .config
                .borrow()
                .playlist(title)
                .map(|playlist| playlist.tracks.clone())
                .unwrap_or_default();
            let state = self.state.borrow();
            paths
                .iter()
                .filter_map(|path| state.tracks.iter().position(|track| &track.path == path))
                .collect::<Vec<_>>()
        } else {
            let state = self.state.borrow();
            state
                .tracks
                .iter()
                .enumerate()
                .filter_map(|(index, track)| {
                    track.album.eq_ignore_ascii_case(title).then_some(index)
                })
                .collect::<Vec<_>>()
        };

        if kind == "album" {
            let state = self.state.borrow();
            indices.sort_by(|left, right| {
                let left = &state.tracks[*left];
                let right = &state.tracks[*right];
                left.disc_number
                    .unwrap_or(u32::MAX)
                    .cmp(&right.disc_number.unwrap_or(u32::MAX))
                    .then_with(|| {
                        left.track_number
                            .unwrap_or(u32::MAX)
                            .cmp(&right.track_number.unwrap_or(u32::MAX))
                    })
                    .then_with(|| left.title.to_lowercase().cmp(&right.title.to_lowercase()))
            });
        }

        let Some(first) = indices.first().copied() else {
            self.show_toast(if kind == "playlist" {
                "Esta playlist local ainda está vazia"
            } else {
                "Nenhuma faixa local foi encontrada para este álbum"
            });
            return;
        };

        self.listening_history_context
            .replace(listening_history::PlaybackHistoryContext {
                kind: kind.to_string(),
                id: title.to_lowercase(),
                title: title.to_string(),
            });
        self.pending_resume_position_us.set(None);
        self.state.borrow_mut().playback_queue = indices.clone();
        self.sync_local_queue_v2(&indices, first);
        self.select_track(first, true);
    }
}

//! Queue controller methods for `AppController`.

use super::*;

impl AppController {
    pub(crate) fn queue_source_kind(source: StartupSource) -> QueueSourceKind {
        match source {
            StartupSource::Local => QueueSourceKind::Local,
            StartupSource::YouTube => QueueSourceKind::YouTube,
        }
    }

    pub(crate) fn report_queue_recovery(&self, source: QueueSourceKind, discarded_entries: usize) {
        if discarded_entries == 0 {
            return;
        }

        eprintln!(
            "Queue 2.0 recovery for {source:?} discarded {discarded_entries} unavailable entr{}",
            if discarded_entries == 1 { "y" } else { "ies" }
        );
    }

    pub(crate) fn persist_active_queue_to_source(&self, context: &str) -> bool {
        let source = self.active_queue_source.get();
        let snapshot = self.playback_queue_v2.borrow().snapshot();

        match crate::playback::queue::save_for(source, &snapshot) {
            Ok(()) => {
                self.queue_last_saved_snapshot.replace(snapshot);
                true
            }
            Err(error) => {
                eprintln!("Could not save {context} Queue 2.0 state for {source:?}: {error}");
                false
            }
        }
    }

    pub(crate) fn switch_active_queue_source(&self, source: QueueSourceKind) {
        if self.active_queue_source.get() == source {
            return;
        }

        if !self.persist_active_queue_to_source("outgoing") {
            self.show_toast("Não foi possível salvar a fila atual antes de trocar de fonte");
            return;
        }

        self.persist_playback_session_now();
        self.maybe_record_listening();
        let _ = self.player.pause();
        self.update_play_icons(false);
        self.playback_source.set(PlaybackSource::None);
        self.state.borrow_mut().current = None;
        self.youtube_state.borrow_mut().take();
        self.queue_v2_pending_entry.set(None);
        self.queue_dragged_entry.set(None);

        let queue_load = crate::playback::queue::load_for(source);
        self.report_queue_recovery(source, queue_load.discarded_entries);
        let snapshot = queue_load.queue.snapshot();

        self.playback_queue_v2.replace(queue_load.queue);
        self.queue_last_saved_snapshot.replace(snapshot);
        self.active_queue_source.set(source);

        let restored_session = crate::playback::session::load_for(source);
        let restored_seconds = restored_session
            .as_ref()
            .map(|session| (session.position_us.max(0) as u64) / 1_000_000)
            .unwrap_or_default();
        let restored_shuffle = restored_session
            .as_ref()
            .is_some_and(|session| session.shuffle_enabled);
        let restored_repeat = restored_session
            .as_ref()
            .is_some_and(|session| session.repeat_enabled);
        self.restored_playback_session.replace(restored_session);
        self.playback_session_last_position_seconds
            .set(restored_seconds);
        self.playback_session_last_shuffle.set(restored_shuffle);
        self.playback_session_last_repeat.set(restored_repeat);
        self.shuffle_enabled.set(false);
        self.shuffle_button.set_active(false);
        self.footer_shuffle_button.set_active(false);
        self.repeat_button.set_active(false);
        self.footer_repeat_button.set_active(false);
        self.shuffle_navigation.borrow_mut().clear();
        self.playback_session_restore_attempts.set(0);
        self.pending_resume_position_us.set(None);
        self.startup_restore_autoplay.set(None);

        self.reset_shuffle_navigation(self.shuffle_enabled.get());
        self.publish_mpris_capabilities();
        self.update_footer_source();
        self.try_restore_playback_session();
    }

    pub(crate) fn initial_queue_entry_id(&self) -> Option<QueueEntryId> {
        let queue = self.playback_queue_v2.borrow();
        queue
            .current_id()
            .or_else(|| queue.entries().first().map(|entry| entry.id))
    }

    pub(crate) fn persist_queue_if_changed(&self) {
        let snapshot = self.playback_queue_v2.borrow().snapshot();
        if *self.queue_last_saved_snapshot.borrow() == snapshot {
            return;
        }

        let source = self.active_queue_source.get();
        match crate::playback::queue::save_for(source, &snapshot) {
            Ok(()) => {
                self.queue_last_saved_snapshot.replace(snapshot);
            }
            Err(error) => {
                eprintln!("Could not save Queue 2.0 state for {source:?}: {error}");
            }
        }
    }

    pub(crate) fn persist_queue_now(&self) {
        let _ = self.persist_active_queue_to_source("final");
    }

    // queue2_playback_bridge_v1
    pub(crate) fn enqueue_browser_media(&self, media: QueueMedia, play_next: bool) {
        let expected = self.active_queue_source.get();
        if media.source.kind() != expected {
            eprintln!(
                "Rejected Queue 2.0 enqueue: active source is {expected:?}, media source is {:?}",
                media.source.kind()
            );
            self.show_toast("Esta faixa pertence a outra fonte de reprodução");
            return;
        }

        self.ensure_active_queue_v2();
        let title = media.title.clone();

        if play_next {
            self.playback_queue_v2.borrow_mut().insert_next(media);
        } else {
            self.playback_queue_v2.borrow_mut().append(media);
        }

        let message = match (self.config.borrow().language, play_next) {
            (AppLanguage::Portuguese, true) => format!("‘{title}’ será reproduzida em seguida"),
            (AppLanguage::Portuguese, false) => format!("‘{title}’ foi adicionada ao fim da fila"),
            (AppLanguage::English, true) => format!("‘{title}’ will play next"),
            (AppLanguage::English, false) => format!("‘{title}’ was added to the queue"),
            (AppLanguage::Spanish, true) => format!("‘{title}’ se reproducirá a continuación"),
            (AppLanguage::Spanish, false) => {
                format!("‘{title}’ se añadió al final de la cola")
            }
        };
        self.show_toast(&message);
    }

    pub(crate) fn enqueue_local_track(&self, index: usize, play_next: bool) {
        let media = self
            .state
            .borrow()
            .tracks
            .get(index)
            .map(Self::local_queue_media);
        if let Some(media) = media {
            self.enqueue_browser_media(media, play_next);
        }
    }

    pub(crate) fn enqueue_youtube_track(&self, item: &YouTubeItem, play_next: bool) {
        if item.playable() {
            self.enqueue_browser_media(Self::youtube_queue_media(item), play_next);
        }
    }

    pub(crate) fn enqueue_media_collection(
        &self,
        media: Vec<QueueMedia>,
        play_next: bool,
        title: &str,
    ) {
        if media.is_empty() {
            return;
        }

        let expected = self.active_queue_source.get();
        if media.iter().any(|item| item.source.kind() != expected) {
            eprintln!("Rejected Queue 2.0 collection enqueue: active source is {expected:?}");
            self.show_toast("Esta coleção pertence a outra fonte de reprodução");
            return;
        }

        self.ensure_active_queue_v2();
        let count = media.len();

        if play_next {
            let mut queue = self.playback_queue_v2.borrow_mut();
            for item in media.into_iter().rev() {
                queue.insert_next(item);
            }
        } else {
            let mut queue = self.playback_queue_v2.borrow_mut();
            for item in media {
                queue.append(item);
            }
        }

        let message = match (self.config.borrow().language, play_next) {
            (AppLanguage::Portuguese, true) => {
                format!("‘{title}’ ({count} faixas) será reproduzido em seguida")
            }
            (AppLanguage::Portuguese, false) => {
                format!("‘{title}’ ({count} faixas) foi adicionado ao fim da fila")
            }
            (AppLanguage::English, true) => {
                format!("‘{title}’ ({count} tracks) will play next")
            }
            (AppLanguage::English, false) => {
                format!("‘{title}’ ({count} tracks) was added to the queue")
            }
            (AppLanguage::Spanish, true) => {
                format!("‘{title}’ ({count} pistas) se reproducirá a continuación")
            }
            (AppLanguage::Spanish, false) => {
                format!("‘{title}’ ({count} pistas) se añadió al final de la cola")
            }
        };
        self.show_toast(&message);
    }

    pub(crate) fn enqueue_local_collection(&self, kind: &str, title: &str, play_next: bool) {
        let indices = if kind == "playlist" {
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
            let mut indices = state
                .tracks
                .iter()
                .enumerate()
                .filter_map(|(index, track)| {
                    track.album.eq_ignore_ascii_case(title).then_some(index)
                })
                .collect::<Vec<_>>();
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
            indices
        };

        let media = {
            let state = self.state.borrow();
            indices
                .iter()
                .filter_map(|index| state.tracks.get(*index))
                .map(Self::local_queue_media)
                .collect::<Vec<_>>()
        };

        if media.is_empty() {
            self.show_toast(if kind == "playlist" {
                "Esta playlist local ainda está vazia"
            } else {
                "Nenhuma faixa local foi encontrada para este álbum"
            });
            return;
        }

        self.enqueue_media_collection(media, play_next, title);
    }

    pub(crate) fn enqueue_youtube_collection(
        &self,
        item: &YouTubeItem,
        playlist: bool,
        play_next: bool,
    ) {
        let (items, collection_cover) = {
            let library = self.youtube_library.borrow();
            let items = if playlist {
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
            };

            let collection_cover = item.cached_cover().map(Path::to_path_buf).or_else(|| {
                (!playlist)
                    .then(|| {
                        library
                            .albums
                            .iter()
                            .find(|entry| {
                                (!item.browse_id.trim().is_empty()
                                    && entry.source.browse_id == item.browse_id)
                                    || entry.title.eq_ignore_ascii_case(&item.title)
                            })
                            .and_then(|entry| entry.cached_cover().map(Path::to_path_buf))
                    })
                    .flatten()
            });

            (items, collection_cover)
        };

        let media = items
            .iter()
            .filter(|track| track.playable())
            .map(|track| {
                Self::youtube_queue_media_with_fallback(track, collection_cover.as_deref())
            })
            .collect::<Vec<_>>();

        if media.is_empty() {
            self.load_youtube_collection_for_queue(item.clone(), playlist, play_next);
            return;
        }

        self.enqueue_media_collection(media, play_next, &item.title);
    }

    pub(crate) fn load_youtube_collection_for_queue(
        &self,
        item: YouTubeItem,
        playlist: bool,
        play_next: bool,
    ) {
        let Some(bridge) = self.youtube_bridge.clone() else {
            self.show_toast("As dependências do YouTube Music não estão instaladas");
            return;
        };

        let request_id = self
            .youtube_collection_queue_request_id
            .get()
            .wrapping_add(1);
        self.youtube_collection_queue_request_id.set(request_id);

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

        let message = match (self.config.borrow().language, play_next) {
            (AppLanguage::Portuguese, true) => "Carregando coleção para reproduzir em seguida…",
            (AppLanguage::Portuguese, false) => "Carregando coleção para adicionar à fila…",
            (AppLanguage::English, true) => "Loading collection to play next…",
            (AppLanguage::English, false) => "Loading collection to add to queue…",
            (AppLanguage::Spanish, true) => "Cargando colección para reproducir a continuación…",
            (AppLanguage::Spanish, false) => "Cargando colección para añadirla a la cola…",
        };
        self.show_toast(message);
        self.refresh_browser();

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

            let _ = sender.send(BackgroundMessage::YouTubeCollectionQueueLoaded {
                request_id,
                item,
                playlist,
                play_next,
                result,
            });
        });
    }

    pub(crate) fn local_queue_media(track: &Track) -> QueueMedia {
        QueueMedia::local(
            track.path.clone(),
            track.title.clone(),
            track.artist.clone(),
            track.album.clone(),
            track.duration_seconds,
            track.cover_path.clone(),
        )
    }

    pub(crate) fn youtube_queue_media(item: &YouTubeItem) -> QueueMedia {
        Self::youtube_queue_media_with_fallback(item, None)
    }

    pub(crate) fn youtube_queue_media_with_fallback(
        item: &YouTubeItem,
        fallback_cover: Option<&Path>,
    ) -> QueueMedia {
        let cover_path = item
            .cached_cover()
            .map(Path::to_path_buf)
            .or_else(|| fallback_cover.map(Path::to_path_buf));

        QueueMedia::youtube(
            item.video_id.clone(),
            item.title.clone(),
            item.artist.clone(),
            item.album.clone(),
            item.duration_seconds,
            cover_path,
        )
    }

    pub(crate) fn sync_local_queue_v2(&self, sequence: &[usize], selected: usize) {
        let (media, selected_position) = {
            let state = self.state.borrow();
            let media = sequence
                .iter()
                .filter_map(|index| state.tracks.get(*index))
                .map(Self::local_queue_media)
                .collect::<Vec<_>>();
            let selected_position = sequence.iter().position(|index| *index == selected);
            (media, selected_position)
        };

        let incoming_keys = media
            .iter()
            .map(|item| item.source.stable_key())
            .collect::<Vec<_>>();
        let current_keys = self
            .playback_queue_v2
            .borrow()
            .entries()
            .iter()
            .map(|entry| entry.media.source.stable_key())
            .collect::<Vec<_>>();

        let mut queue = self.playback_queue_v2.borrow_mut();
        if incoming_keys != current_keys {
            queue.replace(media, selected_position);
        } else if let Some(position) = selected_position {
            queue.select_index(position);
        }
    }

    pub(crate) fn sync_youtube_queue_v2(&self, items: &[YouTubeItem], selected: usize) {
        let media = items
            .iter()
            .filter(|item| item.playable())
            .map(Self::youtube_queue_media)
            .collect::<Vec<_>>();
        let selected_video_id = items.get(selected).map(|item| item.video_id.as_str());
        let selected_position = selected_video_id.and_then(|video_id| {
            media.iter().position(|item| {
                matches!(
                    &item.source,
                    QueueSource::YouTube {
                        video_id: candidate
                    } if candidate == video_id
                )
            })
        });

        let incoming_keys = media
            .iter()
            .map(|item| item.source.stable_key())
            .collect::<Vec<_>>();
        let current_keys = self
            .playback_queue_v2
            .borrow()
            .entries()
            .iter()
            .map(|entry| entry.media.source.stable_key())
            .collect::<Vec<_>>();

        let mut queue = self.playback_queue_v2.borrow_mut();
        if incoming_keys != current_keys {
            queue.replace(media, selected_position);
        } else if let Some(position) = selected_position {
            queue.select_index(position);
        }
    }

    pub(crate) fn ensure_local_queue_v2(&self, selected: usize) {
        let selected_path = {
            let state = self.state.borrow();
            state.tracks.get(selected).map(|track| track.path.clone())
        };
        let Some(selected_path) = selected_path else {
            return;
        };

        let matching_id = self
            .playback_queue_v2
            .borrow()
            .entries()
            .iter()
            .find_map(|entry| match &entry.media.source {
                QueueSource::Local { path } if path == &selected_path => Some(entry.id),
                _ => None,
            });

        if let Some(id) = matching_id {
            let _ = self.playback_queue_v2.borrow_mut().select(id);
            return;
        }

        let sequence = self.playback_sequence();
        self.sync_local_queue_v2(&sequence, selected);
    }

    pub(crate) fn ensure_active_queue_v2(&self) {
        let playback_kind = match self.playback_source.get() {
            PlaybackSource::Local => Some(QueueSourceKind::Local),
            PlaybackSource::YouTube => Some(QueueSourceKind::YouTube),
            PlaybackSource::None => None,
        };
        if playback_kind.is_some_and(|kind| kind != self.active_queue_source.get()) {
            return;
        }

        match self.playback_source.get() {
            PlaybackSource::Local => {
                if let Some(selected) = self.state.borrow().current {
                    self.ensure_local_queue_v2(selected);
                }
            }
            PlaybackSource::YouTube => {
                let snapshot = {
                    let state = self.youtube_state.borrow();
                    state.as_ref().map(|state| {
                        (
                            state.queue.clone(),
                            state.current,
                            state.item.video_id.clone(),
                        )
                    })
                };
                let Some((items, current, video_id)) = snapshot else {
                    return;
                };

                let matching_id =
                    self.playback_queue_v2
                        .borrow()
                        .entries()
                        .iter()
                        .find_map(|entry| match &entry.media.source {
                            QueueSource::YouTube {
                                video_id: candidate,
                            } if candidate == &video_id => Some(entry.id),
                            _ => None,
                        });

                if let Some(id) = matching_id {
                    let _ = self.playback_queue_v2.borrow_mut().select(id);
                } else {
                    self.sync_youtube_queue_v2(&items, current);
                }
            }
            PlaybackSource::None => {}
        }
    }

    pub(crate) fn reset_shuffle_navigation(&self, enabled: bool) {
        let mut rng = self.rng_state.get();

        if enabled {
            let queue = self.playback_queue_v2.borrow();
            self.shuffle_navigation.borrow_mut().reset(
                queue.entries(),
                queue.current_id(),
                &mut rng,
            );
        } else {
            self.shuffle_navigation.borrow_mut().clear();
        }

        self.rng_state.set(rng);
    }

    pub(crate) fn next_queue_entry_id(&self) -> Option<QueueEntryId> {
        let queue = self.playback_queue_v2.borrow();

        if !self.shuffle_enabled.get() {
            return match queue.current_index() {
                Some(position) => queue.entries().get(position + 1).map(|entry| entry.id),
                None => queue.entries().first().map(|entry| entry.id),
            };
        }

        let mut rng = self.rng_state.get();
        let next = self.shuffle_navigation.borrow_mut().next(
            queue.entries(),
            queue.current_id(),
            &mut rng,
        );
        self.rng_state.set(rng);
        next
    }

    pub(crate) fn previous_queue_entry_id(&self) -> Option<QueueEntryId> {
        let queue = self.playback_queue_v2.borrow();

        if !self.shuffle_enabled.get() {
            return queue
                .current_index()
                .and_then(|position| position.checked_sub(1))
                .and_then(|position| queue.entries().get(position))
                .map(|entry| entry.id);
        }

        let mut rng = self.rng_state.get();
        let previous = self.shuffle_navigation.borrow_mut().previous(
            queue.entries(),
            queue.current_id(),
            &mut rng,
        );
        self.rng_state.set(rng);
        previous
    }

    pub(crate) fn prepare_playback_queue(&self, selected: usize) {
        let mut sequence = self.browser.visible_indices();
        if sequence.is_empty() || !sequence.contains(&selected) {
            sequence = (0..self.state.borrow().tracks.len()).collect();
        }
        self.state.borrow_mut().playback_queue = sequence.clone();
        self.sync_local_queue_v2(&sequence, selected);
    }

    pub(crate) fn playback_sequence(&self) -> Vec<usize> {
        let state = self.state.borrow();
        if !state.playback_queue.is_empty()
            && state
                .current
                .is_none_or(|current| state.playback_queue.contains(&current))
        {
            return state.playback_queue.clone();
        }
        drop(state);

        let visible = self.browser.visible_indices();
        if !visible.is_empty() {
            return visible;
        }
        match self.browser.route() {
            BrowserRoute::Albums
            | BrowserRoute::Artists
            | BrowserRoute::Playlists
            | BrowserRoute::YouTubeAlbum(_)
            | BrowserRoute::YouTubeArtist(_)
            | BrowserRoute::YouTubePlaylist { .. } => {
                (0..self.state.borrow().tracks.len()).collect()
            }
            _ => visible,
        }
    }
}

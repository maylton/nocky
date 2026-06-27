//! Offline download controller methods for `AppController`.

use super::*;

impl AppController {
    pub(crate) fn download_youtube_collection(&self, item: YouTubeItem, playlist: bool) {
        self.download_youtube_collection_with_mode(item, playlist, false);
    }

    pub(crate) fn download_youtube_collection_automatically(
        &self,
        item: YouTubeItem,
        playlist: bool,
    ) {
        self.download_youtube_collection_with_mode(item, playlist, true);
    }

    pub(crate) fn download_youtube_collection_with_mode(
        &self,
        item: YouTubeItem,
        playlist: bool,
        automatic: bool,
    ) {
        let collection_id = if playlist {
            format!("playlist:{}", item.browse_id)
        } else {
            format!("album:{}", youtube_collection_cache_key(&item))
        };
        if !automatic {
            if let Err(error) =
                self.offline_store
                    .borrow_mut()
                    .follow_collection(&collection_id, &item, playlist)
            {
                self.show_toast(&error);
                return;
            }
        }

        if !self
            .offline_download_pending
            .borrow_mut()
            .insert(collection_id.clone())
        {
            if !automatic {
                self.show_toast("Esta coleção já está sendo baixada");
            }
            return;
        }

        let items = if playlist {
            self.youtube_library
                .borrow()
                .playlist_tracks
                .get(&item.browse_id)
                .cloned()
                .unwrap_or_default()
        } else {
            self.youtube_library
                .borrow()
                .collection_tracks
                .get(&youtube_collection_cache_key(&item))
                .cloned()
                .unwrap_or_default()
        };
        let items = items
            .into_iter()
            .filter(|track| {
                let store = self.offline_store.borrow();
                track.playable()
                    && !store.contains(&track.video_id)
                    && !store.is_unavailable(&track.video_id)
            })
            .collect::<Vec<_>>();
        if items.is_empty() {
            self.offline_download_pending
                .borrow_mut()
                .remove(&collection_id);
            self.browser
                .set_collection_offline_complete(&collection_id, self.config.borrow().language);
            if !automatic {
                self.show_toast("Esta coleção já está disponível offline");
            }
            return;
        }
        let Some(bridge) = self.youtube_bridge.clone() else {
            self.offline_download_pending
                .borrow_mut()
                .remove(&collection_id);
            self.browser
                .set_collection_offline_retry(&collection_id, self.config.borrow().language);
            if !automatic {
                self.show_toast("As dependências do YouTube Music não estão instaladas");
            }
            return;
        };

        let collection_title = item.title.clone();
        let total = items.len();
        self.browser.set_collection_offline_downloading(
            &collection_id,
            0,
            total,
            self.config.borrow().language,
        );
        let sender = self.background.sender();
        if !automatic {
            self.show_toast(&format!("Baixando {total} faixas de ‘{collection_title}’…"));
        }
        thread::spawn(move || {
            let mut completed = 0;
            let mut failed = 0;
            for track in items {
                let first_result = bridge
                    .resolve(&track.video_id, false)
                    .and_then(|stream| download_youtube_track(&track, &stream));

                let result = match first_result {
                    Err(error) if error.starts_with(OFFLINE_STREAM_REJECTED_PREFIX) => {
                        eprintln!(
                            "Nocky offline stream for '{}' was rejected; refreshing the signed URL once",
                            track.title
                        );
                        bridge
                            .resolve(&track.video_id, true)
                            .and_then(|stream| download_youtube_track(&track, &stream))
                    }
                    other => other,
                };

                if let Err(error) = result.as_ref() {
                    if let Some(reason) = error.strip_prefix("__NOCKY_PREMIUM_STREAM_UNAVAILABLE__")
                    {
                        let mut store = OfflineStore::load_default();
                        if let Err(save_error) =
                            store.mark_unavailable(&track.video_id, reason.trim())
                        {
                            eprintln!(
                                "Could not persist unsupported Premium track '{}': {save_error}",
                                track.title
                            );
                        }
                    }
                }

                if result.is_ok() {
                    completed += 1;
                } else {
                    failed += 1;
                }
                let _ = sender.send(BackgroundMessage::OfflineCollectionProgress {
                    collection_id: collection_id.clone(),
                    completed,
                    total,
                    item: Box::new(track),
                    result,
                });
            }
            let _ = sender.send(BackgroundMessage::OfflineCollectionFinished {
                collection_id,
                collection_title,
                completed,
                failed,
                automatic,
            });
        });
    }

    pub(crate) fn sync_followed_offline_collections(&self) {
        if !self.config.borrow().offline_collection_auto_sync {
            return;
        }

        let followed = self.offline_store.borrow().followed_collections();
        if followed.is_empty() {
            return;
        }

        let ready = {
            let library = self.youtube_library.borrow();
            followed
                .into_iter()
                .filter_map(|collection| {
                    let cache_ready = if collection.playlist {
                        library
                            .playlist_tracks
                            .get(&collection.item.browse_id)
                            .is_some_and(|tracks| !tracks.is_empty())
                    } else {
                        library
                            .collection_tracks
                            .get(&youtube_collection_cache_key(&collection.item))
                            .is_some_and(|tracks| !tracks.is_empty())
                    };

                    cache_ready.then_some((collection.item, collection.playlist))
                })
                .collect::<Vec<_>>()
        };

        for (item, playlist) in ready {
            self.download_youtube_collection_automatically(item, playlist);
        }
    }
}

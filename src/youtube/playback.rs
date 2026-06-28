use crate::{
    app::controller::AppController,
    app::media::{is_refreshable_stream_error, mpris_youtube_track_id, redact_stream_url},
    app::state::{PlaybackSource, YouTubePlaybackState},
    background::BackgroundMessage,
    lyrics,
    playback::queue::{PlaybackQueue, QueueEntryId, QueueSource},
    youtube::{download_cover, save_library_cache, YouTubeItem, YouTubeStream},
};
use gtk::{
    gio,
    prelude::{FileExt, WidgetExt},
};
use std::{
    path::{Path, PathBuf},
    thread,
};

const YOUTUBE_RECOVERY_MAX_ATTEMPTS: u8 = 3;

fn youtube_recovery_delay_seconds(next_attempt: u8) -> Option<u64> {
    match next_attempt {
        2 => Some(1),
        3 => Some(3),
        _ => None,
    }
}
fn matching_youtube_queue_entry(
    queue: &PlaybackQueue,
    preferred: Option<QueueEntryId>,
    video_id: &str,
) -> Option<QueueEntryId> {
    let matches_video = |id: QueueEntryId| {
        queue.entry(id).is_some_and(|entry| {
            matches!(
                &entry.media.source,
                QueueSource::YouTube {
                    video_id: candidate
                } if candidate == video_id
            )
        })
    };

    preferred
        .filter(|id| matches_video(*id))
        .or_else(|| queue.current_id().filter(|id| matches_video(*id)))
        .or_else(|| {
            queue.entries().iter().find_map(|entry| {
                matches!(
                    &entry.media.source,
                    QueueSource::YouTube {
                        video_id: candidate
                    } if candidate == video_id
                )
                .then_some(entry.id)
            })
        })
}

impl AppController {
    pub(crate) fn resolve_youtube_track(
        &self,
        item: YouTubeItem,
        queue: Vec<YouTubeItem>,
        index: usize,
        force: bool,
    ) {
        if item.video_id.is_empty() {
            return;
        }
        if !force {
            if let Some(path) = self.offline_store.borrow().resolve(&item.video_id) {
                let stream = YouTubeStream {
                    video_id: item.video_id.clone(),
                    stream_url: gio::File::for_path(&path).uri().to_string(),
                    webpage_url: format!("https://music.youtube.com/watch?v={}", item.video_id),
                    title: item.title.clone(),
                    artist: item.artist.clone(),
                    album: item.album.clone(),
                    duration_seconds: item.duration_seconds,
                    thumbnail_url: item.thumbnail_url.clone(),
                    http_headers: Default::default(),
                    expires_at: 0.0,
                    ..YouTubeStream::default()
                };
                let cover = item.cached_cover().map(Path::to_path_buf);
                self.apply_youtube_track(queue, index, item, stream, cover);
                return;
            }
        }
        let Some(bridge) = self.youtube_bridge.clone() else {
            self.show_toast("As dependências do YouTube Music não estão instaladas");
            return;
        };
        let request_id = self.youtube_request_id.get().wrapping_add(1);
        self.youtube_request_id.set(request_id);
        let sender = self.background.sender();
        thread::spawn(move || {
            let result = bridge
                .resolve(&item.video_id, force)
                .map(|stream| {
                    let cover = download_cover(&item, &stream.thumbnail_url);
                    (stream, cover)
                })
                .map_err(|error| {
                    if force {
                        format!("__NOCKY_STREAM_RECOVERY_FAILED__{error}")
                    } else {
                        error
                    }
                });
            let _ = sender.send(BackgroundMessage::YouTubeResolved {
                request_id,
                queue,
                index,
                item: Box::new(item),
                result,
            });
        });
    }

    pub(crate) fn try_recover_youtube_stream(&self, error: &str) -> bool {
        if self.playback_source.get() != PlaybackSource::YouTube
            || self.youtube_recovery_in_progress.get()
            || self.youtube_recovery_attempted.get()
            || !is_refreshable_stream_error(error)
        {
            return false;
        }

        let snapshot = {
            let state = self.youtube_state.borrow();
            state
                .as_ref()
                .map(|state| (state.queue.clone(), state.current, state.item.clone()))
        };
        let Some((queue, index, item)) = snapshot else {
            return false;
        };

        let recovery_entry = {
            let playback_queue = self.playback_queue_v2.borrow();
            matching_youtube_queue_entry(
                &playback_queue,
                playback_queue.current_id(),
                &item.video_id,
            )
        };
        self.queue_v2_pending_entry.set(recovery_entry);

        self.youtube_recovery_attempted.set(true);
        self.youtube_recovery_retry_count.set(1);
        self.youtube_recovery_generation
            .set(self.youtube_recovery_generation.get().wrapping_add(1));
        self.youtube_recovery_in_progress.set(true);
        self.youtube_recovery_was_playing
            .set(self.player.is_playing());
        self.youtube_recovery_resume_us
            .set(self.player.position_us().max(0));
        let _ = self.player.stop();

        eprintln!(
            "Nocky YouTube stream rejected; refreshing signed URL (attempt 1/{YOUTUBE_RECOVERY_MAX_ATTEMPTS}): {}",
            redact_stream_url(error)
        );
        self.resolve_youtube_track(item, queue, index, true);
        true
    }

    pub(crate) fn schedule_youtube_recovery_retry(
        &self,
        queue: Vec<YouTubeItem>,
        index: usize,
        item: YouTubeItem,
    ) -> bool {
        let next_attempt = self.youtube_recovery_retry_count.get().saturating_add(1);
        let Some(delay_seconds) = youtube_recovery_delay_seconds(next_attempt) else {
            return false;
        };
        if next_attempt > YOUTUBE_RECOVERY_MAX_ATTEMPTS {
            return false;
        }

        self.youtube_recovery_retry_count.set(next_attempt);
        self.youtube_recovery_in_progress.set(true);
        let generation = self.youtube_recovery_generation.get();
        let sender = self.background.sender();

        eprintln!(
            "Nocky YouTube recovery attempt {next_attempt}/{YOUTUBE_RECOVERY_MAX_ATTEMPTS} scheduled in {delay_seconds}s"
        );

        thread::spawn(move || {
            thread::sleep(std::time::Duration::from_secs(delay_seconds));
            let _ = sender.send(BackgroundMessage::YouTubeRecoveryRetry {
                generation,
                queue,
                index,
                item: Box::new(item),
            });
        });
        true
    }

    pub(crate) fn reset_youtube_recovery(&self) {
        self.youtube_recovery_in_progress.set(false);
        self.youtube_recovery_attempted.set(false);
        self.youtube_recovery_retry_count.set(0);
        self.youtube_recovery_generation
            .set(self.youtube_recovery_generation.get().wrapping_add(1));
        self.youtube_recovery_resume_us.set(0);
        self.youtube_recovery_was_playing.set(false);
    }

    pub(crate) fn resume_youtube_after_recovery(&self) {
        let resume_us = self.youtube_recovery_resume_us.replace(0);
        if resume_us <= 0 || self.playback_source.get() != PlaybackSource::YouTube {
            return;
        }

        if self.player.duration_us() <= 0 {
            self.youtube_recovery_resume_us.set(resume_us);
            return;
        }

        if let Err(error) = self.player.seek(resume_us) {
            eprintln!("Could not restore YouTube playback position: {error}");
            return;
        }

        self.last_mpris_position.set(resume_us);
        self.mpris
            .send(crate::playback::mpris::MprisUpdate::Position(resume_us));
    }

    pub(crate) fn apply_youtube_track(
        &self,
        queue: Vec<YouTubeItem>,
        index: usize,
        mut item: YouTubeItem,
        stream: YouTubeStream,
        cover_path: Option<PathBuf>,
    ) {
        let recovering = self.youtube_recovery_in_progress.replace(false);
        if recovering {
            self.youtube_recovery_attempted.set(false);
            self.youtube_recovery_retry_count.set(0);
        }
        let pending = self.queue_v2_pending_entry.replace(None);
        let preserved_id = if recovering {
            let playback_queue = self.playback_queue_v2.borrow();
            matching_youtube_queue_entry(&playback_queue, pending, &item.video_id)
        } else {
            pending.filter(|id| {
                self.playback_queue_v2
                    .borrow()
                    .entry(*id)
                    .is_some_and(|entry| {
                        matches!(
                            &entry.media.source,
                            QueueSource::YouTube {
                                video_id: candidate,
                            } if candidate == item.video_id.as_str()
                        )
                    })
            })
        };

        let selected_queue_id = if let Some(id) = preserved_id {
            let _ = self.playback_queue_v2.borrow_mut().select(id);
            Some(id)
        } else {
            self.sync_youtube_queue_v2(&queue, index);
            self.playback_queue_v2.borrow().current_id()
        };
        if !recovering {
            self.maybe_record_listening();
        }
        let (preserved_lyrics, preserved_cover) = if recovering {
            self.youtube_state
                .borrow()
                .as_ref()
                .filter(|state| state.item.video_id == item.video_id)
                .map(|state| (state.lyrics.clone(), state.cover_path.clone()))
                .unwrap_or_default()
        } else {
            self.youtube_recovery_attempted.set(false);
            self.youtube_recovery_resume_us.set(0);
            (Vec::new(), None)
        };
        let cover_path = cover_path.or(preserved_cover);

        if item.title.is_empty() {
            item.title = stream.title.clone();
        }
        if item.artist.is_empty() {
            item.artist = stream.artist.clone();
        }
        if item.album.is_empty() {
            item.album = stream.album.clone();
        }
        if item.duration_seconds == 0 {
            item.duration_seconds = stream.duration_seconds;
        }

        let autoplay = if recovering {
            self.youtube_recovery_was_playing.replace(false)
        } else {
            self.startup_restore_autoplay.replace(None).unwrap_or(true)
        };
        if !stream.stream_client.is_empty() {
            let label = if stream.stream_client_label.is_empty() {
                stream.stream_client.as_str()
            } else {
                stream.stream_client_label.as_str()
            };
            eprintln!(
                "Nocky YouTube stream resolved with {label}; fallback={}, attempts=[{}], format={}, protocol={}, codec={}",
                stream.fallback_used,
                stream.attempted_clients.join(", "),
                stream.format_id,
                stream.protocol,
                stream.audio_codec,
            );
        }

        if let Err(error) =
            self.player
                .load_with_headers(&stream.stream_url, autoplay, stream.http_headers.clone())
        {
            self.youtube_recovery_in_progress.set(false);
            self.youtube_recovery_resume_us.set(0);
            self.youtube_recovery_was_playing.set(false);
            self.show_error(&error);
            return;
        }

        if item.thumbnail_url.is_empty() {
            item.thumbnail_url = stream.thumbnail_url.clone();
        }
        if let Some(path) = cover_path.as_ref() {
            item.cover_path = path.to_string_lossy().into_owned();
        }

        if let Some(id) = selected_queue_id {
            let media = Self::youtube_queue_media(&item);
            if let Err(error) = self.playback_queue_v2.borrow_mut().update_media(id, media) {
                eprintln!("Could not refresh Queue 2.0 YouTube metadata: {error}");
            }
        }

        {
            let mut library = self.youtube_library.borrow_mut();
            if library.observe_playback(&item) {
                if let Err(error) = save_library_cache(&library) {
                    eprintln!("Could not save recently played YouTube item: {error}");
                }
            }
        }
        self.state.borrow_mut().current = None;
        self.playback_source.set(PlaybackSource::YouTube);
        self.update_footer_source();
        if !recovering {
            self.begin_listening_session(format!("youtube:{}", item.video_id));
        }
        self.youtube_state.replace(Some(YouTubePlaybackState {
            queue,
            current: index,
            item: item.clone(),
            cover_path: cover_path.clone(),
            lyrics: preserved_lyrics.clone(),
        }));

        self.player_view.set_metadata(
            &item.title,
            if item.artist.is_empty() {
                "YouTube Music"
            } else {
                &item.artist
            },
            if item.album.is_empty() {
                "YouTube Music"
            } else {
                &item.album
            },
        );
        self.mini_title.set_text(&item.title);
        self.mini_artist.set_text(if item.artist.is_empty() {
            "YouTube Music"
        } else {
            &item.artist
        });
        self.hero_cover.set_path(cover_path.as_deref());
        self.mini_cover.set_path(cover_path.as_deref());
        self.visual_theme_manager
            .update_artwork(cover_path.as_deref());
        self.player_view.set_favorite(false);
        self.footer_favorite_icon
            .set_icon_name(Some("emblem-favorite-symbolic"));
        self.footer_favorite_icon.set_opacity(0.28);

        if recovering && !preserved_lyrics.is_empty() {
            self.rebuild_youtube_lyrics(&preserved_lyrics);
        } else if self.config.borrow().auto_download_lyrics {
            self.set_lyrics_message("Searching synchronized lyrics for this YouTube track…");
            self.request_youtube_lyrics(&item, false);
        } else {
            self.set_lyrics_message(
                "No synchronized lyrics loaded yet. Use the menu to search for this YouTube track.",
            );
        }

        self.update_play_icons(autoplay);
        if !recovering {
            self.last_mpris_position.set(0);
            self.mpris
                .send(crate::playback::mpris::MprisUpdate::Position(0));
        }
        self.publish_mpris_youtube(&item, &stream, cover_path.as_deref());
        self.mpris
            .send(crate::playback::mpris::MprisUpdate::Playback(if autoplay {
                crate::playback::mpris::MprisPlayback::Playing
            } else {
                crate::playback::mpris::MprisPlayback::Paused
            }));
        self.prefetch_youtube_queue();
    }

    fn prefetch_youtube_queue(&self) {
        let Some(bridge) = self.youtube_bridge.clone() else {
            return;
        };
        let (queue, current) = {
            let state = self.youtube_state.borrow();
            let Some(state) = state.as_ref() else {
                return;
            };
            (state.queue.clone(), state.current)
        };
        thread::spawn(move || bridge.preload_streams(&queue, current, 4));
    }

    fn publish_mpris_youtube(
        &self,
        item: &YouTubeItem,
        stream: &YouTubeStream,
        cover_path: Option<&Path>,
    ) {
        let length_us = item
            .duration_seconds
            .max(stream.duration_seconds)
            .saturating_mul(1_000_000)
            .min(i64::MAX as u64) as i64;
        let art_url = cover_path.map(|path| gio::File::for_path(path).uri().to_string());
        self.mpris
            .send(crate::playback::mpris::MprisUpdate::Metadata(
                crate::playback::mpris::MprisTrack {
                    track_id: mpris_youtube_track_id(&item.video_id),
                    title: item.title.clone(),
                    artist: if item.artist.is_empty() {
                        stream.artist.clone()
                    } else {
                        item.artist.clone()
                    },
                    album: if item.album.is_empty() {
                        stream.album.clone()
                    } else {
                        item.album.clone()
                    },
                    length_us,
                    art_url,
                    url: Some(stream.webpage_url.clone()),
                },
            ));
        self.publish_mpris_capabilities();
    }

    pub(crate) fn request_youtube_lyrics(&self, item: &YouTubeItem, notify: bool) {
        if item.video_id.is_empty() {
            return;
        }
        let lookup = lyrics::provider::LyricsLookup {
            title: item.title.clone(),
            artist: item.artist.clone(),
            album: item.album.clone(),
            duration_seconds: item.duration_seconds,
        };
        let video_id = item.video_id.clone();
        let sender = self.background.sender();
        thread::spawn(move || {
            let result = lyrics::provider::fetch_lyrics(&lookup, notify).map(|document| {
                eprintln!(
                    "YouTube lyrics loaded from {} ({})",
                    document.provider,
                    if document.synchronized {
                        "synchronized"
                    } else {
                        "plain fallback"
                    }
                );
                lyrics::parse_lrc(&document.contents)
            });
            let _ = sender.send(BackgroundMessage::YouTubeLyricsDownloaded {
                video_id,
                notify,
                result,
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::playback::queue::QueueMedia;

    #[test]
    fn bounded_recovery_delays_are_explicit() {
        assert_eq!(youtube_recovery_delay_seconds(1), None);
        assert_eq!(youtube_recovery_delay_seconds(2), Some(1));
        assert_eq!(youtube_recovery_delay_seconds(3), Some(3));
        assert_eq!(youtube_recovery_delay_seconds(4), None);
    }

    #[test]
    fn recovery_prefers_the_exact_entry_id_for_duplicate_videos() {
        let mut queue = PlaybackQueue::new();
        let first = queue.append(QueueMedia::youtube(
            "duplicate-video",
            "First occurrence",
            "Artist",
            "Album",
            180,
            None,
        ));
        let second = queue.append(QueueMedia::youtube(
            "duplicate-video",
            "Second occurrence",
            "Artist",
            "Album",
            180,
            None,
        ));
        queue.select(first).expect("select first occurrence");

        assert_eq!(
            matching_youtube_queue_entry(&queue, Some(second), "duplicate-video"),
            Some(second)
        );
    }

    #[test]
    fn recovery_falls_back_to_the_current_matching_entry() {
        let mut queue = PlaybackQueue::new();
        let current = queue.append(QueueMedia::youtube(
            "current-video",
            "Current",
            "Artist",
            "Album",
            180,
            None,
        ));
        let unrelated = queue.append(QueueMedia::youtube(
            "other-video",
            "Other",
            "Artist",
            "Album",
            180,
            None,
        ));
        queue.select(current).expect("select current occurrence");

        assert_eq!(
            matching_youtube_queue_entry(&queue, Some(unrelated), "current-video"),
            Some(current)
        );
    }
}

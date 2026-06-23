use crate::{
    background::BackgroundMessage,
    lyrics, lyrics_provider, mpris,
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

use super::{
    is_refreshable_stream_error, mpris_youtube_track_id, redact_stream_url, AppController,
    PlaybackSource, YouTubePlaybackState,
};

impl AppController {
    pub(super) fn resolve_youtube_track(
        &self,
        item: YouTubeItem,
        queue: Vec<YouTubeItem>,
        index: usize,
        force: bool,
    ) {
        let Some(bridge) = self.youtube_bridge.clone() else {
            self.show_toast("As dependências do YouTube Music não estão instaladas");
            return;
        };
        if item.video_id.is_empty() {
            return;
        }
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

    pub(super) fn try_recover_youtube_stream(&self, error: &str) -> bool {
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

        self.youtube_recovery_attempted.set(true);
        self.youtube_recovery_in_progress.set(true);
        self.youtube_recovery_resume_us
            .set(self.player.position_us().max(0));
        let _ = self.player.stop();

        eprintln!(
            "Nocky YouTube stream rejected; refreshing signed URL: {}",
            redact_stream_url(error)
        );
        self.resolve_youtube_track(item, queue, index, true);
        true
    }

    pub(super) fn reset_youtube_recovery(&self) {
        self.youtube_recovery_in_progress.set(false);
        self.youtube_recovery_attempted.set(false);
        self.youtube_recovery_resume_us.set(0);
    }

    pub(super) fn resume_youtube_after_recovery(&self) {
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
        self.mpris.send(mpris::MprisUpdate::Position(resume_us));
    }

    pub(super) fn apply_youtube_track(
        &self,
        queue: Vec<YouTubeItem>,
        index: usize,
        mut item: YouTubeItem,
        stream: YouTubeStream,
        cover_path: Option<PathBuf>,
    ) {
        let recovering = self.youtube_recovery_in_progress.replace(false);
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

        if let Err(error) =
            self.player
                .load_with_headers(&stream.stream_url, true, stream.http_headers.clone())
        {
            self.youtube_recovery_in_progress.set(false);
            self.youtube_recovery_resume_us.set(0);
            self.show_error(&error);
            return;
        }

        if item.thumbnail_url.is_empty() {
            item.thumbnail_url = stream.thumbnail_url.clone();
        }
        if let Some(path) = cover_path.as_ref() {
            item.cover_path = path.to_string_lossy().into_owned();
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

        self.update_play_icons(true);
        if !recovering {
            self.last_mpris_position.set(0);
            self.mpris.send(mpris::MprisUpdate::Position(0));
        }
        self.publish_mpris_youtube(&item, &stream, cover_path.as_deref());
        self.mpris
            .send(mpris::MprisUpdate::Playback(mpris::MprisPlayback::Playing));
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

    pub(super) fn youtube_next_track(&self) {
        let state = self.youtube_state.borrow();
        let Some(current) = state.as_ref() else {
            return;
        };
        if current.queue.is_empty() {
            return;
        }
        let next = if self.shuffle_enabled.get() && current.queue.len() > 1 {
            let mut value = self.rng_state.get();
            value ^= value << 13;
            value ^= value >> 7;
            value ^= value << 17;
            self.rng_state.set(value);
            let mut position = value as usize % current.queue.len();
            if position == current.current {
                position = (position + 1) % current.queue.len();
            }
            Some(position)
        } else {
            current
                .current
                .checked_add(1)
                .filter(|position| *position < current.queue.len())
        };
        let Some(next) = next else {
            return;
        };
        let queue = current.queue.clone();
        let item = queue[next].clone();
        drop(state);
        self.resolve_youtube_track(item, queue, next, false);
    }

    pub(super) fn youtube_previous_track(&self) {
        if self.player.position_us() > 5_000_000 {
            self.seek_to(0, true);
            return;
        }
        let state = self.youtube_state.borrow();
        let Some(current) = state.as_ref() else {
            return;
        };
        let Some(previous) = current.current.checked_sub(1) else {
            drop(state);
            self.seek_to(0, true);
            return;
        };
        let queue = current.queue.clone();
        let item = queue[previous].clone();
        drop(state);
        self.resolve_youtube_track(item, queue, previous, false);
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
            .send(mpris::MprisUpdate::Metadata(mpris::MprisTrack {
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
            }));
        self.publish_mpris_capabilities();
    }

    pub(super) fn request_youtube_lyrics(&self, item: &YouTubeItem, notify: bool) {
        if item.video_id.is_empty() {
            return;
        }
        let lookup = lyrics_provider::LyricsLookup {
            title: item.title.clone(),
            artist: item.artist.clone(),
            album: item.album.clone(),
        };
        let video_id = item.video_id.clone();
        let sender = self.background.sender();
        thread::spawn(move || {
            let result = lyrics_provider::fetch_synced_lyrics(&lookup)
                .map(|contents| lyrics::parse_lrc(&contents));
            let _ = sender.send(BackgroundMessage::YouTubeLyricsDownloaded {
                video_id,
                notify,
                result,
            });
        });
    }
}

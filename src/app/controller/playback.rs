//! Playback controller methods for `AppController`.

use super::AppController;
use crate::{
    app::{
        media::{
            format_time, mpris_track_id, mpris_youtube_track_id, playback_error_message,
            redact_stream_url,
        },
        state::PlaybackSource,
    },
    browser::BrowserRoute,
    i18n::Message,
    listening_history::ListeningSource,
    model::Track,
    playback::{
        queue::{queue_end_action, QueueEndAction, QueueEntryId, QueueSource},
        PlaybackEvent,
    },
    youtube::YouTubeItem,
};
use gtk::{gio, prelude::*};
use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

impl AppController {
    pub(crate) fn play_queue_entry(&self, id: QueueEntryId, autoplay: bool) {
        let media = self
            .playback_queue_v2
            .borrow()
            .entry(id)
            .map(|entry| entry.media.clone());
        let Some(media) = media else {
            return;
        };

        match &media.source {
            QueueSource::Local { path } => {
                let index = self
                    .state
                    .borrow()
                    .tracks
                    .iter()
                    .position(|track| &track.path == path);
                if let Some(index) = index {
                    self.select_track(index, autoplay);
                }
            }
            QueueSource::YouTube { video_id } => {
                let existing = {
                    let state = self.youtube_state.borrow();
                    state.as_ref().and_then(|state| {
                        let queue = state.queue.clone();
                        queue
                            .iter()
                            .position(|item| &item.video_id == video_id)
                            .map(|position| (queue[position].clone(), queue, position))
                    })
                };

                let (item, queue, position) = existing.unwrap_or_else(|| {
                    let item = YouTubeItem {
                        result_type: "song".to_string(),
                        title: media.title.clone(),
                        artist: media.artist.clone(),
                        album: media.album.clone(),
                        video_id: video_id.clone(),
                        duration_seconds: media.duration_seconds,
                        cover_path: media
                            .cover_path
                            .as_ref()
                            .map(|path| path.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                        ..YouTubeItem::default()
                    };
                    (item.clone(), vec![item], 0)
                });

                self.queue_v2_pending_entry.set(Some(id));
                self.resolve_youtube_track(item, queue, position, false);
            }
        }
    }

    pub(crate) fn toggle_playback(&self) {
        if self.playback_source.get() == PlaybackSource::YouTube
            && self.youtube_state.borrow().is_some()
        {
            if self.player.is_playing() {
                self.pause_current();
            } else {
                self.play_current();
            }
            return;
        }

        if self.state.borrow().current.is_none() {
            let queued = self.initial_queue_entry_id();
            if let Some(id) = queued {
                self.play_queue_entry(id, true);
                return;
            }

            let sequence = self.playback_sequence();
            if let Some(index) = sequence.first().copied() {
                self.state.borrow_mut().playback_queue = sequence;
                self.select_track(index, true);
            } else if !self.state.borrow().tracks.is_empty() {
                let sequence = (0..self.state.borrow().tracks.len()).collect::<Vec<_>>();
                self.state.borrow_mut().playback_queue = sequence;
                self.select_track(0, true);
            }
            return;
        }

        if self.player.is_playing() {
            self.pause_current();
        } else {
            self.play_current();
        }
    }

    pub(crate) fn next_track(&self) -> bool {
        self.ensure_active_queue_v2();

        let Some(next) = self.next_queue_entry_id() else {
            return false;
        };

        self.play_queue_entry(next, true);
        true
    }

    pub(crate) fn previous_track(&self) {
        if self.player.position_us() > 5_000_000 {
            self.seek_to(0, true);
            return;
        }

        self.ensure_active_queue_v2();
        let previous = self.previous_queue_entry_id();
        let has_current = self.playback_queue_v2.borrow().current_id().is_some();

        if let Some(previous) = previous {
            self.play_queue_entry(previous, true);
        } else if has_current {
            self.seek_to(0, true);
        }
    }

    pub(crate) fn play_current(&self) {
        match self.player.play() {
            Ok(()) => {
                self.update_play_icons(true);
                self.mpris
                    .send(crate::playback::mpris::MprisUpdate::Playback(
                        crate::playback::mpris::MprisPlayback::Playing,
                    ));
            }
            Err(error) => self.show_error(&error),
        }
    }

    pub(crate) fn pause_current(&self) {
        self.maybe_record_listening();

        match self.player.pause() {
            Ok(()) => {
                self.update_play_icons(false);
                self.mpris
                    .send(crate::playback::mpris::MprisUpdate::Playback(
                        crate::playback::mpris::MprisPlayback::Paused,
                    ));
            }
            Err(error) => self.show_error(&error),
        }
    }

    pub(crate) fn handle_playback_events(&self) {
        while let Some(event) = self.player.try_recv() {
            match event {
                PlaybackEvent::EndOfStream => self.handle_end_of_stream(),
                PlaybackEvent::DurationChanged => {
                    self.publish_mpris_capabilities();
                    self.resume_youtube_after_recovery();
                    self.apply_pending_resume_position();
                }
                PlaybackEvent::ClockLost => {
                    if let Err(error) = self.player.recover_clock() {
                        eprintln!("Could not recover GStreamer clock after resume: {error}");
                    }
                }
                PlaybackEvent::Spectrum(values) => self.visualizer.set_values(&values),
                PlaybackEvent::Error(error) => {
                    if self.youtube_recovery_in_progress.get() {
                        eprintln!(
                            "Ignoring follow-up GStreamer error during stream refresh: {}",
                            redact_stream_url(&error)
                        );
                        continue;
                    }
                    if self.try_recover_youtube_stream(&error) {
                        continue;
                    }

                    eprintln!("Nocky playback error: {}", redact_stream_url(&error));
                    self.update_play_icons(false);
                    self.mpris
                        .send(crate::playback::mpris::MprisUpdate::Playback(
                            crate::playback::mpris::MprisPlayback::Stopped,
                        ));
                    self.show_error(playback_error_message(&error));
                }
            }
        }
    }

    pub(crate) fn handle_end_of_stream(&self) {
        self.maybe_record_listening();
        self.ensure_active_queue_v2();

        let repeat_one = self.repeat_button.is_active();
        let next = if repeat_one {
            None
        } else {
            self.next_queue_entry_id()
        };

        match queue_end_action(repeat_one, next) {
            QueueEndAction::RepeatCurrent => {
                self.seek_to(0, true);
                self.play_current();
            }
            QueueEndAction::Play(id) => self.play_queue_entry(id, true),
            QueueEndAction::Stop => {
                let _ = self.player.pause();
                self.update_play_icons(false);
                self.mpris
                    .send(crate::playback::mpris::MprisUpdate::Playback(
                        crate::playback::mpris::MprisPlayback::Stopped,
                    ));
            }
        }
    }

    pub(crate) fn handle_mpris_commands(&self) {
        while let Ok(command) = self.mpris.commands.try_recv() {
            match command {
                crate::playback::mpris::MprisCommand::Ready => {}
                crate::playback::mpris::MprisCommand::Error(error) => {
                    eprintln!("Nocky MPRIS bridge error: {error}");
                }
                crate::playback::mpris::MprisCommand::Raise => self.window.present(),
                crate::playback::mpris::MprisCommand::Quit => {
                    if let Some(application) = self.window.application() {
                        application.quit();
                    }
                }
                crate::playback::mpris::MprisCommand::Play => {
                    if self.playback_source.get() == PlaybackSource::YouTube
                        && self.youtube_state.borrow().is_some()
                    {
                        self.play_current();
                    } else if self.state.borrow().current.is_none() {
                        let queued = self.initial_queue_entry_id();
                        if let Some(id) = queued {
                            self.play_queue_entry(id, true);
                            continue;
                        }

                        let sequence = self.playback_sequence();
                        if let Some(index) = sequence.first().copied() {
                            self.state.borrow_mut().playback_queue = sequence;
                            self.select_track(index, true);
                        } else if !self.state.borrow().tracks.is_empty() {
                            let sequence =
                                (0..self.state.borrow().tracks.len()).collect::<Vec<_>>();
                            self.state.borrow_mut().playback_queue = sequence;
                            self.select_track(0, true);
                        }
                    } else {
                        self.play_current();
                    }
                }
                crate::playback::mpris::MprisCommand::Pause => self.pause_current(),
                crate::playback::mpris::MprisCommand::PlayPause => self.toggle_playback(),
                crate::playback::mpris::MprisCommand::Stop => {
                    self.pause_current();
                    self.seek_to(0, true);
                    self.mpris
                        .send(crate::playback::mpris::MprisUpdate::Playback(
                            crate::playback::mpris::MprisPlayback::Stopped,
                        ));
                }
                crate::playback::mpris::MprisCommand::Next => {
                    self.next_track();
                }
                crate::playback::mpris::MprisCommand::Previous => self.previous_track(),
                crate::playback::mpris::MprisCommand::Seek(offset) => {
                    let position = self.player.position_us().saturating_add(offset);
                    self.seek_to(position, true);
                }
                crate::playback::mpris::MprisCommand::SetPosition { track_id, position } => {
                    if self.current_mpris_track_id().as_deref() == Some(track_id.as_str()) {
                        self.seek_to(position, true);
                    }
                }
                crate::playback::mpris::MprisCommand::SetLoop(enabled) => {
                    if self.repeat_button.is_active() != enabled {
                        self.repeat_button.set_active(enabled);
                    }
                }
                crate::playback::mpris::MprisCommand::SetShuffle(enabled) => {
                    if self.shuffle_button.is_active() != enabled {
                        self.shuffle_button.set_active(enabled);
                    }
                }
                crate::playback::mpris::MprisCommand::SetVolume(value) => {
                    let value = value.clamp(0.0, 1.0);
                    if (self.volume.value() - value).abs() > f64::EPSILON {
                        self.volume.set_value(value);
                    }
                }
            }
        }
    }

    pub(crate) fn seek_to(&self, position: i64, announce: bool) {
        if !self.player.is_seekable() {
            return;
        }

        let duration = self.player.duration_us().max(0);
        let position = if duration > 0 {
            position.clamp(0, duration)
        } else {
            position.max(0)
        };

        if let Err(error) = self.player.seek(position) {
            self.show_error(&error);
            return;
        }
        self.last_mpris_position.set(position);
        if announce {
            self.mpris
                .send(crate::playback::mpris::MprisUpdate::Seeked(position));
        } else {
            self.mpris
                .send(crate::playback::mpris::MprisUpdate::Position(position));
        }
    }

    pub(crate) fn current_mpris_track_id(&self) -> Option<String> {
        if self.playback_source.get() == PlaybackSource::YouTube {
            return self
                .youtube_state
                .borrow()
                .as_ref()
                .map(|state| mpris_youtube_track_id(&state.item.video_id));
        }
        let state = self.state.borrow();
        state
            .current
            .and_then(|index| state.tracks.get(index))
            .map(|track| mpris_track_id(&track.path))
    }

    pub(crate) fn publish_mpris_track(&self, track: &Track) {
        let length_us = track
            .duration_seconds
            .saturating_mul(1_000_000)
            .min(i64::MAX as u64) as i64;
        let art_url = track
            .cover_path
            .as_ref()
            .map(|path| gio::File::for_path(path).uri().to_string());
        let url = Some(track.file.uri().to_string());

        self.mpris
            .send(crate::playback::mpris::MprisUpdate::Metadata(
                crate::playback::mpris::MprisTrack {
                    track_id: mpris_track_id(&track.path),
                    title: track.title.clone(),
                    artist: track.artist.clone(),
                    album: track.album.clone(),
                    length_us,
                    art_url,
                    url,
                },
            ));
        self.publish_mpris_capabilities();
    }

    pub(crate) fn publish_mpris_capabilities(&self) {
        let state = self.state.borrow();
        let has_youtube = self.youtube_state.borrow().is_some();
        let has_tracks = !state.tracks.is_empty() || has_youtube;
        let can_seek = state
            .current
            .and_then(|index| state.tracks.get(index))
            .is_some_and(|track| track.duration_seconds > 0)
            || has_youtube
            || self.player.is_seekable();
        drop(state);

        self.mpris
            .send(crate::playback::mpris::MprisUpdate::Capabilities {
                has_tracks,
                can_seek,
            });
    }

    pub(crate) fn update_play_icons(&self, playing: bool) {
        self.player_view.set_playing(playing);
        let icon = if playing {
            "media-playback-pause-symbolic"
        } else {
            "media-playback-start-symbolic"
        };
        self.play_icon.set_icon_name(Some(icon));
        self.hero_play_icon.set_icon_name(Some(icon));
        self.player_view
            .set_visualizer_active(playing && self.visualizer.widget().is_visible());
        let config = self.config.borrow();
        let animate_m3 = playing && config.visual_theme.is_expressive();
        let language = config.language;
        drop(config);
        self.home_wave_progress.set_playing(animate_m3);
        self.footer_progress.set_playing(animate_m3);

        if matches!(self.browser.route(), BrowserRoute::All) {
            self.browser.update_home_playback_state(playing, language);
        }
    }

    pub(crate) fn begin_listening_session(&self, id: String) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        self.listening_session_id
            .replace(Some(format!("{id}:{nonce}")));
        self.listening_session_last_saved_seconds.set(0);
    }

    pub(crate) fn maybe_record_listening(&self) {
        let listened_seconds = (self.player.position_us().max(0) / 1_000_000) as u64;
        let duration_seconds = (self.player.duration_us().max(0) / 1_000_000) as u64;
        let completed = duration_seconds > 0
            && listened_seconds.saturating_mul(100) >= duration_seconds.saturating_mul(90);

        if listened_seconds < 30 && !completed {
            return;
        }

        let previous = self.listening_session_last_saved_seconds.get();
        let first_checkpoint = previous == 0;
        let checkpoint_due = first_checkpoint || listened_seconds >= previous.saturating_add(15);
        if !completed && !checkpoint_due {
            return;
        }

        let Some(session_id) = self.listening_session_id.borrow().clone() else {
            return;
        };

        let updated = match self.playback_source.get() {
            PlaybackSource::Local => {
                let state = self.state.borrow();
                let Some(index) = state.current else {
                    return;
                };
                let Some(track) = state.tracks.get(index) else {
                    return;
                };
                self.listening_history
                    .borrow_mut()
                    .record_playback_progress(
                        session_id,
                        track.path.to_string_lossy().into_owned(),
                        track.title.clone(),
                        track.artist.clone(),
                        track.album.clone(),
                        ListeningSource::Local,
                        listened_seconds,
                        listened_seconds,
                        duration_seconds.max(track.duration_seconds),
                        self.listening_history_context.borrow().clone(),
                        completed,
                    )
            }
            PlaybackSource::YouTube => {
                let state = self.youtube_state.borrow();
                let Some(state) = state.as_ref() else {
                    return;
                };
                self.listening_history
                    .borrow_mut()
                    .record_playback_progress(
                        session_id,
                        state.item.video_id.clone(),
                        state.item.title.clone(),
                        state.item.artist.clone(),
                        state.item.album.clone(),
                        ListeningSource::YouTube,
                        listened_seconds,
                        listened_seconds,
                        duration_seconds,
                        self.listening_history_context.borrow().clone(),
                        completed,
                    )
            }
            PlaybackSource::None => false,
        };

        if updated {
            self.listening_session_last_saved_seconds
                .set(listened_seconds);

            if first_checkpoint || completed {
                self.refresh_browser();
            }
        }
    }

    pub(crate) fn refresh_progress(&self) {
        self.apply_pending_resume_position();

        self.maybe_record_listening();
        let timestamp = self.player.position_us().max(0);
        let duration = self.player.duration_us().max(0);
        let fraction = if duration > 0 {
            timestamp as f64 / duration as f64
        } else {
            0.0
        };

        self.updating_progress.set(true);
        self.progress.set_value(fraction.clamp(0.0, 1.0));
        self.footer_traditional_progress
            .set_value(fraction.clamp(0.0, 1.0));
        self.home_wave_progress.set_fraction(fraction);
        self.footer_progress.set_fraction(fraction);
        self.updating_progress.set(false);
        let elapsed = format_time(timestamp);
        let duration_text = format_time(duration);
        self.elapsed.set_text(&elapsed);
        self.duration.set_text(&duration_text);
        self.footer_elapsed.set_text(&elapsed);
        self.footer_duration.set_text(&duration_text);
        self.highlight_lyric(timestamp);

        let previous = self.last_mpris_position.get();
        if previous < 0 || (timestamp - previous).abs() >= 500_000 {
            self.last_mpris_position.set(timestamp);
            self.mpris
                .send(crate::playback::mpris::MprisUpdate::Position(timestamp));
        }
    }

    pub(crate) fn reset_now_playing(&self, message: &str) {
        let _ = self.player.stop();
        self.playback_source.set(PlaybackSource::None);
        self.youtube_state.replace(None);
        self.playback_queue_v2.borrow_mut().clear();
        self.queue_v2_pending_entry.set(None);
        self.reset_youtube_recovery();
        self.player_view.set_metadata(
            self.tr(Message::IntegratedMusic),
            self.tr(Message::NoTrackSelected),
            message,
        );
        self.set_footer_metadata(self.tr(Message::NothingPlaying), "Nocky");
        self.update_footer_source();
        self.lyrics.show_state(
            "As letras aparecerão aqui",
            Some("Reproduza uma música com letras sincronizadas para acompanhar cada verso."),
            "As letras aparecerão aqui",
            Some("Reproduza uma música com letras sincronizadas para ver o contexto."),
        );
        self.hero_cover.set_path(None);
        self.visual_theme_manager.update_artwork(None);
        self.mini_cover.set_path(None);
        self.elapsed.set_text("0:00");
        self.duration.set_text("0:00");
        self.footer_elapsed.set_text("0:00");
        self.footer_duration.set_text("0:00");
        self.progress.set_value(0.0);
        self.footer_traditional_progress.set_value(0.0);
        self.home_wave_progress.set_fraction(0.0);
        self.footer_progress.set_fraction(0.0);
        self.update_play_icons(false);
        self.last_mpris_position.set(0);
        self.mpris
            .send(crate::playback::mpris::MprisUpdate::ClearMetadata);
        self.mpris
            .send(crate::playback::mpris::MprisUpdate::Playback(
                crate::playback::mpris::MprisPlayback::Stopped,
            ));
        self.mpris
            .send(crate::playback::mpris::MprisUpdate::Position(0));
        self.publish_mpris_capabilities();
    }

    pub(crate) fn current_track_path(&self) -> Option<PathBuf> {
        let state = self.state.borrow();
        state
            .current
            .and_then(|index| state.tracks.get(index))
            .map(|track| track.path.clone())
    }

    pub(crate) fn select_track(&self, index: usize, autoplay: bool) {
        self.maybe_record_listening();

        let track = {
            let state = self.state.borrow();
            let Some(track) = state.tracks.get(index).cloned() else {
                return;
            };
            track
        };

        let uri = track.file.uri().to_string();
        if let Err(error) = self.player.load(&uri, autoplay) {
            self.show_error(&error);
            return;
        }

        self.playback_source.set(PlaybackSource::Local);
        self.queue_v2_pending_entry.set(None);
        self.update_footer_source();
        if let Some(index) = self.state.borrow().current {
            if let Some(track) = self.state.borrow().tracks.get(index) {
                self.begin_listening_session(format!("local:{}", track.path.display()));
            }
        }
        self.youtube_state.replace(None);
        self.reset_youtube_recovery();
        self.state.borrow_mut().current = Some(index);
        self.ensure_local_queue_v2(index);
        self.player_view
            .set_metadata(&track.title, &track.artist, &track.album);
        self.set_footer_metadata(&track.title, &track.artist);
        self.hero_cover.set_path(track.cover_path.as_deref());
        self.mini_cover.set_path(track.cover_path.as_deref());
        self.visual_theme_manager
            .update_artwork(track.cover_path.as_deref());
        self.rebuild_lyrics(&track);
        self.update_favorite_icon(&track.path);
        self.publish_mpris_track(&track);
        self.last_mpris_position.set(0);
        self.update_play_icons(autoplay);
        self.mpris
            .send(crate::playback::mpris::MprisUpdate::Position(0));
        self.mpris
            .send(crate::playback::mpris::MprisUpdate::Playback(if autoplay {
                crate::playback::mpris::MprisPlayback::Playing
            } else {
                crate::playback::mpris::MprisPlayback::Paused
            }));

        self.browser.select_track(index);

        if track.lyrics.is_empty() && self.config.borrow().auto_download_lyrics {
            self.request_lyrics(index, false, false);
        }
    }
}

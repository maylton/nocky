//! Persistence and history helpers for `AppController`.

use super::AppController;
use crate::{
    listening_history,
    playback::{queue::QueueSource, session::PlaybackSession},
    youtube::YouTubeItem,
};
use gtk::prelude::*;
use std::time::{SystemTime, UNIX_EPOCH};

impl AppController {
    pub(crate) fn save_config(&self) {
        if let Err(error) = self.config.borrow().save() {
            eprintln!("Could not save Nocky settings: {error}");
        }
    }

    pub(crate) fn playback_session_snapshot(&self) -> Option<PlaybackSession> {
        let queue = self.playback_queue_v2.borrow();
        let current = queue.current()?;
        let context = self.listening_history_context.borrow();

        let mut session = PlaybackSession::new(&current.media.source);
        session.position_us = self.player.position_us().max(0);
        session.was_playing = self.player.is_playing();
        session.shuffle_enabled = self.shuffle_enabled.get();
        session.repeat_enabled = self.repeat_button.is_active();
        session.shuffle_state = session
            .shuffle_enabled
            .then(|| self.shuffle_navigation.borrow().snapshot());
        session.shuffle_rng_state = self.rng_state.get();
        session.context_kind = context.kind.clone();
        session.context_id = context.id.clone();
        session.context_title = context.title.clone();
        session.saved_at_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or_default();
        Some(session)
    }

    pub(crate) fn persist_playback_session_if_changed(&self) {
        let Some(session) = self.playback_session_snapshot() else {
            return;
        };

        let seconds = (session.position_us.max(0) as u64) / 1_000_000;
        let shuffle = session.shuffle_enabled;
        let repeat = session.repeat_enabled;
        if seconds == self.playback_session_last_position_seconds.get()
            && shuffle == self.playback_session_last_shuffle.get()
            && repeat == self.playback_session_last_repeat.get()
        {
            return;
        }

        self.playback_session_last_position_seconds.set(seconds);
        self.playback_session_last_shuffle.set(shuffle);
        self.playback_session_last_repeat.set(repeat);
        let source = self.active_queue_source.get();
        if let Err(error) = crate::playback::session::save_for(source, &session) {
            eprintln!("Could not save playback session for {source:?}: {error}");
        }
    }

    pub(crate) fn persist_playback_session_now(&self) {
        let source = self.active_queue_source.get();
        if let Some(session) = self.playback_session_snapshot() {
            if let Err(error) = crate::playback::session::save_for(source, &session) {
                eprintln!("Could not save playback session for {source:?}: {error}");
            }
        } else if let Err(error) = crate::playback::session::clear_for(source) {
            eprintln!("Could not clear playback session for {source:?}: {error}");
        }
    }

    pub(crate) fn try_restore_playback_session(&self) {
        let Some(session) = self.restored_playback_session.borrow().clone() else {
            return;
        };

        let attempts = self.playback_session_restore_attempts.get();
        if attempts >= 30 {
            self.restored_playback_session.replace(None);
            return;
        }
        self.playback_session_restore_attempts
            .set(attempts.saturating_add(1));

        let current_media = self
            .playback_queue_v2
            .borrow()
            .current()
            .map(|entry| entry.media.clone());

        let Some(current_media) = current_media else {
            self.restored_playback_session.replace(None);
            return;
        };

        if current_media.source.stable_key() != session.source_key {
            self.restored_playback_session.replace(None);
            return;
        }

        self.shuffle_enabled.set(session.shuffle_enabled);
        self.shuffle_button.set_active(session.shuffle_enabled);
        self.footer_shuffle_button
            .set_active(session.shuffle_enabled);
        self.repeat_button.set_active(session.repeat_enabled);
        self.footer_repeat_button.set_active(session.repeat_enabled);

        if session.shuffle_enabled {
            if session.shuffle_rng_state != 0 {
                self.rng_state.set(session.shuffle_rng_state);
            }
            let restored_shuffle = session.shuffle_state.as_ref().is_some_and(|snapshot| {
                let queue = self.playback_queue_v2.borrow();
                self.shuffle_navigation.borrow_mut().restore(
                    queue.entries(),
                    queue.current_id(),
                    snapshot,
                )
            });
            if !restored_shuffle {
                self.reset_shuffle_navigation(true);
            }
        } else {
            self.shuffle_navigation.borrow_mut().clear();
        }

        self.listening_history_context
            .replace(listening_history::PlaybackHistoryContext {
                kind: session.context_kind.clone(),
                id: session.context_id.clone(),
                title: session.context_title.clone(),
            });
        self.pending_resume_position_us
            .set(Some(session.position_us.max(0)));
        let autoplay = self.config.borrow().resume_playback_on_startup && session.was_playing;

        match &current_media.source {
            QueueSource::Local { path } => {
                let index = self
                    .state
                    .borrow()
                    .tracks
                    .iter()
                    .position(|track| &track.path == path);
                let Some(index) = index else {
                    return;
                };
                self.select_track(index, autoplay);
            }
            QueueSource::YouTube { video_id } => {
                let queue = self
                    .playback_queue_v2
                    .borrow()
                    .entries()
                    .iter()
                    .filter_map(|entry| match &entry.media.source {
                        QueueSource::YouTube { video_id } => Some(YouTubeItem {
                            result_type: "song".to_string(),
                            title: entry.media.title.clone(),
                            artist: entry.media.artist.clone(),
                            album: entry.media.album.clone(),
                            duration_seconds: entry.media.duration_seconds,
                            video_id: video_id.clone(),
                            cover_path: entry
                                .media
                                .cover_path
                                .as_ref()
                                .map(|path| path.to_string_lossy().to_string())
                                .unwrap_or_default(),
                            ..YouTubeItem::default()
                        }),
                        QueueSource::Local { .. } => None,
                    })
                    .collect::<Vec<_>>();
                let Some(index) = queue.iter().position(|item| item.video_id == *video_id) else {
                    self.restored_playback_session.replace(None);
                    return;
                };
                self.startup_restore_autoplay.set(Some(autoplay));
                self.resolve_youtube_track(queue[index].clone(), queue, index, false);
            }
        }

        self.playback_session_last_position_seconds
            .set((session.position_us.max(0) as u64) / 1_000_000);
        self.playback_session_last_shuffle
            .set(session.shuffle_enabled);
        self.playback_session_last_repeat
            .set(session.repeat_enabled);
        self.restored_playback_session.replace(None);
        self.playback_session_restore_attempts.set(0);
        self.show_toast("Reprodução anterior restaurada");
    }

    pub(crate) fn apply_pending_resume_position(&self) {
        let Some(position) = self.pending_resume_position_us.get() else {
            return;
        };

        if !self.player.is_seekable() || self.player.duration_us() <= 0 {
            return;
        }

        match self.player.seek(position.max(0)) {
            Ok(()) => {
                self.pending_resume_position_us.set(None);
                self.last_mpris_position.set(position.max(0));
                self.mpris
                    .send(crate::playback::mpris::MprisUpdate::Position(
                        position.max(0),
                    ));
            }
            Err(error) => {
                eprintln!("Could not restore playback position: {error}");
            }
        }
    }
}

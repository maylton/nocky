//! Playback controller methods for `AppController`.

use super::*;

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
}

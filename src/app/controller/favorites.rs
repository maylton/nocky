//! Favorite state helpers for `AppController`.

use super::AppController;
use crate::{app::state::PlaybackSource, i18n::Message};
use gtk::prelude::*;

impl AppController {
    pub(crate) fn toggle_favorite(&self) {
        if self.playback_source.get() == PlaybackSource::YouTube {
            self.toggle_youtube_favorite();
            return;
        }

        if self.playback_source.get() == PlaybackSource::YouTube {
            self.show_toast("Gerencie curtidas do YouTube Music pela conta conectada");
            return;
        }

        let path = {
            let state = self.state.borrow();
            let Some(track) = state.current.and_then(|index| state.tracks.get(index)) else {
                self.show_toast("Selecione uma faixa primeiro");
                return;
            };
            track.path.clone()
        };

        let liked = self.config.borrow_mut().toggle_liked(&path);
        self.save_config();
        self.update_favorite_icon(&path);
        self.refresh_browser();
        self.show_toast(if liked {
            self.tr(Message::AddedLiked)
        } else {
            self.tr(Message::RemovedLiked)
        });
    }

    pub(crate) fn update_favorite_icon(&self, path: &std::path::Path) {
        let liked = self.config.borrow().is_liked(path);
        self.favorite_icon
            .set_icon_name(Some("emblem-favorite-symbolic"));
        self.favorite_icon
            .set_opacity(if liked { 0.98 } else { 0.28 });
        self.footer_favorite_icon
            .set_icon_name(Some("emblem-favorite-symbolic"));
        self.footer_favorite_icon
            .set_opacity(if liked { 0.98 } else { 0.28 });
    }
}

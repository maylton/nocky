//! Safe native action for adding the current YouTube track to the open playlist.
//!
//! The action is absent until read-only metadata confirms editability. Mutations
//! run once on a worker thread, and native state changes only after a fresh server
//! read confirms the track in the playlist.

use super::AppController;
use crate::{
    app::state::PlaybackSource,
    browser::BrowserRoute,
    config::AppLanguage,
    youtube::{cache_items_for_browser, YouTubeItem},
};
use gtk::prelude::*;
use std::{
    collections::HashSet,
    sync::{
        mpsc::{self, Receiver, Sender},
        Mutex, OnceLock,
    },
    thread,
};

type PlaylistAddRequest = (String, String);
type PlaylistAddResult = (String, String, Result<Vec<YouTubeItem>, String>);

fn request_channel() -> &'static (Sender<PlaylistAddRequest>, Mutex<Receiver<PlaylistAddRequest>>) {
    static CHANNEL: OnceLock<(Sender<PlaylistAddRequest>, Mutex<Receiver<PlaylistAddRequest>>)> =
        OnceLock::new();
    CHANNEL.get_or_init(|| {
        let (sender, receiver) = mpsc::channel();
        (sender, Mutex::new(receiver))
    })
}

fn result_channel() -> &'static (Sender<PlaylistAddResult>, Mutex<Receiver<PlaylistAddResult>>) {
    static CHANNEL: OnceLock<(Sender<PlaylistAddResult>, Mutex<Receiver<PlaylistAddResult>>)> =
        OnceLock::new();
    CHANNEL.get_or_init(|| {
        let (sender, receiver) = mpsc::channel();
        (sender, Mutex::new(receiver))
    })
}

fn pending_additions() -> &'static Mutex<HashSet<String>> {
    static PENDING: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(HashSet::new()))
}

fn normalized_playlist_id(value: &str) -> &str {
    value.trim().trim_start_matches("VL")
}

fn same_playlist_id(left: &str, right: &str) -> bool {
    let left = normalized_playlist_id(left);
    !left.is_empty() && left == normalized_playlist_id(right)
}

fn valid_video_id(value: &str) -> bool {
    value.len() == 11
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

fn pending_key(playlist_id: &str, video_id: &str) -> String {
    format!(
        "{}:{}",
        normalized_playlist_id(playlist_id),
        video_id.trim()
    )
}

fn mark_pending(playlist_id: &str, video_id: &str) -> bool {
    pending_additions()
        .lock()
        .map(|mut pending| pending.insert(pending_key(playlist_id, video_id)))
        .unwrap_or(false)
}

fn clear_pending(playlist_id: &str, video_id: &str) {
    if let Ok(mut pending) = pending_additions().lock() {
        pending.remove(&pending_key(playlist_id, video_id));
    }
}

fn is_pending(playlist_id: &str, video_id: &str) -> bool {
    pending_additions()
        .lock()
        .map(|pending| pending.contains(&pending_key(playlist_id, video_id)))
        .unwrap_or(false)
}

fn find_css(widget: &gtk::Widget, class_name: &str) -> Option<gtk::Widget> {
    if widget.has_css_class(class_name) {
        return Some(widget.clone());
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        if let Some(found) = find_css(&current, class_name) {
            return Some(found);
        }
        child = current.next_sibling();
    }
    None
}

fn remove_from_parent(widget: &gtk::Widget) {
    if let Some(parent) = widget
        .parent()
        .and_then(|parent| parent.downcast::<gtk::Box>().ok())
    {
        parent.remove(widget);
    }
}

fn labels(language: AppLanguage, pending: bool) -> (&'static str, &'static str) {
    match (language, pending) {
        (AppLanguage::Portuguese, false) => (
            "Adicionar faixa atual",
            "Adicionar a faixa atual a esta playlist",
        ),
        (AppLanguage::Portuguese, true) => ("Adicionando…", "Adição em andamento"),
        (AppLanguage::English, false) => (
            "Add current track",
            "Add the current track to this playlist",
        ),
        (AppLanguage::English, true) => ("Adding…", "Addition in progress"),
        (AppLanguage::Spanish, false) => (
            "Añadir pista actual",
            "Añadir la pista actual a esta playlist",
        ),
        (AppLanguage::Spanish, true) => ("Añadiendo…", "Adición en curso"),
    }
}

fn success_message(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::Portuguese => "Faixa adicionada e confirmada pelo YouTube Music.",
        AppLanguage::English => "Track added and confirmed by YouTube Music.",
        AppLanguage::Spanish => "Pista añadida y confirmada por YouTube Music.",
    }
}

fn error_message(language: AppLanguage, error: &str) -> &'static str {
    let error = error.to_lowercase();
    let auth = error.contains("session")
        || error.contains("authentication")
        || error.contains("unauthorized")
        || error.contains("401");
    let permission = error.contains("ownership")
        || error.contains("editability")
        || error.contains("permission")
        || error.contains("forbidden")
        || error.contains("403");
    let duplicate = error.contains("already") || error.contains("duplicate");
    let ambiguous = error.contains("network")
        || error.contains("offline")
        || error.contains("timeout")
        || error.contains("connect")
        || error.contains("fresh playlist")
        || error.contains("confirm the added track");

    match (language, auth, permission, duplicate, ambiguous) {
        (AppLanguage::Portuguese, true, _, _, _) => {
            "A sessão do YouTube Music expirou. Reconecte sua conta para continuar."
        }
        (AppLanguage::Portuguese, _, true, _, _) => {
            "A playlist não está mais disponível para edição nesta conta."
        }
        (AppLanguage::Portuguese, _, _, true, _) => "A faixa já está nesta playlist.",
        (AppLanguage::Portuguese, _, _, _, true) => {
            "Não foi possível confirmar a alteração. A playlist no Nocky não foi modificada."
        }
        (AppLanguage::Portuguese, _, _, _, _) => {
            "Não foi possível adicionar a faixa à playlist."
        }
        (AppLanguage::English, true, _, _, _) => {
            "Your YouTube Music session expired. Reconnect your account to continue."
        }
        (AppLanguage::English, _, true, _, _) => {
            "This playlist is no longer editable by the connected account."
        }
        (AppLanguage::English, _, _, true, _) => "The track is already in this playlist.",
        (AppLanguage::English, _, _, _, true) => {
            "The server change could not be confirmed. Nocky's playlist was left unchanged."
        }
        (AppLanguage::English, _, _, _, _) => "The track could not be added to the playlist.",
        (AppLanguage::Spanish, true, _, _, _) => {
            "La sesión de YouTube Music caducó. Vuelve a conectar tu cuenta."
        }
        (AppLanguage::Spanish, _, true, _, _) => {
            "La cuenta conectada ya no puede editar esta playlist."
        }
        (AppLanguage::Spanish, _, _, true, _) => "La pista ya está en esta playlist.",
        (AppLanguage::Spanish, _, _, _, true) => {
            "No se pudo confirmar el cambio. La playlist de Nocky no cambió."
        }
        (AppLanguage::Spanish, _, _, _, _) => "No se pudo añadir la pista a la playlist.",
    }
}

impl AppController {
    fn current_youtube_video_id(&self) -> Option<String> {
        if self.playback_source.get() != PlaybackSource::YouTube {
            return None;
        }
        self.youtube_state
            .borrow()
            .as_ref()
            .map(|state| state.item.video_id.trim().to_string())
            .filter(|video_id| valid_video_id(video_id))
    }

    pub(crate) fn update_youtube_playlist_add_action(&self) {
        let root = self.browser.root().clone().upcast::<gtk::Widget>();
        let existing = find_css(&root, "youtube-playlist-add-current");
        let BrowserRoute::YouTubePlaylist { browse_id, .. } = self.browser.route() else {
            if let Some(existing) = existing {
                remove_from_parent(&existing);
            }
            return;
        };

        if self.cached_youtube_playlist_editability(&browse_id) != Some(true) {
            if let Some(existing) = existing {
                remove_from_parent(&existing);
            }
            return;
        }
        let Some(video_id) = self.current_youtube_video_id() else {
            if let Some(existing) = existing {
                remove_from_parent(&existing);
            }
            return;
        };

        let pending = is_pending(&browse_id, &video_id);
        let identity = format!(
            "youtube-playlist-add-current:{}:{}",
            normalized_playlist_id(&browse_id),
            video_id
        );
        let language = self.config.borrow().language;
        let (label, tooltip) = labels(language, pending);

        if let Some(existing) = existing {
            if existing.widget_name().as_str() == identity {
                if let Ok(button) = existing.downcast::<gtk::Button>() {
                    button.set_label(label);
                    button.set_tooltip_text(Some(tooltip));
                    button.set_sensitive(!pending);
                }
                return;
            }
            remove_from_parent(&existing);
        }

        let Some(header) = find_css(&root, "collection-page-header")
            .and_then(|widget| widget.downcast::<gtk::Box>().ok())
        else {
            return;
        };
        let button = gtk::Button::with_label(label);
        button.set_widget_name(&identity);
        button.set_tooltip_text(Some(tooltip));
        button.set_valign(gtk::Align::Center);
        button.set_sensitive(!pending);
        button.add_css_class("pill");
        button.add_css_class("suggested-action");
        button.add_css_class("youtube-playlist-add-current");

        let sender = request_channel().0.clone();
        button.connect_clicked(move |button| {
            if !mark_pending(&browse_id, &video_id) {
                return;
            }
            let (label, tooltip) = labels(language, true);
            button.set_label(label);
            button.set_tooltip_text(Some(tooltip));
            button.set_sensitive(false);
            let _ = sender.send((browse_id.clone(), video_id.clone()));
        });
        header.append(&button);
    }

    pub(crate) fn handle_youtube_playlist_add_requests(&self) {
        let Ok(receiver) = request_channel().1.lock() else {
            return;
        };
        while let Ok((browse_id, video_id)) = receiver.try_recv() {
            let route_matches = matches!(
                self.browser.route(),
                BrowserRoute::YouTubePlaylist {
                    browse_id: current,
                    ..
                } if same_playlist_id(&current, &browse_id)
            );
            let valid = route_matches
                && self.cached_youtube_playlist_editability(&browse_id) == Some(true)
                && self.current_youtube_video_id().as_deref() == Some(video_id.as_str());
            if !valid {
                clear_pending(&browse_id, &video_id);
                self.update_youtube_playlist_add_action();
                continue;
            }

            let playlist = self
                .youtube_library
                .borrow()
                .playlists
                .iter()
                .find(|playlist| same_playlist_id(&playlist.browse_id, &browse_id))
                .cloned();
            let (Some(playlist), Some(bridge)) = (playlist, self.youtube_bridge.clone()) else {
                clear_pending(&browse_id, &video_id);
                self.update_youtube_playlist_add_action();
                continue;
            };

            let sender = result_channel().0.clone();
            thread::spawn(move || {
                let result = bridge
                    .add_playlist_item(&browse_id, &video_id)
                    .and_then(|_| bridge.playlist(&playlist))
                    .and_then(|mut items| {
                        if !items.iter().any(|item| item.video_id == video_id) {
                            return Err(
                                "The fresh playlist read could not confirm the added track"
                                    .to_string(),
                            );
                        }
                        cache_items_for_browser(&mut items);
                        Ok(items)
                    });
                let _ = sender.send((browse_id, video_id, result));
            });
        }
    }

    pub(crate) fn handle_youtube_playlist_add_updates(&self) {
        let Ok(receiver) = result_channel().1.lock() else {
            return;
        };
        while let Ok((browse_id, video_id, result)) = receiver.try_recv() {
            clear_pending(&browse_id, &video_id);
            let language = self.config.borrow().language;
            match result {
                Ok(items) => {
                    self.youtube_library
                        .borrow_mut()
                        .playlist_tracks
                        .insert(browse_id.clone(), items);
                    self.invalidate_youtube_playlist_metadata(&browse_id);
                    let playlist = self
                        .youtube_library
                        .borrow()
                        .playlists
                        .iter()
                        .find(|playlist| same_playlist_id(&playlist.browse_id, &browse_id))
                        .cloned();
                    if let Some(playlist) = playlist {
                        self.request_youtube_playlist_metadata(playlist);
                    }
                    if matches!(
                        self.browser.route(),
                        BrowserRoute::YouTubePlaylist {
                            browse_id: current,
                            ..
                        } if same_playlist_id(&current, &browse_id)
                    ) {
                        self.refresh_browser();
                    }
                    self.show_toast(success_message(language));
                }
                Err(error) => {
                    eprintln!(
                        "Could not add YouTube track {video_id} to playlist {browse_id}: {error}"
                    );
                    self.show_toast(error_message(language, &error));
                }
            }
            self.update_youtube_playlist_add_action();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{pending_key, same_playlist_id, valid_video_id};

    #[test]
    fn validates_current_video_identity() {
        assert!(valid_video_id("abcdefghijk"));
        assert!(valid_video_id("abc_def-123"));
        assert!(!valid_video_id("too-short"));
        assert!(!valid_video_id("invalid/id!"));
    }

    #[test]
    fn normalizes_playlist_identity_for_pending_and_routes() {
        assert!(same_playlist_id("VLPL-owned", "PL-owned"));
        assert!(!same_playlist_id("PL-owned", "PL-other"));
        assert_eq!(
            pending_key("VLPL-owned", "abcdefghijk"),
            "PL-owned:abcdefghijk"
        );
    }
}

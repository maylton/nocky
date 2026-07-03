#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path.cwd()
BROWSER = ROOT / "src/browser.rs"
CSS = ROOT / "assets/themes/material-expressive/080-home-browser.css"
THEME_CSS = ROOT / "src/theme_css.rs"
ROADMAP = ROOT / "ROADMAP.md"
AUDIT = ROOT / "docs/CARD_ACTIONS_LOADING_AUDIT.md"


class PatchError(RuntimeError):
    pass


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count == 0 and new in text:
        print(f"[already applied] {label}")
        return text
    if count != 1:
        raise PatchError(f"{label}: expected one match, found {count}")
    print(f"[changed] {label}")
    return text.replace(old, new, 1)


def patch_refresh_calls(text: str) -> str:
    text = replace_once(
        text,
        """            BrowserRoute::Albums => {\n                self.rebuild_albums(tracks, youtube, query);\n                self.root.set_visible_child_name(\"albums\");\n            }\n""",
        """            BrowserRoute::Albums => {\n                self.rebuild_albums(tracks, config, youtube, query, context.playback);\n                self.root.set_visible_child_name(\"albums\");\n            }\n""",
        "Albums refresh context",
    )
    return replace_once(
        text,
        """            BrowserRoute::Playlists => {\n                self.rebuild_playlists(config, youtube, query);\n                self.root.set_visible_child_name(\"playlists\");\n            }\n""",
        """            BrowserRoute::Playlists => {\n                self.rebuild_playlists(config, youtube, query, context.playback);\n                self.root.set_visible_child_name(\"playlists\");\n            }\n""",
        "Playlists refresh context",
    )


def patch_album_grid(text: str) -> str:
    text = replace_once(
        text,
        """    fn rebuild_albums(&self, tracks: &[Track], youtube: &YouTubeLibraryCache, query: &str) {\n""",
        """    fn rebuild_albums(\n        &self,\n        tracks: &[Track],\n        config: &AppConfig,\n        youtube: &YouTubeLibraryCache,\n        query: &str,\n        playback: &BrowserPlaybackState,\n    ) {\n""",
        "Albums rebuild signature",
    )

    text = replace_once(
        text,
        """            append_collection_grid_card(\n                &self.albums_grid,\n                position,\n                collection_button(\n                    collection_card(\n                        cover,\n                        &album,\n                        &artists,\n                        &format!(\"Local • {} faixas\", album_tracks.len()),\n                        false,\n                    ),\n                    BrowserRoute::Album(album),\n                    &self.event_tx,\n                ),\n            );\n""",
        """            let actions = local_album_action_spec(&album, playback, config);\n            append_collection_grid_card(\n                &self.albums_grid,\n                position,\n                collection_grid_card_with_actions(\n                    collection_card(\n                        cover,\n                        &album,\n                        &artists,\n                        &format!(\"Local • {} faixas\", album_tracks.len()),\n                        false,\n                    ),\n                    actions,\n                    &self.event_tx,\n                    config.language,\n                ),\n            );\n""",
        "Local album grid actions",
    )

    return replace_once(
        text,
        """            append_collection_grid_card(\n                &self.albums_grid,\n                position,\n                collection_event_button(\n                    collection_card(\n                        album_entry.cached_cover(),\n                        &album_entry.title,\n                        &album_entry.subtitle,\n                        &album_entry.detail,\n                        true,\n                    ),\n                    BrowserEvent::OpenYouTubeCollection(album_entry.source.clone()),\n                    &self.event_tx,\n                ),\n            );\n""",
        """            let actions = youtube_album_action_spec(\n                &album_entry.source,\n                &album_entry.title,\n                playback,\n                config,\n            );\n            append_collection_grid_card(\n                &self.albums_grid,\n                position,\n                collection_grid_card_with_actions(\n                    collection_card(\n                        album_entry.cached_cover(),\n                        &album_entry.title,\n                        &album_entry.subtitle,\n                        &album_entry.detail,\n                        true,\n                    ),\n                    actions,\n                    &self.event_tx,\n                    config.language,\n                ),\n            );\n""",
        "YouTube album grid actions",
    )


def patch_playlist_page(text: str) -> str:
    text = replace_once(
        text,
        """    fn rebuild_playlists(&self, config: &AppConfig, youtube: &YouTubeLibraryCache, query: &str) {\n""",
        """    fn rebuild_playlists(\n        &self,\n        config: &AppConfig,\n        youtube: &YouTubeLibraryCache,\n        query: &str,\n        playback: &BrowserPlaybackState,\n    ) {\n""",
        "Playlists rebuild signature",
    )

    text = replace_once(
        text,
        """            self.playlists_list.append(&playlist_row(\n                None,\n                &playlist.name,\n                \"Playlist local\",\n                &format!(\"{} faixas\", playlist.tracks.len()),\n                false,\n            ));\n""",
        """            let actions = local_playlist_action_spec(&playlist.name, playback, config);\n            self.playlists_list.append(&playlist_row_with_actions(\n                None,\n                &playlist.name,\n                \"Playlist local\",\n                &format!(\"{} faixas\", playlist.tracks.len()),\n                false,\n                actions,\n                &self.event_tx,\n                config.language,\n            ));\n""",
        "Local playlist row actions",
    )

    text = replace_once(
        text,
        """            self.playlists_list\n                .append(&youtube_mix_row(mix, track_count, preferred_cover));\n""",
        """            let actions = youtube_playlist_action_spec(mix, playback, config);\n            self.playlists_list.append(&youtube_mix_row_with_actions(\n                mix,\n                track_count,\n                preferred_cover,\n                actions,\n                &self.event_tx,\n                config.language,\n            ));\n""",
        "YouTube mix row actions",
    )

    return replace_once(
        text,
        """            self.playlists_list.append(&playlist_row(\n                playlist.cached_cover(),\n                &playlist.title,\n                youtube_playlist_subtitle(playlist),\n                &detail,\n                true,\n            ));\n""",
        """            let actions = youtube_playlist_action_spec(playlist, playback, config);\n            self.playlists_list.append(&playlist_row_with_actions(\n                playlist.cached_cover(),\n                &playlist.title,\n                youtube_playlist_subtitle(playlist),\n                &detail,\n                true,\n                actions,\n                &self.event_tx,\n                config.language,\n            ));\n""",
        "YouTube playlist row actions",
    )


def patch_grid_insertion(text: str) -> str:
    return replace_once(
        text,
        """fn append_collection_grid_card(grid: &gtk::FlowBox, _position: i32, button: gtk::Button) {\n    if grid.has_css_class(\"skip-card-entry-animation\") {\n        button.set_opacity(1.0);\n        button.set_margin_top(0);\n        grid.insert(&button, -1);\n        return;\n    }\n\n    button.set_opacity(0.0);\n    button.set_margin_top(14);\n    button.add_css_class(\"collection-card-entering\");\n    grid.insert(&button, -1);\n\n    let button_weak = button.downgrade();\n    let started_at = Rc::new(Cell::new(None::<i64>));\n    button.add_tick_callback(move |_, frame_clock| {\n        let Some(button) = button_weak.upgrade() else {\n            return glib::ControlFlow::Break;\n        };\n\n        let now = frame_clock.frame_time();\n        let start = started_at.get().unwrap_or_else(|| {\n            started_at.set(Some(now));\n            now\n        });\n        let progress = ((now - start) as f64 / 420_000.0).clamp(0.0, 1.0);\n\n        // Damped spring entrance: fast arrival, subtle overshoot and settle.\n        let damping = (-6.5 * progress).exp();\n        let oscillation = (progress * std::f64::consts::TAU * 1.65).cos();\n        let spring = 1.0 - damping * oscillation;\n\n        let opacity = (progress / 0.42).clamp(0.0, 1.0);\n        let displacement = (1.0 - spring) * 18.0;\n\n        button.set_opacity(opacity);\n        button.set_margin_top(displacement.round().clamp(-4.0, 18.0) as i32);\n\n        if progress >= 1.0 {\n            button.set_opacity(1.0);\n            button.set_margin_top(0);\n            button.remove_css_class(\"collection-card-entering\");\n            glib::ControlFlow::Break\n        } else {\n            glib::ControlFlow::Continue\n        }\n    });\n}\n""",
        """fn append_collection_grid_card<W: IsA<gtk::Widget>>(\n    grid: &gtk::FlowBox,\n    _position: i32,\n    widget: W,\n) {\n    let widget = widget.upcast::<gtk::Widget>();\n    if grid.has_css_class(\"skip-card-entry-animation\") {\n        widget.set_opacity(1.0);\n        widget.set_margin_top(0);\n        grid.insert(&widget, -1);\n        return;\n    }\n\n    widget.set_opacity(0.0);\n    widget.set_margin_top(14);\n    widget.add_css_class(\"collection-card-entering\");\n    grid.insert(&widget, -1);\n\n    let widget_weak = widget.downgrade();\n    let started_at = Rc::new(Cell::new(None::<i64>));\n    widget.add_tick_callback(move |_, frame_clock| {\n        let Some(widget) = widget_weak.upgrade() else {\n            return glib::ControlFlow::Break;\n        };\n\n        let now = frame_clock.frame_time();\n        let start = started_at.get().unwrap_or_else(|| {\n            started_at.set(Some(now));\n            now\n        });\n        let progress = ((now - start) as f64 / 420_000.0).clamp(0.0, 1.0);\n\n        // Damped spring entrance: fast arrival, subtle overshoot and settle.\n        let damping = (-6.5 * progress).exp();\n        let oscillation = (progress * std::f64::consts::TAU * 1.65).cos();\n        let spring = 1.0 - damping * oscillation;\n\n        let opacity = (progress / 0.42).clamp(0.0, 1.0);\n        let displacement = (1.0 - spring) * 18.0;\n\n        widget.set_opacity(opacity);\n        widget.set_margin_top(displacement.round().clamp(-4.0, 18.0) as i32);\n\n        if progress >= 1.0 {\n            widget.set_opacity(1.0);\n            widget.set_margin_top(0);\n            widget.remove_css_class(\"collection-card-entering\");\n            glib::ControlFlow::Break\n        } else {\n            glib::ControlFlow::Continue\n        }\n    });\n}\n""",
        "Generic collection-grid insertion",
    )


ACTION_HELPERS = r'''#[derive(Clone)]
struct CollectionActionSpec {
    play_event: BrowserEvent,
    play_next_event: BrowserEvent,
    append_event: BrowserEvent,
    open_event: BrowserEvent,
    favorite_identity: String,
    favorite_selected: bool,
    offline_collection: Option<(String, BrowserEvent)>,
    is_active: bool,
    is_loading: bool,
    inline_loading_on_click: bool,
    playing: bool,
    widget_key: String,
}

fn local_album_action_spec(
    title: &str,
    playback: &BrowserPlaybackState,
    config: &AppConfig,
) -> CollectionActionSpec {
    let id = title.trim().to_lowercase();
    let favorite_identity = format!("local-album:{id}");
    CollectionActionSpec {
        play_event: BrowserEvent::PlayLocalAlbum(title.to_string()),
        play_next_event: BrowserEvent::QueueLocalCollection {
            kind: "album".to_string(),
            title: title.to_string(),
            play_next: true,
        },
        append_event: BrowserEvent::QueueLocalCollection {
            kind: "album".to_string(),
            title: title.to_string(),
            play_next: false,
        },
        open_event: BrowserEvent::Navigate(BrowserRoute::Album(title.to_string())),
        favorite_selected: config.is_collection_favorite(&favorite_identity),
        favorite_identity,
        offline_collection: None,
        is_active: playback.matches_collection("album", &id, title),
        is_loading: playback.collection_is_loading("album", &id, title),
        inline_loading_on_click: false,
        playing: playback.playing,
        widget_key: home_playback_key("album", &id, title)
            .unwrap_or_else(|| format!("album:{id}")),
    }
}

fn youtube_album_action_spec(
    item: &YouTubeItem,
    display_title: &str,
    playback: &BrowserPlaybackState,
    config: &AppConfig,
) -> CollectionActionSpec {
    let id = youtube_collection_cache_key(item);
    let favorite_identity = format!("youtube-album:{}", display_title.to_lowercase());
    let is_active = playback.matches_collection("album", &id, display_title);
    CollectionActionSpec {
        play_event: BrowserEvent::PlayYouTubeAlbum(item.clone()),
        play_next_event: BrowserEvent::QueueYouTubeCollection {
            item: item.clone(),
            playlist: false,
            play_next: true,
        },
        append_event: BrowserEvent::QueueYouTubeCollection {
            item: item.clone(),
            playlist: false,
            play_next: false,
        },
        open_event: BrowserEvent::OpenYouTubeCollection(item.clone()),
        favorite_selected: config.is_collection_favorite(&favorite_identity),
        favorite_identity,
        offline_collection: Some((
            format!("album:{id}"),
            BrowserEvent::DownloadYouTubeCollection {
                item: item.clone(),
                playlist: false,
            },
        )),
        is_active,
        is_loading: playback.collection_is_loading("album", &id, display_title),
        inline_loading_on_click: !is_active,
        playing: playback.playing,
        widget_key: home_playback_key("album", &id, display_title)
            .unwrap_or_else(|| format!("album:{id}")),
    }
}

fn local_playlist_action_spec(
    title: &str,
    playback: &BrowserPlaybackState,
    config: &AppConfig,
) -> CollectionActionSpec {
    let id = title.trim().to_lowercase();
    let favorite_identity = format!("local-playlist:{id}");
    CollectionActionSpec {
        play_event: BrowserEvent::PlayLocalPlaylist(title.to_string()),
        play_next_event: BrowserEvent::QueueLocalCollection {
            kind: "playlist".to_string(),
            title: title.to_string(),
            play_next: true,
        },
        append_event: BrowserEvent::QueueLocalCollection {
            kind: "playlist".to_string(),
            title: title.to_string(),
            play_next: false,
        },
        open_event: BrowserEvent::Navigate(BrowserRoute::Playlist(title.to_string())),
        favorite_selected: config.is_collection_favorite(&favorite_identity),
        favorite_identity,
        offline_collection: None,
        is_active: playback.matches_collection("playlist", &id, title),
        is_loading: playback.collection_is_loading("playlist", &id, title),
        inline_loading_on_click: false,
        playing: playback.playing,
        widget_key: home_playback_key("playlist", &id, title)
            .unwrap_or_else(|| format!("playlist:{id}")),
    }
}

fn youtube_playlist_action_spec(
    item: &YouTubeItem,
    playback: &BrowserPlaybackState,
    config: &AppConfig,
) -> CollectionActionSpec {
    let id = if item.browse_id.trim().is_empty() {
        item.title.to_lowercase()
    } else {
        item.browse_id.clone()
    };
    let favorite_identity = format!("youtube-playlist:{}", item.title.to_lowercase());
    let is_active = playback.matches_collection("playlist", &id, &item.title);
    CollectionActionSpec {
        play_event: BrowserEvent::PlayYouTubePlaylist(item.clone()),
        play_next_event: BrowserEvent::QueueYouTubeCollection {
            item: item.clone(),
            playlist: true,
            play_next: true,
        },
        append_event: BrowserEvent::QueueYouTubeCollection {
            item: item.clone(),
            playlist: true,
            play_next: false,
        },
        open_event: BrowserEvent::OpenYouTubePlaylist(item.clone()),
        favorite_selected: config.is_collection_favorite(&favorite_identity),
        favorite_identity,
        offline_collection: Some((
            format!("playlist:{}", item.browse_id),
            BrowserEvent::DownloadYouTubeCollection {
                item: item.clone(),
                playlist: true,
            },
        )),
        is_active,
        is_loading: playback.collection_is_loading("playlist", &id, &item.title),
        inline_loading_on_click: !is_active,
        playing: playback.playing,
        widget_key: home_playback_key("playlist", &id, &item.title)
            .unwrap_or_else(|| format!("playlist:{id}")),
    }
}

fn collection_loading_label(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::Portuguese => "Carregando coleção…",
        AppLanguage::English => "Loading collection…",
        AppLanguage::Spanish => "Cargando colección…",
    }
}

fn collection_play_tooltip(
    language: AppLanguage,
    is_active: bool,
    playing: bool,
) -> &'static str {
    match (language, is_active, playing) {
        (AppLanguage::Portuguese, true, true) => "Pausar coleção",
        (AppLanguage::Portuguese, true, false) => "Continuar coleção",
        (AppLanguage::Portuguese, false, _) => "Reproduzir coleção",
        (AppLanguage::English, true, true) => "Pause collection",
        (AppLanguage::English, true, false) => "Resume collection",
        (AppLanguage::English, false, _) => "Play collection",
        (AppLanguage::Spanish, true, true) => "Pausar colección",
        (AppLanguage::Spanish, true, false) => "Continuar colección",
        (AppLanguage::Spanish, false, _) => "Reproducir colección",
    }
}

fn collection_primary_action_button(
    spec: &CollectionActionSpec,
    event_tx: &Sender<BrowserEvent>,
    language: AppLanguage,
) -> gtk::Button {
    let control = gtk::Button::new();
    control.add_css_class("circular");
    control.add_css_class("collection-card-context-action");
    control.add_css_class("material-card-primary-action");
    control.set_widget_name(&format!("collection-play-control:{}", spec.widget_key));

    if spec.is_loading {
        let label = collection_loading_label(language);
        let loading = MaterialLoadingIndicator::compact();
        loading
            .widget()
            .update_property(&[gtk::accessible::Property::Label(label)]);
        control.set_child(Some(loading.widget()));
        control.set_sensitive(false);
        control.add_css_class("loading");
        control.set_tooltip_text(Some(label));
        control.update_property(&[gtk::accessible::Property::Label(label)]);
        return control;
    }

    let tooltip = collection_play_tooltip(language, spec.is_active, spec.playing);
    control.set_icon_name(if spec.is_active && spec.playing {
        "media-playback-pause-symbolic"
    } else {
        "media-playback-start-symbolic"
    });
    control.set_tooltip_text(Some(tooltip));
    control.update_property(&[gtk::accessible::Property::Label(tooltip)]);
    if spec.is_active {
        control.add_css_class("active");
    }

    let sender = event_tx.clone();
    let play_event = spec.play_event.clone();
    let inline_loading_on_click = spec.inline_loading_on_click;
    control.connect_clicked(move |button| {
        let active = button.has_css_class("active");
        if inline_loading_on_click && !active {
            let label = collection_loading_label(language);
            let loading = MaterialLoadingIndicator::compact();
            loading
                .widget()
                .update_property(&[gtk::accessible::Property::Label(label)]);
            button.set_child(Some(loading.widget()));
            button.set_sensitive(false);
            button.add_css_class("loading");
            button.set_tooltip_text(Some(label));
            button.update_property(&[gtk::accessible::Property::Label(label)]);
        }

        let event = if active {
            BrowserEvent::TogglePlayback
        } else {
            play_event.clone()
        };
        let _ = sender.send(event);
    });
    control
}

fn collection_menu_action_button(
    label: &str,
    icon_name: &str,
    selected: bool,
    event: BrowserEvent,
    popover: &gtk::Popover,
    event_tx: &Sender<BrowserEvent>,
) -> gtk::Button {
    let icon = gtk::Image::from_icon_name(icon_name);
    icon.set_pixel_size(if icon_name == "go-next-symbolic" { 20 } else { 18 });
    icon.set_size_request(20, 20);
    icon.set_halign(gtk::Align::Center);
    icon.set_valign(gtk::Align::Center);
    icon.add_css_class("collection-card-overflow-action-icon");

    let text = gtk::Label::new(Some(label));
    text.set_xalign(0.0);
    text.set_hexpand(true);
    text.add_css_class("collection-card-overflow-action-label");

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    content.set_hexpand(true);
    content.set_halign(gtk::Align::Fill);
    content.append(&icon);
    content.append(&text);

    let button = gtk::Button::new();
    button.set_child(Some(&content));
    button.set_halign(gtk::Align::Fill);
    button.set_hexpand(true);
    button.add_css_class("flat");
    button.add_css_class("collection-card-overflow-action");
    button.add_css_class("material-card-menu-action");
    if selected {
        button.add_css_class("material-card-menu-action-selected");
    }

    let sender = event_tx.clone();
    let popover = popover.clone();
    button.connect_clicked(move |_| {
        popover.popdown();
        let _ = sender.send(event.clone());
    });
    button
}

fn collection_overflow_menu(
    spec: &CollectionActionSpec,
    event_tx: &Sender<BrowserEvent>,
    language: AppLanguage,
) -> gtk::MenuButton {
    let more_options_label = match language {
        AppLanguage::Portuguese => "Mais opções",
        AppLanguage::English => "More options",
        AppLanguage::Spanish => "Más opciones",
    };
    let menu = gtk::MenuButton::builder()
        .icon_name("view-more-symbolic")
        .tooltip_text(more_options_label)
        .build();
    menu.update_property(&[gtk::accessible::Property::Label(more_options_label)]);
    menu.add_css_class("circular");
    menu.add_css_class("collection-card-overflow-button");
    menu.add_css_class("material-card-overflow-trigger");
    menu.set_sensitive(!spec.is_loading);
    menu.set_widget_name(&format!("collection-overflow:{}", spec.widget_key));

    let popover = gtk::Popover::new();
    popover.set_autohide(true);
    popover.add_css_class("collection-card-overflow-popover");

    let actions = gtk::Box::new(gtk::Orientation::Vertical, 4);
    actions.set_margin_top(8);
    actions.set_margin_bottom(8);
    actions.set_margin_start(8);
    actions.set_margin_end(8);

    let labels = match language {
        AppLanguage::Portuguese => (
            "Reproduzir em seguida",
            "Adicionar ao fim da fila",
            "Abrir coleção",
            if spec.favorite_selected {
                "Remover dos favoritos"
            } else {
                "Adicionar aos favoritos"
            },
        ),
        AppLanguage::English => (
            "Play next",
            "Add to queue",
            "Open collection",
            if spec.favorite_selected {
                "Remove from favorites"
            } else {
                "Add to favorites"
            },
        ),
        AppLanguage::Spanish => (
            "Reproducir a continuación",
            "Añadir al final de la cola",
            "Abrir colección",
            if spec.favorite_selected {
                "Quitar de favoritos"
            } else {
                "Añadir a favoritos"
            },
        ),
    };

    for button in [
        collection_menu_action_button(
            labels.0,
            "media-skip-forward-symbolic",
            false,
            spec.play_next_event.clone(),
            &popover,
            event_tx,
        ),
        collection_menu_action_button(
            labels.1,
            "list-add-symbolic",
            false,
            spec.append_event.clone(),
            &popover,
            event_tx,
        ),
        collection_menu_action_button(
            labels.2,
            "go-next-symbolic",
            false,
            spec.open_event.clone(),
            &popover,
            event_tx,
        ),
        collection_menu_action_button(
            labels.3,
            if spec.favorite_selected {
                "emblem-favorite-symbolic"
            } else {
                "non-starred-symbolic"
            },
            spec.favorite_selected,
            BrowserEvent::ToggleCollectionFavorite(spec.favorite_identity.clone()),
            &popover,
            event_tx,
        ),
    ] {
        actions.append(&button);
    }

    if let Some((offline_collection_id, offline_event)) = spec.offline_collection.clone() {
        let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
        separator.add_css_class("collection-card-overflow-separator");
        actions.append(&separator);

        let label = match language {
            AppLanguage::Portuguese => "Disponibilizar offline",
            AppLanguage::English => "Make available offline",
            AppLanguage::Spanish => "Hacer disponible sin conexión",
        };
        let button = collection_menu_action_button(
            label,
            "folder-download-symbolic",
            false,
            offline_event.clone(),
            &popover,
            event_tx,
        );
        button.set_widget_name(&format!(
            "youtube-home-offline-menu:{offline_collection_id}"
        ));
        button.add_css_class("collection-card-offline-action");

        let sender = event_tx.clone();
        let popover_for_click = popover.clone();
        button.connect_clicked(move |button| {
            button.set_sensitive(false);
            button.add_css_class("material-card-menu-action-loading");
            set_home_offline_menu_content(
                button,
                "emblem-synchronizing-symbolic",
                match language {
                    AppLanguage::Portuguese => "Preparando download…",
                    AppLanguage::English => "Preparing download…",
                    AppLanguage::Spanish => "Preparando descarga…",
                },
            );
            popover_for_click.popdown();
            let _ = sender.send(offline_event.clone());
        });
        actions.append(&button);
    }

    popover.set_child(Some(&actions));
    menu.set_popover(Some(&popover));
    menu
}

fn apply_collection_action_state(card: &gtk::Box, spec: &CollectionActionSpec) {
    card.set_widget_name(&format!("collection-play-card:{}", spec.widget_key));
    if spec.is_active {
        card.add_css_class("collection-card-playing");
    }
    if spec.is_loading {
        card.add_css_class("collection-card-loading");
        card.add_css_class("collection-card-skeleton");
    }
    if let Some((offline_collection_id, _)) = &spec.offline_collection {
        let target_name = format!("youtube-home-offline:{offline_collection_id}");
        tag_home_collection_cache_indicator(
            &card.clone().upcast::<gtk::Widget>(),
            &target_name,
        );
    }
}

fn collection_grid_card_with_actions(
    card: gtk::Box,
    spec: CollectionActionSpec,
    event_tx: &Sender<BrowserEvent>,
    language: AppLanguage,
) -> gtk::Widget {
    apply_collection_action_state(&card, &spec);
    let main_button = collection_event_button(card, spec.open_event.clone(), event_tx);

    let overlay = gtk::Overlay::new();
    overlay.set_size_request(
        COLLECTION_CARD_MAX_WIDTH + 20,
        COLLECTION_CARD_MIN_HEIGHT + 12,
    );
    overlay.set_hexpand(true);
    overlay.set_halign(gtk::Align::Fill);
    overlay.set_valign(gtk::Align::Start);
    overlay.set_child(Some(&main_button));
    overlay.add_css_class("collection-grid-action-overlay");

    let play = collection_primary_action_button(&spec, event_tx, language);
    play.set_halign(gtk::Align::End);
    play.set_valign(gtk::Align::Start);
    play.set_margin_top(10);
    play.set_margin_end(10);
    play.add_css_class("collection-grid-primary-action");
    overlay.add_overlay(&play);

    let menu = collection_overflow_menu(&spec, event_tx, language);
    menu.set_halign(gtk::Align::Start);
    menu.set_valign(gtk::Align::Start);
    menu.set_margin_top(10);
    menu.set_margin_start(10);
    menu.add_css_class("collection-grid-overflow-action");
    overlay.add_overlay(&menu);
    overlay.upcast()
}

fn decorate_playlist_row_with_actions(
    row: &gtk::ListBoxRow,
    spec: &CollectionActionSpec,
    event_tx: &Sender<BrowserEvent>,
    language: AppLanguage,
) {
    let Some(content) = row.child().and_then(|child| child.downcast::<gtk::Box>().ok()) else {
        return;
    };

    let trailing_arrow = content.last_child();
    if let Some(arrow) = trailing_arrow.as_ref() {
        content.remove(arrow);
    }

    let menu = collection_overflow_menu(spec, event_tx, language);
    menu.set_halign(gtk::Align::End);
    menu.set_valign(gtk::Align::Center);
    menu.add_css_class("playlist-row-overflow-action");
    content.append(&menu);

    let play = collection_primary_action_button(spec, event_tx, language);
    play.set_halign(gtk::Align::End);
    play.set_valign(gtk::Align::Center);
    play.add_css_class("playlist-row-primary-action");
    content.append(&play);

    if let Some(arrow) = trailing_arrow {
        content.append(&arrow);
    }

    row.add_css_class("playlist-card-row-with-actions");
    row.set_widget_name(&format!("collection-play-row:{}", spec.widget_key));
    if spec.is_active {
        row.add_css_class("collection-card-playing");
    }
    if spec.is_loading {
        row.add_css_class("collection-card-loading");
    }
    if let Some((offline_collection_id, _)) = &spec.offline_collection {
        let target_name = format!("youtube-home-offline:{offline_collection_id}");
        tag_home_collection_cache_indicator(
            &content.clone().upcast::<gtk::Widget>(),
            &target_name,
        );
    }
}

fn playlist_row_with_actions(
    cover_path: Option<&Path>,
    name: &str,
    subtitle: &str,
    detail: &str,
    online: bool,
    spec: CollectionActionSpec,
    event_tx: &Sender<BrowserEvent>,
    language: AppLanguage,
) -> gtk::ListBoxRow {
    let row = playlist_row(cover_path, name, subtitle, detail, online);
    decorate_playlist_row_with_actions(&row, &spec, event_tx, language);
    row
}

fn youtube_mix_row_with_actions(
    item: &YouTubeItem,
    track_count: Option<usize>,
    cover_path: Option<&Path>,
    spec: CollectionActionSpec,
    event_tx: &Sender<BrowserEvent>,
    language: AppLanguage,
) -> gtk::ListBoxRow {
    let row = youtube_mix_row(item, track_count, cover_path);
    decorate_playlist_row_with_actions(&row, &spec, event_tx, language);
    row
}

'''


def patch_action_helpers(text: str) -> str:
    return replace_once(
        text,
        "fn collection_button(\n",
        ACTION_HELPERS + "fn collection_button(\n",
        "Reusable collection action component",
    )


def patch_css(text: str) -> str:
    marker = "/* Collection-page card actions */"
    if marker in text:
        print("[already applied] collection-page action CSS")
        return text
    addition = r'''

/* Collection-page card actions */
window.theme-material-expressive .collection-grid-action-overlay {
  min-width: 240px;
  min-height: 222px;
}

window.theme-material-expressive
  .collection-grid-action-overlay
  .collection-card-context-action,
window.theme-material-expressive
  .collection-grid-action-overlay
  .collection-card-overflow-button {
  opacity: 1;
}

window.theme-material-expressive
  .playlist-card-row-with-actions
  .collection-card-context-action {
  min-width: 38px;
  min-height: 38px;
}

window.theme-material-expressive
  .playlist-card-row-with-actions
  .collection-card-overflow-button {
  min-width: 34px;
  min-height: 34px;
}

window.theme-material-expressive
  .playlist-card-row-with-actions.collection-card-playing {
  background-color: alpha(@m3_primary_container, 0.34);
  box-shadow: inset 3px 0 0 @m3_primary;
}

window.theme-material-expressive
  .playlist-card-row-with-actions.collection-card-loading {
  opacity: 0.78;
}
'''
    print("[changed] collection-page action CSS")
    return text.rstrip() + addition.rstrip() + "\n"


def patch_theme_tests(text: str) -> str:
    return replace_once(
        text,
        """            \".youtube-home-loading-placeholders\",\n            \".home-card-loading-placeholder\",\n""",
        """            \".youtube-home-loading-placeholders\",\n            \".home-card-loading-placeholder\",\n            \".collection-grid-action-overlay\",\n            \".playlist-card-row-with-actions\",\n""",
        "Collection-page action CSS contract",
    )


def patch_roadmap(text: str) -> str:
    text = replace_once(
        text,
        """- Reuse Home play/pause and overflow action overlays on album and playlist collection grids.\n- Keep artist cards navigation-first until deterministic artist queue resolution is available.\n""",
        """- ✅ Reusable play/pause and overflow actions are applied to album grids and playlist rows.\n- Keep artist cards navigation-first until deterministic artist queue resolution is available.\n""",
        "Roadmap collection-page action status",
    )
    return text


def patch_audit(text: str) -> str:
    text = replace_once(
        text,
        """Current behavior:\n\n- album cards and artist-album cards are navigation-only;\n- compact artist cards are navigation-only;\n- collection-grid buttons do not yet reuse the Home action overlay;\n- loading currently falls back to one placeholder card or one status row.\n\nRecommended action follow-up:\n\n1. extract a reusable collection-card action overlay from the Home card builder;\n2. add play/pause and overflow to album and playlist grid cards;\n3. keep artist cards navigation-only until artist queue resolution is explicit;\n4. preserve a single full-card navigation target and independent accessible\n   names for floating controls.\n""",
        """Current behavior:\n\n- local and YouTube album grids use the reusable collection action component;\n- playlist rows use the same play/pause and overflow semantics without replacing row navigation;\n- artist-album and compact artist cards remain navigation-only;\n- loading disables overflow and uses the inline Material Loading Indicator in the primary action.\n\nRecommended action follow-up:\n\n1. validate album and playlist action focus order with keyboard and screen readers;\n2. keep artist cards navigation-only until artist queue resolution is explicit;\n3. decide separately whether artist-album cards should gain actions after async artist-page refreshes can preserve playback context;\n4. keep search rows compact and avoid copying the complete card action cluster there.\n""",
        "Audit collection-page actions",
    )
    text = replace_once(
        text,
        """4. extract reusable album/playlist action overlays for collection grids;\n5. audit keyboard and screen-reader behavior before extending search-row actions.\n""",
        """4. ✅ extract reusable album/playlist action overlays for collection pages;\n5. audit keyboard and screen-reader behavior before extending search-row actions.\n""",
        "Audit implementation order",
    )
    return text


def main() -> int:
    paths = [BROWSER, CSS, THEME_CSS, ROADMAP, AUDIT]
    missing = [path for path in paths if not path.is_file()]
    if missing:
        print("Run this script from the Nocky repository root.", file=sys.stderr)
        for path in missing:
            print(f"missing: {path}", file=sys.stderr)
        return 1

    original = {path: path.read_text(encoding="utf-8") for path in paths}
    if "should_show_youtube_home_loading_placeholders" not in original[BROWSER]:
        print(
            "ERROR: apply-card-actions-loading-checkpoint.py must be applied first.",
            file=sys.stderr,
        )
        return 1

    updated = dict(original)
    try:
        updated[BROWSER] = patch_refresh_calls(updated[BROWSER])
        updated[BROWSER] = patch_album_grid(updated[BROWSER])
        updated[BROWSER] = patch_playlist_page(updated[BROWSER])
        updated[BROWSER] = patch_grid_insertion(updated[BROWSER])
        updated[BROWSER] = patch_action_helpers(updated[BROWSER])
        updated[CSS] = patch_css(updated[CSS])
        updated[THEME_CSS] = patch_theme_tests(updated[THEME_CSS])
        updated[ROADMAP] = patch_roadmap(updated[ROADMAP])
        updated[AUDIT] = patch_audit(updated[AUDIT])
    except PatchError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        print("No files were written.", file=sys.stderr)
        return 1

    changed = []
    for path in paths:
        if updated[path] != original[path]:
            path.write_text(updated[path], encoding="utf-8")
            changed.append(path.relative_to(ROOT))

    print("Collection-page actions patch applied successfully.")
    for path in changed:
        print(f"  {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

//! Visual update helpers for `AppController`.

use super::AppController;
use crate::{
    app::state::PlaybackSource,
    config::{AppLanguage, BlurMode, VisualTheme},
    i18n::{self, Message},
    ui::{
        footer::{
            self, footer_full_artwork_size_for_card_height, footer_mode_plan, AdaptiveFooterTier,
        },
        widgets::{run_compact_volume_spring, CompactVolumeSpring},
    },
    APP_ID,
};
use adw::prelude::*;
use gtk::glib;
use std::{cell::Cell, rc::Rc, time::Duration};

impl AppController {
    pub(crate) fn tr(&self, message: Message) -> &'static str {
        i18n::text(self.config.borrow().language, message)
    }

    pub(crate) fn set_footer_metadata(&self, title: &str, artist: &str) {
        if !adw::is_animations_enabled(&self.mini_title) {
            self.mini_title.set_text(title);
            self.mini_artist.set_text(artist);
            self.mini_title.set_opacity(1.0);
            self.mini_artist.set_opacity(1.0);
            return;
        }

        if self.mini_title.text().as_str() == title && self.mini_artist.text().as_str() == artist {
            return;
        }

        let token = self.footer_metadata_transition.next();
        self.footer_metadata_transition.fade(
            token,
            &self.mini_title,
            self.mini_title.opacity(),
            0.0,
            0,
            86,
        );
        self.footer_metadata_transition.fade(
            token,
            &self.mini_artist,
            self.mini_artist.opacity(),
            0.0,
            14,
            86,
        );

        let title_label = self.mini_title.clone();
        let artist_label = self.mini_artist.clone();
        let transition = self.footer_metadata_transition.clone();
        let title = title.to_owned();
        let artist = artist.to_owned();

        self.footer_metadata_transition.after(token, 104, move || {
            title_label.set_text(&title);
            artist_label.set_text(&artist);
            transition.fade(token, &title_label, 0.0, 1.0, 0, 180);
            transition.fade(token, &artist_label, 0.0, 1.0, 44, 180);
        });
    }

    pub(crate) fn update_footer_source(&self) {
        self.footer_source.remove_css_class("youtube-source-badge");
        match self.playback_source.get() {
            PlaybackSource::Local => self.footer_source.set_text(self.tr(Message::SourceLocal)),
            PlaybackSource::YouTube => {
                self.footer_source.set_text(self.tr(Message::SourceYoutube));
                self.footer_source.add_css_class("youtube-source-badge");
            }
            PlaybackSource::None => self.footer_source.set_text(self.tr(Message::SourceNone)),
        }

        if self.playback_source.get() == PlaybackSource::YouTube {
            if let Some(item) = self.current_youtube_item() {
                let liked = self.youtube_item_is_liked(&item.video_id);
                self.set_youtube_favorite_visual_state(liked);
            }
        }
    }

    pub(crate) fn apply_volume_icon(&self) {
        let value = self.volume.value();
        let icon = if value <= 0.001 {
            "audio-volume-muted-symbolic"
        } else if value < 0.34 {
            "audio-volume-low-symbolic"
        } else if value < 0.67 {
            "audio-volume-medium-symbolic"
        } else {
            "audio-volume-high-symbolic"
        };
        self.mute_icon.set_icon_name(Some(icon));

        let compact = self.player_bar.has_css_class("footer-mode-compact");
        let tooltip = if compact {
            if self.compact_volume_expanded.get() {
                self.tr(Message::HideVolumeControl)
            } else {
                self.tr(Message::AdjustVolume)
            }
        } else if value <= 0.001 {
            self.tr(Message::Unmute)
        } else {
            self.tr(Message::Mute)
        };
        self.mute_button.set_tooltip_text(Some(tooltip));
    }

    pub(crate) fn apply_compact_volume_expansion(&self) {
        let compact = self.player_bar.has_css_class("footer-mode-compact");
        let expanded = compact && self.compact_volume_expanded.get();
        let material_expressive = self.config.borrow().visual_theme.is_expressive();

        self.footer_right_controls
            .remove_css_class("volume-expanded");
        self.footer_right_controls
            .remove_css_class("volume-spring-active");
        self.mute_button.remove_css_class("volume-panel-open");

        if expanded && material_expressive {
            self.footer_right_controls.add_css_class("volume-expanded");
            self.mute_button.add_css_class("volume-panel-open");
        }

        let token = self.compact_volume_spring_generation.get().wrapping_add(1);
        self.compact_volume_spring_generation.set(token);

        if !compact {
            self.volume_revealer.set_visible(true);
            self.volume_revealer.set_reveal_child(true);
            self.footer_right_controls.set_size_request(190, 52);
            self.apply_volume_icon();
            return;
        }

        let current_width = self
            .footer_right_controls
            .width()
            .max(self.footer_right_controls.width_request())
            .max(100);
        let target_width = if expanded { 234 } else { 100 };

        if expanded {
            self.volume_revealer.set_visible(true);
            self.volume_revealer.set_reveal_child(false);

            let revealer = self.volume_revealer.clone();
            let generation = self.compact_volume_spring_generation.clone();
            glib::timeout_add_local_once(Duration::from_millis(16), move || {
                if generation.get() == token {
                    revealer.set_reveal_child(true);
                }
            });
        } else {
            self.volume_revealer.set_reveal_child(false);

            let revealer = self.volume_revealer.clone();
            let generation = self.compact_volume_spring_generation.clone();
            glib::timeout_add_local_once(Duration::from_millis(380), move || {
                if generation.get() == token {
                    revealer.set_visible(false);
                }
            });
        }

        let animate_material_spring =
            material_expressive && adw::is_animations_enabled(&self.footer_right_controls);

        if animate_material_spring {
            run_compact_volume_spring(CompactVolumeSpring {
                group: self.footer_right_controls.clone(),
                generation: self.compact_volume_spring_generation.clone(),
                token,
                from_width: current_width,
                target_width,
                expanding: expanded,
                delay_ms: if expanded { 18 } else { 0 },
            });
        } else {
            // Noctalia keeps the native GtkRevealer slide without the custom
            // Material overshoot/rebound geometry.
            self.footer_right_controls
                .set_size_request(target_width, 52);
            self.footer_right_controls.queue_allocate();
        }

        self.apply_volume_icon();
    }

    pub(crate) fn apply_expressive_transport_effects(&self) {
        let enabled = {
            let config = self.config.borrow();
            config.expressive_transport_effects && config.visual_theme.is_expressive()
        };

        self.main_transport_motion.set_effects_enabled(enabled);
        self.footer_transport_motion.set_effects_enabled(enabled);
    }

    pub(crate) fn apply_progress_style(&self) {
        let use_m3 = self.config.borrow().visual_theme.is_expressive();
        let child = if use_m3 { "m3" } else { "classic" };
        self.home_progress_stack.set_visible_child_name(child);
        self.footer_progress_stack.set_visible_child_name(child);

        let animate = use_m3 && self.player.is_playing();
        self.home_wave_progress.set_playing(animate);
        self.footer_progress.set_playing(animate);
    }

    pub(crate) fn apply_translations(&self) {
        let language = self.config.borrow().language;
        let tr = |message| i18n::text(language, message);

        self.lyrics.set_language(language);
        self.refresh_browser();

        self.sidebar_button
            .set_tooltip_text(Some(tr(Message::SidebarToggle)));
        self.search_button
            .set_tooltip_text(Some(tr(Message::SearchLibrary)));
        self.folder_button
            .set_tooltip_text(Some(tr(Message::ChooseMusicFolderTooltip)));
        self.search_entry
            .set_placeholder_text(Some(tr(Message::SearchPlaceholder)));
        self.settings_button
            .set_tooltip_text(Some(tr(Message::SettingsTitle)));

        self.sidebar_all_label.set_text(tr(Message::Library));
        self.sidebar_albums_label.set_text(tr(Message::Albums));
        self.sidebar_artists_label.set_text(tr(Message::Artists));
        self.sidebar_playlists_label
            .set_text(tr(Message::Playlists));
        self.sidebar_liked_label.set_text(tr(Message::LikedSongs));
        self.sidebar_section_label
            .set_text(tr(Message::LocalCollection));
        self.apply_source_aware_library_navigation();

        self.now_heading.set_text(tr(Message::NowPlaying));
        let (artist_tooltip, album_tooltip) = match language {
            AppLanguage::Portuguese => ("Abrir página do artista", "Abrir página do álbum"),
            AppLanguage::English => ("Open artist page", "Open album page"),
            AppLanguage::Spanish => ("Abrir página del artista", "Abrir página del álbum"),
        };
        self.player_artist.set_tooltip_text(Some(artist_tooltip));
        self.album.set_tooltip_text(Some(album_tooltip));
        self.favorite_button
            .set_tooltip_text(Some(tr(Message::FavoriteTooltip)));
        self.footer_favorite_button
            .set_tooltip_text(Some(tr(Message::FavoriteTooltip)));
        self.previous_button
            .set_tooltip_text(Some(tr(Message::PreviousTrack)));
        self.hero_play_button
            .set_tooltip_text(Some(tr(Message::PlayPause)));
        self.next_button
            .set_tooltip_text(Some(tr(Message::NextTrack)));
        self.repeat_button
            .set_tooltip_text(Some(tr(Message::RepeatTrack)));
        self.shuffle_button
            .set_tooltip_text(Some(tr(Message::Shuffle)));

        self.footer_previous
            .set_tooltip_text(Some(tr(Message::PreviousTrack)));
        self.footer_play_button
            .set_tooltip_text(Some(tr(Message::PlayPause)));
        self.footer_next
            .set_tooltip_text(Some(tr(Message::NextTrack)));
        self.footer_repeat_button
            .set_tooltip_text(Some(tr(Message::RepeatTrack)));
        self.footer_shuffle_button
            .set_tooltip_text(Some(tr(Message::Shuffle)));
        self.lyrics_button
            .set_tooltip_text(Some(tr(Message::LyricsTooltip)));

        self.music_page.set_title(Some(tr(Message::MusicTab)));
        self.lyrics_page.set_title(Some(tr(Message::LyricsTab)));
        let queue_label = match self.config.borrow().language {
            AppLanguage::Portuguese => "Fila",
            AppLanguage::English => "Queue",
            AppLanguage::Spanish => "Cola",
        };
        self.page_switcher
            .set_labels(tr(Message::MusicTab), tr(Message::LyricsTab), queue_label);
        self.empty_title.set_text(tr(Message::EmptyLibraryTitle));
        self.empty_text
            .set_text(tr(Message::EmptyLibraryDescription));
        self.empty_add.set_label(tr(Message::ChooseFolderAction));

        if self.playback_source.get() == PlaybackSource::None {
            self.player_view.set_metadata(
                tr(Message::IntegratedMusic),
                tr(Message::NoTrackSelected),
                tr(Message::ChooseFolderToStart),
            );
            self.mini_title.set_text(tr(Message::NothingPlaying));
        }

        self.apply_home_player_visibility();
        self.update_footer_source();
        self.apply_volume_icon();
    }

    pub(crate) fn apply_visual_theme(&self) {
        let (visual_theme, noctalia_sync) = {
            let config = self.config.borrow();
            (config.visual_theme, config.noctalia_theme_sync)
        };

        self.visual_theme_manager.apply(&self.window, visual_theme);
        self.mpris
            .send(crate::playback::mpris::MprisUpdate::VisualTheme(
                visual_theme,
            ));

        let (blur_mode, blur_opacity) = {
            let config = self.config.borrow();
            (config.blur_mode, config.blur_opacity)
        };
        self._theme.set_blur_preferences(blur_mode, blur_opacity);

        self.window.remove_css_class("material-blur-enabled");
        self.window.remove_css_class("material-blur-disabled");
        let material_blur_enabled = visual_theme.is_expressive() && blur_mode != BlurMode::Off;
        self.window.add_css_class(if material_blur_enabled {
            "material-blur-enabled"
        } else {
            "material-blur-disabled"
        });

        self._theme.set_noctalia_enabled(
            visual_theme == VisualTheme::Noctalia
                && noctalia_sync
                && self._theme.noctalia_shell_detected(),
        );

        self.apply_progress_style();
        self.apply_expressive_transport_effects();

        if self.player_bar.has_css_class("footer-mode-compact") {
            self.apply_compact_volume_expansion();
        }
    }

    pub(crate) fn apply_footer_mode(&self) {
        let configured = self.config.borrow().footer_mode;

        // The main Home player remains visible across internal music routes.
        // Automatic therefore stays compact while that player is visible and
        // returns to Full outside it.
        let home_player_visible = self.content_stack.visible_child_name().as_deref()
            == Some("main")
            && (self.views.visible_child_name().as_deref() == Some("music")
                && !self.config.borrow().home_player_collapsed);
        let plan = footer_mode_plan(configured, home_player_visible);

        self.player_bar.remove_css_class("footer-mode-full");
        self.player_bar.remove_css_class("footer-mode-compact");
        self.player_bar.remove_css_class("footer-mode-hidden");

        if !plan.bar_visible {
            self.compact_volume_expanded.set(false);
            self.volume_revealer.set_reveal_child(false);
            self.player_bar.add_css_class(plan.css_class);
            self.player_bar.set_visible(false);
            return;
        }

        self.player_bar.set_visible(true);
        self.footer_now_playing.set_visible(true);

        let card_margin = if plan.full {
            0
        } else {
            footer::FOOTER_COMPACT_CARD_MARGIN
        };
        self.footer_now_playing.set_vexpand(plan.full);
        self.footer_now_playing.set_valign(if plan.full {
            gtk::Align::Fill
        } else {
            gtk::Align::Center
        });
        self.footer_now_playing.set_margin_top(card_margin);
        self.footer_now_playing.set_margin_bottom(card_margin);

        self.mini_cover
            .set_display_size(plan.now_playing_artwork_size);
        self.mini_title.set_margin_bottom(plan.metadata_spacing);
        self.mini_artist.set_margin_bottom(plan.metadata_spacing);

        self.footer_center.set_visible(plan.full);
        self.footer_center.set_valign(gtk::Align::Center);
        self.footer_center.set_margin_top(0);
        self.footer_center.set_margin_bottom(0);
        self.footer_right_controls.set_visible(true);
        self.footer_right_controls.set_valign(gtk::Align::Center);

        self.footer_progress_stack.set_visible(plan.full);
        self.footer_elapsed.set_visible(plan.full);
        self.footer_duration.set_visible(plan.full);
        self.footer_previous.set_visible(true);
        self.footer_next.set_visible(true);
        self.footer_play_button.set_visible(true);
        self.footer_repeat_button.set_visible(plan.full);
        self.footer_shuffle_button.set_visible(plan.full);
        self.footer_source.set_visible(plan.full);
        self.footer_favorite_button.set_visible(plan.full);
        self.mini_artist.set_visible(true);
        self.mute_button.set_visible(true);

        if plan.full {
            self.compact_volume_expanded.set(false);
        }

        self.player_bar.add_css_class(plan.css_class);
        self.player_bar.set_height_request(plan.bar_height);
        self.footer_now_playing
            .set_size_request(plan.now_playing_size.0, plan.now_playing_size.1);
        self.footer_center
            .set_size_request(plan.center_size.0, plan.center_size.1);

        if let Some((width, height)) = plan.right_size {
            self.footer_right_controls.set_size_request(width, height);
        }

        self.apply_compact_volume_expansion();
    }

    pub(crate) fn install_footer_adaptive(&self) {
        let tier = Rc::new(Cell::new(None::<AdaptiveFooterTier>));
        let tier_state = tier.clone();
        let now_playing = self.footer_now_playing.clone();
        let cover = self.mini_cover.clone();
        let center = self.footer_center.clone();
        let right = self.footer_right_controls.clone();
        let source = self.footer_source.clone();
        let artist = self.mini_artist.clone();
        let elapsed = self.footer_elapsed.clone();
        let duration = self.footer_duration.clone();
        let shuffle = self.footer_shuffle_button.clone();
        let repeat = self.footer_repeat_button.clone();

        self.player_bar.add_tick_callback(move |bar, _| {
            if bar.has_css_class("footer-mode-compact") {
                tier_state.set(None);
                return glib::ControlFlow::Continue;
            }

            let artwork_size = footer_full_artwork_size_for_card_height(now_playing.height());
            cover.set_display_size(artwork_size);

            let next_tier = AdaptiveFooterTier::for_width(bar.width());
            if tier_state.get() == Some(next_tier) {
                return glib::ControlFlow::Continue;
            }
            tier_state.set(Some(next_tier));

            let plan = next_tier.plan();
            now_playing.set_size_request(plan.now_playing_size.0, plan.now_playing_size.1);
            center.set_size_request(plan.center_size.0, plan.center_size.1);
            right.set_size_request(plan.right_size.0, plan.right_size.1);
            source.set_visible(plan.show_source);
            artist.set_visible(plan.show_artist);
            elapsed.set_visible(plan.show_elapsed);
            duration.set_visible(plan.show_duration);
            shuffle.set_visible(plan.show_shuffle);
            repeat.set_visible(plan.show_repeat);

            glib::ControlFlow::Continue
        });
    }

    pub(crate) fn apply_home_player_visibility(&self) {
        let collapsed = self.config.borrow().home_player_collapsed;

        self.player_bounce.set_revealed(
            &self.player_revealer,
            &self.player_motion,
            &self.player_viewport,
            !collapsed,
            false,
        );
        self.player_toggle_icon.set_icon_name(Some(if collapsed {
            "audio-headphones-symbolic"
        } else {
            "view-grid-symbolic"
        }));

        self.player_toggle_button.remove_css_class("active");
        if collapsed {
            self.player_toggle_button.add_css_class("active");
        }

        let tooltip = if collapsed {
            self.tr(Message::ShowMainPlayer)
        } else {
            self.tr(Message::CollapseMainPlayer)
        };
        self.player_toggle_button.set_tooltip_text(Some(tooltip));
    }

    pub(crate) fn apply_home_preferences(&self) {
        let config = self.config.borrow();
        self.visualizer
            .widget()
            .set_visible(config.show_home_visualizer);
        self.player_view
            .set_visualizer_active(config.show_home_visualizer && self.player.is_playing());
        self.player_view.set_lyrics_visible(config.show_home_lyrics);
        self._theme
            .set_blur_preferences(config.blur_mode, config.blur_opacity);
        drop(config);
        self.apply_visual_theme();
    }

    pub(crate) fn apply_popup_visual_theme<W>(&self, widget: &W)
    where
        W: IsA<gtk::Widget>,
    {
        widget.remove_css_class("theme-material-expressive");
        widget.remove_css_class("theme-noctalia");

        if self.window.has_css_class("theme-material-expressive") {
            widget.add_css_class("theme-material-expressive");
        } else {
            widget.add_css_class("theme-noctalia");
        }
    }

    pub(crate) fn show_about_window(&self) {
        let language = self.config.borrow().language;
        let title = match language {
            AppLanguage::Portuguese => "Sobre o Nocky",
            AppLanguage::English => "About Nocky",
            AppLanguage::Spanish => "Acerca de Nocky",
        };
        let license = match language {
            AppLanguage::Portuguese => "Software livre licenciado sob a GPL-3.0",
            AppLanguage::English => "Free software licensed under GPL-3.0",
            AppLanguage::Spanish => "Software libre con licencia GPL-3.0",
        };

        let window = adw::Window::builder()
            .title(title)
            .transient_for(&self.window)
            .modal(true)
            .default_width(500)
            .default_height(520)
            .resizable(false)
            .build();
        window.add_css_class("nocky-about-window");
        self.apply_popup_visual_theme(&window);

        let toolbar = adw::ToolbarView::new();
        toolbar.add_css_class("nocky-popup-toolbar");
        toolbar.add_top_bar(&adw::HeaderBar::new());

        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        content.set_margin_top(30);
        content.set_margin_bottom(30);
        content.set_margin_start(34);
        content.set_margin_end(34);
        content.set_halign(gtk::Align::Fill);
        content.set_valign(gtk::Align::Center);
        content.add_css_class("nocky-about-content");

        let icon_surface = gtk::CenterBox::new();
        icon_surface.add_css_class("nocky-about-icon-surface");

        let icon = gtk::Image::from_icon_name(APP_ID);
        icon.set_pixel_size(96);
        icon.add_css_class("nocky-about-icon");
        icon_surface.set_center_widget(Some(&icon));

        let name = gtk::Label::new(Some("Nocky"));
        name.add_css_class("title-1");
        name.add_css_class("nocky-about-name");

        let version_prefix = match language {
            AppLanguage::Portuguese => "Versão",
            AppLanguage::English => "Version",
            AppLanguage::Spanish => "Versión",
        };
        let version = gtk::Label::new(Some(&format!(
            "{version_prefix} {}",
            env!("CARGO_PKG_VERSION")
        )));
        version.add_css_class("nocky-about-version");

        let description = gtk::Label::new(Some(self.tr(Message::AboutDescription)));
        description.set_wrap(true);
        description.set_justify(gtk::Justification::Center);
        description.set_max_width_chars(48);
        description.add_css_class("dim-label");
        description.add_css_class("nocky-about-description");

        let license_label = gtk::Label::new(Some(license));
        license_label.set_wrap(true);
        license_label.set_justify(gtk::Justification::Center);
        license_label.add_css_class("nocky-about-license");

        let technology = gtk::Label::new(Some("Rust · GTK4 · libadwaita"));
        technology.add_css_class("nocky-about-technology");

        content.append(&icon_surface);
        content.append(&name);
        content.append(&version);
        content.append(&description);
        content.append(&license_label);
        content.append(&technology);

        toolbar.set_content(Some(&content));
        window.set_content(Some(&toolbar));
        window.present();
    }

    pub(crate) fn show_shortcuts_window(&self) {
        let language = self.config.borrow().language;
        let title = match language {
            AppLanguage::Portuguese => "Atalhos de teclado",
            AppLanguage::English => "Keyboard shortcuts",
            AppLanguage::Spanish => "Atajos de teclado",
        };

        let rows: [(&str, &str); 6] = match language {
            AppLanguage::Portuguese => [
                ("Ctrl+F", "Pesquisar na biblioteca"),
                ("Ctrl+,", "Abrir Configurações"),
                ("Ctrl+O", "Escolher pasta de músicas"),
                ("F5", "Atualizar a biblioteca"),
                ("Ctrl+L", "Baixar a letra da faixa atual"),
                ("Ctrl+Q", "Fechar o Nocky"),
            ],
            AppLanguage::English => [
                ("Ctrl+F", "Search the library"),
                ("Ctrl+,", "Open Settings"),
                ("Ctrl+O", "Choose the music folder"),
                ("F5", "Refresh the library"),
                ("Ctrl+L", "Download lyrics for the current track"),
                ("Ctrl+Q", "Quit Nocky"),
            ],
            AppLanguage::Spanish => [
                ("Ctrl+F", "Buscar en la biblioteca"),
                ("Ctrl+,", "Abrir Configuración"),
                ("Ctrl+O", "Elegir carpeta de música"),
                ("F5", "Actualizar la biblioteca"),
                ("Ctrl+L", "Descargar la letra de la canción actual"),
                ("Ctrl+Q", "Cerrar Nocky"),
            ],
        };

        let window = adw::Window::builder()
            .title(title)
            .transient_for(&self.window)
            .modal(true)
            .default_width(560)
            .default_height(520)
            .resizable(false)
            .build();
        window.add_css_class("nocky-shortcuts-window");
        self.apply_popup_visual_theme(&window);

        let toolbar = adw::ToolbarView::new();
        toolbar.add_css_class("nocky-popup-toolbar");
        toolbar.add_top_bar(&adw::HeaderBar::new());

        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        content.set_margin_top(22);
        content.set_margin_bottom(26);
        content.set_margin_start(24);
        content.set_margin_end(24);
        content.add_css_class("nocky-shortcuts-content");

        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::None);
        list.add_css_class("boxed-list");
        list.add_css_class("nocky-shortcuts-list");

        for (shortcut, description) in rows {
            let shortcut_label = gtk::Label::new(Some(shortcut));
            shortcut_label.set_width_chars(9);
            shortcut_label.set_xalign(0.5);
            shortcut_label.add_css_class("nocky-shortcut-key");

            let description_label = gtk::Label::new(Some(description));
            description_label.set_xalign(0.0);
            description_label.set_hexpand(true);
            description_label.set_wrap(true);
            description_label.add_css_class("nocky-shortcut-description");

            let row_content = gtk::Box::new(gtk::Orientation::Horizontal, 16);
            row_content.set_margin_top(12);
            row_content.set_margin_bottom(12);
            row_content.set_margin_start(14);
            row_content.set_margin_end(14);
            row_content.append(&shortcut_label);
            row_content.append(&description_label);

            let row = gtk::ListBoxRow::new();
            row.set_activatable(false);
            row.set_selectable(false);
            row.set_child(Some(&row_content));
            row.add_css_class("nocky-shortcut-row");
            list.append(&row);
        }

        content.append(&list);
        toolbar.set_content(Some(&content));
        window.set_content(Some(&toolbar));
        window.present();
    }
}

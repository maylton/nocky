//! Visual update helpers for `AppController`.

use super::*;

impl AppController {
    pub(crate) fn tr(&self, message: Message) -> &'static str {
        i18n::text(self.config.borrow().language, message)
    }

    // nocky_real_metadata_transition_v1
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

    // nocky_theme_scoped_expressive_effects_v1: Material-only compact volume spring
    pub(crate) fn apply_compact_volume_expansion(&self) {
        let compact = self.player_bar.has_css_class("footer-mode-compact");
        let expanded = compact && self.compact_volume_expanded.get();
        let material_expressive =
            self.config.borrow().visual_theme == VisualTheme::MaterialExpressive;

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
            config.expressive_transport_effects
                && config.visual_theme == VisualTheme::MaterialExpressive
        };

        self.main_transport_motion.set_effects_enabled(enabled);
        self.footer_transport_motion.set_effects_enabled(enabled);
    }

    pub(crate) fn apply_progress_style(&self) {
        let use_m3 = self.config.borrow().visual_theme == VisualTheme::MaterialExpressive;
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

        // material_carousel_indicator_blur_runtime_v2
        let (blur_mode, blur_opacity) = {
            let config = self.config.borrow();
            (config.blur_mode, config.blur_opacity)
        };
        self._theme.set_blur_preferences(blur_mode, blur_opacity);

        self.window.remove_css_class("material-blur-enabled");
        self.window.remove_css_class("material-blur-disabled");
        let material_blur_enabled =
            visual_theme == VisualTheme::MaterialExpressive && blur_mode != BlurMode::Off;
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

        // nocky_footer_metadata_fill_available_height_v8
        // nocky_footer_compact_restores_vertical_air_v12
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

        // nocky_footer_metadata_full_mode_breathing_room_v4
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

            // nocky_footer_artwork_tracks_card_height_v11
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
}

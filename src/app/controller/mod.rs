//! Application controller data structures.

mod actions;
mod appearance;
mod background;
mod callbacks;
mod construction;
mod favorites;
mod feedback;
mod lyrics;
mod navigation;
mod offline;
mod persistence;
mod playback;
mod queue;
mod youtube;

pub(crate) use construction::build_application;

use crate::{
    app::sidebar::build_sidebar,
    app::state::{AppState, PlaybackSource, YouTubePlaybackState},
    app::{
        library_state::scanned_library_matches,
        media::{
            format_time, mpris_track_id, mpris_youtube_track_id, playback_error_message,
            redact_stream_url,
        },
    },
    background::{BackgroundChannel, BackgroundMessage},
    browser::{
        BrowserEvent, BrowserPlaybackState, BrowserRenderContext, BrowserRoute, LibraryBrowser,
        YouTubeCollectionRoute,
    },
    config::{self, AppLanguage, BlurMode, StartupSource, VisualTheme},
    dialogs,
    dialogs::SettingsEvent,
    i18n::{self, Message},
    library,
    listening_history::{self, ListeningHistory, ListeningSource},
    lyrics::{self as lyrics_domain, LyricLine, LyricsPresenter},
    model::{Track, TrackData},
    offline_store::{download_youtube_track, OfflineStore, OFFLINE_STREAM_REJECTED_PREFIX},
    onboarding,
    playback::{
        queue::{
            queue_end_action, PlaybackQueue, QueueEndAction, QueueEntryId, QueueMedia,
            QueuePresentation, QueueSection, QueueSnapshot, QueueSource, QueueSourceKind,
            ShuffleNavigator,
        },
        session::PlaybackSession,
        transition::TransitionClock,
        PlaybackEngine, PlaybackEvent,
    },
    reveal_bounce::RevealBounce,
    theme,
    ui::{
        footer::{
            self, build_footer_view, footer_full_artwork_size_for_card_height, footer_mode_plan,
            AdaptiveFooterTier, FooterViewParts, FOOTER_ARTWORK_SOURCE_SIZE,
        },
        player::{PlayerView, PlayerViewHandle},
        settings::SettingsPage,
        widgets::{
            build_cover, run_compact_volume_spring, AnimatedPageSwitcher, CompactVolumeSpring,
            CoverView, ExpressiveTransport, TopPage, WaveProgress,
        },
    },
    visual_theme,
    visualizer::SpectrumVisualizer,
    youtube::{
        self as youtube_domain, cache_items_for_browser, credited_artists,
        diagnostics as youtube_diagnostics, load_library_cache, resolve_youtube_collection_item,
        youtube_collection_cache_key, youtube_collection_key, youtube_home_prefetch_candidates,
        YouTubeBridge, YouTubeItem, YouTubeLibraryCache, YouTubePage, YouTubePageEvent,
        YouTubeSearchResults, YouTubeStatus,
    },
    APP_ID, HOME_PLAYER_WIDTH,
};
use adw::prelude::*;
use gtk::prelude::FileExt;
use gtk::{gdk, gio, glib};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
    rc::Rc,
    sync::{mpsc, Arc, Mutex},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub(crate) struct AppController {
    pub(crate) window: adw::ApplicationWindow,
    pub(crate) toast_overlay: adw::ToastOverlay,
    pub(crate) player: PlaybackEngine,
    pub(crate) state: RefCell<AppState>,
    pub(crate) playback_queue_v2: RefCell<PlaybackQueue>,
    pub(crate) active_queue_source: Cell<QueueSourceKind>,
    pub(crate) queue_last_saved_snapshot: RefCell<QueueSnapshot>,
    pub(crate) queue_dragged_entry: Cell<Option<QueueEntryId>>,
    pub(crate) queue_v2_pending_entry: Cell<Option<QueueEntryId>>,
    pub(crate) config: RefCell<config::AppConfig>,
    pub(crate) listening_history: RefCell<ListeningHistory>,
    pub(crate) listening_session_id: RefCell<Option<String>>,
    pub(crate) listening_session_last_saved_seconds: Cell<u64>,
    pub(crate) listening_history_context: RefCell<listening_history::PlaybackHistoryContext>,
    pub(crate) pending_resume_position_us: Cell<Option<i64>>,
    pub(crate) restored_playback_session: RefCell<Option<PlaybackSession>>,
    pub(crate) startup_restore_autoplay: Cell<Option<bool>>,
    pub(crate) playback_session_last_position_seconds: Cell<u64>,
    pub(crate) playback_session_last_shuffle: Cell<bool>,
    pub(crate) playback_session_last_repeat: Cell<bool>,
    pub(crate) playback_session_restore_attempts: Cell<u8>,
    pub(crate) updating_progress: Cell<bool>,
    pub(crate) scanning: Cell<bool>,
    pub(crate) shuffle_enabled: Cell<bool>,
    pub(crate) shuffle_navigation: RefCell<ShuffleNavigator>,
    pub(crate) rng_state: Cell<u64>,
    pub(crate) search_query: RefCell<String>,
    pub(crate) lyrics_pending: RefCell<HashSet<PathBuf>>,
    pub(crate) background: BackgroundChannel,
    pub(crate) mpris: crate::playback::mpris::MprisBridge,
    pub(crate) last_mpris_position: Cell<i64>,
    pub(crate) playback_source: Cell<PlaybackSource>,
    pub(crate) youtube_state: RefCell<Option<YouTubePlaybackState>>,
    pub(crate) youtube_request_id: Cell<u64>,
    pub(crate) youtube_search_request_id: Cell<u64>,
    pub(crate) youtube_recovery_in_progress: Cell<bool>,
    pub(crate) youtube_recovery_attempted: Cell<bool>,
    pub(crate) youtube_recovery_retry_count: Cell<u8>,
    pub(crate) youtube_recovery_generation: Cell<u64>,
    pub(crate) youtube_recovery_resume_us: Cell<i64>,
    pub(crate) youtube_recovery_was_playing: Cell<bool>,
    pub(crate) youtube_playlist_request_id: Cell<u64>,
    pub(crate) youtube_collection_play_request_id: Cell<u64>,
    pub(crate) youtube_collection_queue_request_id: Cell<u64>,
    pub(crate) youtube_collection_prefetching: Cell<bool>,
    pub(crate) youtube_playlist_loading: Cell<bool>,
    pub(crate) youtube_playlist_prefetching: Cell<bool>,
    pub(crate) youtube_pending_playlist: RefCell<Option<YouTubeItem>>,
    pub(crate) youtube_bridge: Option<Arc<YouTubeBridge>>,
    pub(crate) youtube_library: RefCell<YouTubeLibraryCache>,
    pub(crate) offline_store: RefCell<OfflineStore>,
    pub(crate) offline_download_pending: RefCell<HashSet<String>>,
    pub(crate) youtube_like_request_id: Cell<u64>,
    pub(crate) youtube_like_pending: RefCell<HashMap<String, u64>>,
    pub(crate) sidebar: gtk::Revealer,
    pub(crate) sidebar_motion: gtk::Fixed,
    pub(crate) sidebar_content: gtk::Box,
    pub(crate) sidebar_bounce: Rc<RevealBounce>,
    pub(crate) sidebar_button: gtk::ToggleButton,
    pub(crate) sidebar_all: gtk::Button,
    pub(crate) sidebar_all_label: gtk::Label,
    pub(crate) sidebar_albums: gtk::Button,
    pub(crate) sidebar_albums_label: gtk::Label,
    pub(crate) sidebar_artists: gtk::Button,
    pub(crate) sidebar_artists_label: gtk::Label,
    pub(crate) sidebar_playlists: gtk::Button,
    pub(crate) sidebar_playlists_label: gtk::Label,
    pub(crate) sidebar_liked: gtk::Button,
    pub(crate) sidebar_liked_label: gtk::Label,
    pub(crate) sidebar_section_label: gtk::Label,
    pub(crate) search_button: gtk::ToggleButton,
    pub(crate) folder_button: gtk::Button,
    pub(crate) search_entry: gtk::SearchEntry,
    pub(crate) settings_button: gtk::ToggleButton,
    pub(crate) content_stack: gtk::Stack,
    pub(crate) settings_page: Rc<SettingsPage>,
    pub(crate) views: adw::ViewStack,
    pub(crate) music_page: adw::ViewStackPage,
    pub(crate) lyrics_page: adw::ViewStackPage,
    pub(crate) queue_page_list: gtk::Box,
    pub(crate) queue_page_summary: gtk::Label,
    pub(crate) queue_page_source: gtk::Label,
    pub(crate) queue_page_upcoming_badge: gtk::Label,
    pub(crate) queue_page_total_badge: gtk::Label,
    pub(crate) queue_page_clear_upcoming: gtk::Button,
    pub(crate) queue_page_clear_all: gtk::Button,
    pub(crate) queue_page_popover_proxy: gtk::Popover,
    pub(crate) queue_page_last_snapshot: RefCell<Option<QueueSnapshot>>,
    pub(crate) queue_page_last_source: Cell<Option<QueueSourceKind>>,
    pub(crate) page_switcher: Rc<AnimatedPageSwitcher>,
    pub(crate) browser: LibraryBrowser,
    pub(crate) lyrics: LyricsPresenter,
    pub(crate) youtube_page: Rc<YouTubePage>,
    pub(crate) player_view: PlayerViewHandle,
    pub(crate) player_revealer: gtk::Revealer,
    pub(crate) player_motion: gtk::Fixed,
    pub(crate) player_viewport: gtk::ScrolledWindow,
    pub(crate) player_bounce: Rc<RevealBounce>,
    pub(crate) player_toggle_button: gtk::Button,
    pub(crate) player_toggle_icon: gtk::Image,
    pub(crate) player_artist: gtk::Label,
    pub(crate) album: gtk::Label,
    pub(crate) now_heading: gtk::Label,
    pub(crate) favorite_button: gtk::Button,
    pub(crate) previous_button: gtk::Button,
    pub(crate) hero_play_button: gtk::Button,
    pub(crate) main_transport_motion: Rc<ExpressiveTransport>,
    pub(crate) next_button: gtk::Button,
    pub(crate) mini_title: gtk::Label,
    pub(crate) mini_artist: gtk::Label,
    pub(crate) footer_source: gtk::Label,
    pub(crate) footer_now_playing: gtk::Button,
    pub(crate) footer_center: gtk::Box,
    pub(crate) footer_right_controls: gtk::Box,
    pub(crate) volume_revealer: gtk::Revealer,
    pub(crate) music_stack: gtk::Stack,
    pub(crate) empty_title: gtk::Label,
    pub(crate) empty_text: gtk::Label,
    pub(crate) empty_add: gtk::Button,
    pub(crate) hero_cover: CoverView,
    pub(crate) mini_cover: CoverView,
    pub(crate) player_bar: gtk::CenterBox,
    pub(crate) play_icon: gtk::Image,
    pub(crate) hero_play_icon: gtk::Image,
    pub(crate) favorite_icon: gtk::Image,
    pub(crate) footer_favorite_icon: gtk::Image,
    pub(crate) footer_favorite_button: gtk::Button,
    pub(crate) progress: gtk::Scale,
    pub(crate) home_progress_stack: gtk::Stack,
    pub(crate) home_wave_progress: WaveProgress,
    pub(crate) elapsed: gtk::Label,
    pub(crate) duration: gtk::Label,
    pub(crate) footer_progress_stack: gtk::Stack,
    pub(crate) footer_traditional_progress: gtk::Scale,
    pub(crate) footer_progress: WaveProgress,
    pub(crate) footer_elapsed: gtk::Label,
    pub(crate) footer_duration: gtk::Label,
    pub(crate) volume: gtk::Adjustment,
    pub(crate) mute_icon: gtk::Image,
    pub(crate) mute_button: gtk::Button,
    pub(crate) volume_before_mute: Cell<f64>,
    pub(crate) compact_volume_expanded: Cell<bool>,
    pub(crate) compact_volume_spring_generation: Rc<Cell<u64>>,
    pub(crate) footer_metadata_transition: TransitionClock,
    pub(crate) lyrics_button: gtk::ToggleButton,
    pub(crate) footer_previous: gtk::Button,
    pub(crate) footer_play_button: gtk::Button,
    pub(crate) footer_transport_motion: Rc<ExpressiveTransport>,
    pub(crate) footer_next: gtk::Button,
    pub(crate) footer_repeat_button: gtk::ToggleButton,
    pub(crate) footer_shuffle_button: gtk::ToggleButton,
    pub(crate) repeat_button: gtk::ToggleButton,
    pub(crate) shuffle_button: gtk::ToggleButton,
    pub(crate) visualizer: SpectrumVisualizer,
    pub(crate) visual_theme_manager: Rc<visual_theme::VisualThemeManager>,
    pub(crate) _theme: Rc<theme::ThemeBridge>,
}

impl AppController {
    pub(crate) fn open_library_home(&self) {
        self.search_query.replace(String::new());
        self.search_entry.set_text("");
        self.content_stack.set_visible_child_name("main");
        if self.settings_button.is_active() {
            self.settings_button.set_active(false);
        }
        self.views.set_visible_child_name("music");

        if self.lyrics_button.is_active() {
            self.lyrics_button.set_active(false);
        }

        self.navigate_browser(BrowserRoute::All);
    }

    pub(crate) fn sync_active_library(&self) {
        let source = self.config.borrow().startup_source;
        match source {
            Some(StartupSource::YouTube) => {
                let (connected, syncing) = {
                    let library = self.youtube_library.borrow();
                    (library.connected, library.syncing)
                };

                if !connected {
                    self.show_toast("Conecte sua conta do YouTube Music primeiro");
                    return;
                }
                if syncing {
                    self.show_toast("A biblioteca já está sendo sincronizada");
                    return;
                }

                if self.sync_youtube_library(true, true) {
                    self.show_toast("Sincronizando biblioteca do YouTube Music…");
                }
            }
            _ => {
                if self.scanning.get() {
                    self.show_toast("A biblioteca local já está sendo atualizada");
                    return;
                }
                self.scan_library();
            }
        }
    }

    pub(crate) fn set_lyrics_message(&self, message: &str) {
        self.lyrics.show_message(message, None);
    }

    pub(crate) fn apply_startup_source(self: &Rc<Self>) {
        self.views.set_visible_child_name("music");
        if self.lyrics_button.is_active() {
            self.lyrics_button.set_active(false);
        }

        let force_onboarding = std::env::var_os("NOCKY_FORCE_ONBOARDING").is_some();

        if force_onboarding || !self.config.borrow().onboarding_completed {
            if force_onboarding {
                eprintln!("NOCKY_FORCE_ONBOARDING is set; showing the first-run wizard");
            }
            self.show_onboarding_wizard();
            return;
        }

        self.apply_source_aware_library_navigation();

        match self.config.borrow().startup_source {
            Some(StartupSource::Local) => self.refresh_browser(),
            Some(StartupSource::YouTube) => {
                self.refresh_browser();
                self.refresh_youtube_status();
            }
            None => self.show_startup_source_dialog(true),
        }
    }

    pub(crate) fn set_startup_source(&self, source: StartupSource) {
        self.switch_active_queue_source(Self::queue_source_kind(source));
        if self.active_queue_source.get() != Self::queue_source_kind(source) {
            return;
        }

        self.config.borrow_mut().startup_source = Some(source);
        self.save_config();
        self.views.set_visible_child_name("music");
        if self.lyrics_button.is_active() {
            self.lyrics_button.set_active(false);
        }
        self.apply_source_aware_library_navigation();

        if matches!(self.browser.route(), BrowserRoute::Liked) {
            self.navigate_browser(BrowserRoute::All);
        }

        match source {
            StartupSource::Local => self.refresh_browser(),
            StartupSource::YouTube => {
                self.refresh_browser();
                self.refresh_youtube_status();
            }
        }
    }

    pub(crate) fn apply_source_aware_library_navigation(&self) {
        let config = self.config.borrow();
        let youtube = config.startup_source == Some(StartupSource::YouTube);

        let (section, liked) = match (config.language, youtube) {
            (AppLanguage::Portuguese, false) => ("COLEÇÃO LOCAL", "Músicas curtidas locais"),
            (AppLanguage::Portuguese, true) => ("YOUTUBE MUSIC", "Músicas curtidas"),
            (AppLanguage::English, false) => ("LOCAL COLLECTION", "Local liked songs"),
            (AppLanguage::English, true) => ("YOUTUBE MUSIC", "Liked songs"),
            (AppLanguage::Spanish, false) => ("COLECCIÓN LOCAL", "Canciones locales favoritas"),
            (AppLanguage::Spanish, true) => ("YOUTUBE MUSIC", "Canciones favoritas"),
        };

        self.sidebar_section_label.set_text(section);
        self.sidebar_liked_label.set_text(liked);
        self.sidebar_liked
            .set_visible(config.startup_source.is_some());
        self.sidebar_liked.set_tooltip_text(Some(liked));
    }

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

    pub(crate) fn open_current_artist_from_player(&self) {
        let artist = match self.playback_source.get() {
            PlaybackSource::Local => {
                let state = self.state.borrow();
                state
                    .current
                    .and_then(|index| state.tracks.get(index))
                    .and_then(|track| credited_artists(&track.artist).into_iter().next())
            }
            PlaybackSource::YouTube => self
                .current_youtube_item()
                .and_then(|item| credited_artists(&item.artist).into_iter().next()),
            PlaybackSource::None => None,
        };

        let Some(artist) = artist.filter(|artist| !artist.trim().is_empty()) else {
            return;
        };

        self.close_settings_page();
        self.views.set_visible_child_name("music");

        if self.playback_source.get() == PlaybackSource::YouTube {
            let item = {
                let library = self.youtube_library.borrow();
                library
                    .artists
                    .iter()
                    .find(|entry| entry.title.eq_ignore_ascii_case(&artist))
                    .map(|entry| entry.source.clone())
            }
            .unwrap_or_else(|| YouTubeItem {
                result_type: "artist".to_string(),
                title: artist.clone(),
                artist: artist.clone(),
                ..YouTubeItem::default()
            });
            self.load_youtube_collection_for_browser(item);
        } else {
            self.navigate_browser(BrowserRoute::Artist(artist));
        }
    }

    pub(crate) fn open_current_album_from_player(&self) {
        let (album, artist) = match self.playback_source.get() {
            PlaybackSource::Local => {
                let state = self.state.borrow();
                let Some(track) = state.current.and_then(|index| state.tracks.get(index)) else {
                    return;
                };
                (
                    track.album.trim().to_string(),
                    credited_artists(&track.artist)
                        .into_iter()
                        .next()
                        .unwrap_or_default(),
                )
            }
            PlaybackSource::YouTube => {
                let Some(item) = self.current_youtube_item() else {
                    return;
                };
                (
                    item.album.trim().to_string(),
                    credited_artists(&item.artist)
                        .into_iter()
                        .next()
                        .unwrap_or_default(),
                )
            }
            PlaybackSource::None => return,
        };

        if album.is_empty() {
            return;
        }

        self.close_settings_page();
        self.views.set_visible_child_name("music");

        if self.playback_source.get() == PlaybackSource::YouTube {
            let item = {
                let library = self.youtube_library.borrow();
                library
                    .albums
                    .iter()
                    .find(|entry| {
                        entry.title.eq_ignore_ascii_case(&album)
                            && (artist.is_empty()
                                || entry.source.artist.eq_ignore_ascii_case(&artist)
                                || entry.subtitle.eq_ignore_ascii_case(&artist))
                    })
                    .or_else(|| {
                        library
                            .albums
                            .iter()
                            .find(|entry| entry.title.eq_ignore_ascii_case(&album))
                    })
                    .map(|entry| entry.source.clone())
            }
            .unwrap_or_else(|| YouTubeItem {
                result_type: "album".to_string(),
                title: album.clone(),
                album: album.clone(),
                artist,
                ..YouTubeItem::default()
            });
            self.load_youtube_collection_for_browser(item);
        } else {
            self.navigate_browser(BrowserRoute::Album(album));
        }
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

    pub(crate) fn open_settings_page(&self) {
        let initial = self.config.borrow().clone();
        self.settings_page
            .rebuild(&initial, self._theme.noctalia_shell_detected());
        self.search_button.set_active(false);
        self.content_stack.set_visible_child_name("settings");
        if !self.settings_button.is_active() {
            self.settings_button.set_active(true);
        }
        self.apply_footer_mode();
    }

    pub(crate) fn close_settings_page(&self) {
        if self.content_stack.visible_child_name().as_deref() != Some("settings") {
            return;
        }
        self.content_stack.set_visible_child_name("main");
        if self.settings_button.is_active() {
            self.settings_button.set_active(false);
        }
        self.apply_footer_mode();
    }

    pub(crate) fn handle_settings_events(self: &Rc<Self>) {
        while let Some(event) = self.settings_page.try_recv() {
            self.apply_settings_event(event);
        }
    }

    pub(crate) fn apply_settings_event(self: &Rc<Self>, event: SettingsEvent) {
        match event {
            SettingsEvent::Language(language) => {
                self.config.borrow_mut().language = language;
                self.save_config();
                self.apply_translations();
                let initial = self.config.borrow().clone();
                self.settings_page
                    .rebuild(&initial, self._theme.noctalia_shell_detected());
            }
            SettingsEvent::StartupSource(source) => self.set_startup_source(source),
            SettingsEvent::BlurMode(mode) => {
                self.config.borrow_mut().blur_mode = mode;
                self.save_config();
                self.apply_home_preferences();
            }
            SettingsEvent::BlurOpacityPreview(value) => {
                let custom = {
                    let mut config = self.config.borrow_mut();
                    config.blur_opacity = value;
                    config.blur_mode == BlurMode::Custom
                };
                if custom {
                    self.apply_home_preferences();
                }
            }
            SettingsEvent::BlurOpacityCommit(value) => {
                self.config.borrow_mut().blur_opacity = value;
                self.save_config();
            }
            SettingsEvent::ShowHomeVisualizer(active) => {
                self.config.borrow_mut().show_home_visualizer = active;
                self.save_config();
                self.apply_home_preferences();
            }
            SettingsEvent::ShowHomeLyrics(active) => {
                self.config.borrow_mut().show_home_lyrics = active;
                self.save_config();
                self.apply_home_preferences();
            }
            SettingsEvent::ShowPersonalizedHomeHistory(active) => {
                self.config.borrow_mut().show_personalized_home_history = active;
                self.save_config();
                self.refresh_browser();
            }
            SettingsEvent::CollectListeningHistory(active) => {
                self.config.borrow_mut().collect_listening_history = active;
                self.listening_history
                    .borrow_mut()
                    .set_recording_enabled(active);
                self.save_config();
                self.show_toast(if active {
                    "O Nocky voltou a aprender com sua atividade"
                } else {
                    "O registro de novas reproduções foi desativado"
                });
            }
            SettingsEvent::ClearListeningHistory => {
                let cleared = self.listening_history.borrow_mut().clear();
                self.refresh_browser();
                self.show_toast(if cleared {
                    "Histórico de reprodução apagado"
                } else {
                    "O histórico já está vazio"
                });
            }
            SettingsEvent::VisualTheme(theme) => {
                self.config.borrow_mut().visual_theme = theme;
                self.save_config();
                self.apply_visual_theme();
                self.refresh_browser();
            }
            SettingsEvent::FooterMode(mode) => {
                self.config.borrow_mut().footer_mode = mode;
                self.save_config();
                self.apply_footer_mode();
            }
            SettingsEvent::ExpressiveTransportEffects(active) => {
                self.config.borrow_mut().expressive_transport_effects = active;
                self.save_config();
                self.apply_expressive_transport_effects();
            }
            SettingsEvent::ExpressiveHomeCardEffects(active) => {
                self.config.borrow_mut().expressive_home_card_effects = active;
                self.save_config();
                self.refresh_browser();
            }
            SettingsEvent::AutoDownloadLyrics(active) => {
                self.config.borrow_mut().auto_download_lyrics = active;
                self.save_config();
                self.apply_home_preferences();
            }
            SettingsEvent::ResumePlaybackOnStartup(active) => {
                self.config.borrow_mut().resume_playback_on_startup = active;
                self.save_config();
            }
            SettingsEvent::YouTubeAutoSync(active) => {
                self.config.borrow_mut().youtube_auto_sync = active;
                self.save_config();
                self.apply_home_preferences();
            }
            SettingsEvent::OfflineCollectionAutoSync(active) => {
                self.config.borrow_mut().offline_collection_auto_sync = active;
                self.save_config();
                if active {
                    self.sync_followed_offline_collections();
                }
            }
            SettingsEvent::NoctaliaThemeSync(active) => {
                self.config.borrow_mut().noctalia_theme_sync = active;
                self.save_config();
                self.apply_home_preferences();
            }
            SettingsEvent::ManageYouTube => self.show_youtube_settings_dialog(),
            SettingsEvent::OpenOfflineFolder => {
                let path = self.offline_store.borrow().root_dir();
                if let Err(error) = std::fs::create_dir_all(&path) {
                    self.show_toast(&format!("Não foi possível abrir a pasta offline: {error}"));
                    return;
                }

                let file = gio::File::for_path(path);
                if let Err(error) = gio::AppInfo::launch_default_for_uri(
                    &file.uri(),
                    None::<&gio::AppLaunchContext>,
                ) {
                    self.show_toast(&format!("Não foi possível abrir a pasta offline: {error}"));
                }
            }
            SettingsEvent::CleanOfflinePartials => {
                let result = self.offline_store.borrow().clear_partials();
                match result {
                    Ok(0) => self.show_toast("Não há downloads incompletos para remover"),
                    Ok(count) => {
                        self.show_toast(&format!("{count} arquivos incompletos foram removidos"))
                    }
                    Err(error) => self.show_toast(&error),
                }

                let initial = self.config.borrow().clone();
                self.settings_page
                    .rebuild(&initial, self._theme.noctalia_shell_detected());
            }
            SettingsEvent::ClearOfflineDownloads => {
                if !self.offline_download_pending.borrow().is_empty() {
                    self.show_toast(
                        "Aguarde os downloads atuais terminarem antes de limpar os arquivos",
                    );
                    return;
                }

                let result = self.offline_store.borrow_mut().clear_all();
                match result {
                    Ok((0, _)) => self.show_toast("O armazenamento offline já está vazio"),
                    Ok((count, _)) => self.show_toast(&format!(
                        "{count} faixas offline foram removidas deste dispositivo"
                    )),
                    Err(error) => self.show_toast(&error),
                }

                self.refresh_browser();
                let initial = self.config.borrow().clone();
                self.settings_page
                    .rebuild(&initial, self._theme.noctalia_shell_detected());
            }
        }
    }

    pub(crate) fn show_youtube_settings_dialog(self: &Rc<Self>) {
        dialogs::present_youtube_settings(&self.window, self.youtube_page.root());
    }

    // themed_about_and_shortcuts_windows_v2
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

        // noctalia_about_action_release_polish_v1
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

    pub(crate) fn show_onboarding_wizard(self: &Rc<Self>) {
        let initial = self.config.borrow().clone();
        let language = initial.language;
        let noctalia_available = self._theme.noctalia_shell_detected();
        let weak = Rc::downgrade(self);

        onboarding::present(
            &self.window,
            language,
            &initial,
            noctalia_available,
            move |choices| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };

                let choose_local_folder = {
                    let mut config = controller.config.borrow_mut();
                    config.startup_source = Some(choices.startup_source);
                    config.show_personalized_home_history = choices.show_personalized_home_history;
                    config.blur_mode = choices.blur_mode;
                    config.blur_opacity = choices.blur_opacity;
                    config.footer_mode = choices.footer_mode;
                    config.visual_theme = choices.visual_theme;
                    config.noctalia_theme_sync = noctalia_available && choices.noctalia_theme_sync;
                    config.onboarding_completed = true;

                    choices.startup_source == StartupSource::Local
                        && config.music_directory.is_none()
                };

                controller.save_config();
                controller.apply_home_preferences();
                controller.apply_footer_mode();
                controller.apply_startup_source();

                if choose_local_folder {
                    let controller = controller.clone();
                    glib::idle_add_local_once(move || {
                        controller.choose_library_folder();
                    });
                }
            },
        );
    }

    pub(crate) fn show_startup_source_dialog(self: &Rc<Self>, first_run: bool) {
        let language = self.config.borrow().language;
        let weak = Rc::downgrade(self);

        dialogs::present_startup_source(&self.window, language, first_run, move |source| {
            if let Some(controller) = weak.upgrade() {
                controller.set_startup_source(source);
            }
        });
    }

    pub(crate) fn load_saved_library(self: &Rc<Self>) {
        if self.config.borrow().music_directory.is_some() {
            self.scan_library();
        }
    }

    pub(crate) fn choose_library_folder(self: &Rc<Self>) {
        let dialog = gtk::FileDialog::builder()
            .title(self.tr(Message::ChooseFolderAction))
            .accept_label("Selecionar")
            .modal(true)
            .build();

        if let Some(path) = self.config.borrow().music_directory.as_ref() {
            let folder = gio::File::for_path(path);
            dialog.set_initial_folder(Some(&folder));
        }

        let weak = Rc::downgrade(self);
        dialog.select_folder(Some(&self.window), gio::Cancellable::NONE, move |result| {
            let Some(controller) = weak.upgrade() else {
                return;
            };
            let Ok(folder) = result else {
                return;
            };
            let Some(path) = folder.path() else {
                controller.show_toast("Apenas pastas locais são suportadas por enquanto");
                return;
            };

            controller.config.borrow_mut().music_directory = Some(path);
            controller.save_config();
            controller.scan_library();
        });
    }

    pub(crate) fn scan_library(&self) {
        if self.scanning.replace(true) {
            self.show_toast("A biblioteca já está sendo escaneada");
            return;
        }

        let Some(root) = self.config.borrow().music_directory.clone() else {
            self.scanning.set(false);
            self.show_toast("Escolha uma pasta de músicas primeiro");
            return;
        };

        let sender = self.background.sender();
        thread::spawn(move || {
            let result = library::scan_music_directory(&root);
            let _ = sender.send(BackgroundMessage::LibraryScanned { root, result });
        });
    }

    pub(crate) fn apply_scanned_library(&self, data: Vec<TrackData>) {
        let unchanged = {
            let state = self.state.borrow();
            scanned_library_matches(&state.tracks, &data)
        };
        if unchanged {
            return;
        }

        let previous_path = {
            let state = self.state.borrow();
            state
                .current
                .and_then(|index| state.tracks.get(index))
                .map(|track| track.path.clone())
        };

        let tracks = data.into_iter().map(Track::from).collect::<Vec<_>>();
        let count = tracks.len();
        let selected = previous_path
            .as_ref()
            .and_then(|path| tracks.iter().position(|track| &track.path == path));

        {
            let mut state = self.state.borrow_mut();
            state.tracks = tracks;
            state.current = None;
            state.playback_queue = (0..state.tracks.len()).collect();
        }

        self.refresh_browser();
        if count > 0 {
            let initial_queue = self.browser.visible_indices();
            if !initial_queue.is_empty() {
                self.state.borrow_mut().playback_queue = initial_queue;
            }
            if self.playback_source.get() != PlaybackSource::YouTube
                && self.config.borrow().startup_source != Some(StartupSource::YouTube)
            {
                self.select_track(selected.unwrap_or(0), false);
            }
        } else {
            if self.playback_source.get() != PlaybackSource::YouTube {
                self.reset_now_playing("No supported audio files were found");
            }
            self.show_toast("Nenhum arquivo de áudio compatível foi encontrado nessa pasta");
        }
    }

    pub(crate) fn browser_playback_state(&self) -> BrowserPlaybackState {
        let context = self.listening_history_context.borrow();
        let youtube = self.youtube_library.borrow();
        let loading_collections = youtube
            .playlist_loading
            .iter()
            .chain(youtube.collection_loading.iter())
            .map(|key| key.trim().to_lowercase())
            .collect::<HashSet<_>>();

        BrowserPlaybackState {
            playing: self.play_icon.icon_name().as_deref() == Some("media-playback-pause-symbolic"),
            collection_kind: context.kind.clone(),
            collection_id: context.id.clone(),
            collection_title: context.title.clone(),
            loading_collections,
        }
    }

    pub(crate) fn refresh_artist_directory(&self) {
        if !matches!(self.browser.route(), BrowserRoute::Artists) {
            return;
        }

        let state = self.state.borrow();
        let config = self.config.borrow();
        let youtube = self.youtube_library.borrow();
        let query = self.search_query.borrow();
        let youtube_only = config.startup_source == Some(StartupSource::YouTube);
        let effective_tracks: &[Track] = if youtube_only {
            &[]
        } else {
            state.tracks.as_slice()
        };

        self.browser
            .refresh_artists_page(effective_tracks, &youtube, &query);
    }

    pub(crate) fn refresh_browser(&self) {
        let home_scroll_positions = self.browser.home_scroll_positions();
        let playback = self.browser_playback_state();
        let state = self.state.borrow();
        let config = self.config.borrow();
        let youtube = self.youtube_library.borrow();
        let query = self.search_query.borrow();
        let youtube_only = config.startup_source == Some(StartupSource::YouTube);
        let effective_tracks: &[Track] = if youtube_only {
            &[]
        } else {
            state.tracks.as_slice()
        };
        let mut effective_config = config.clone();
        if youtube_only {
            effective_config.playlists.clear();
        }
        let has_library = !query.trim().is_empty()
            || !effective_tracks.is_empty()
            || youtube.has_content()
            || youtube.syncing;
        self.music_stack
            .set_visible_child_name(if has_library { "library" } else { "empty" });
        self.browser.refresh(
            effective_tracks,
            &effective_config,
            &youtube,
            &BrowserRenderContext {
                history: &self.listening_history.borrow(),
                playback: &playback,
                offline: &self.offline_store.borrow(),
            },
            &query,
        );
        self.browser
            .restore_home_scroll_positions(home_scroll_positions);
        if !youtube_only {
            if let Some(current) = state.current {
                self.browser.select_track(current);
            }
        }
    }

    pub(crate) fn navigate_browser(&self, route: BrowserRoute) {
        if matches!(&route, BrowserRoute::Artists) {
            self.prefetch_home_artist_profiles(true);
        }
        let playback = self.browser_playback_state();
        let state = self.state.borrow();
        let config = self.config.borrow();
        let youtube = self.youtube_library.borrow();
        let query = self.search_query.borrow();
        let youtube_only = config.startup_source == Some(StartupSource::YouTube);
        let effective_tracks: &[Track] = if youtube_only {
            &[]
        } else {
            state.tracks.as_slice()
        };
        let mut effective_config = config.clone();
        if youtube_only {
            effective_config.playlists.clear();
        }
        self.browser.navigate(
            route.clone(),
            effective_tracks,
            &effective_config,
            &youtube,
            &BrowserRenderContext {
                history: &self.listening_history.borrow(),
                playback: &playback,
                offline: &self.offline_store.borrow(),
            },
            &query,
        );
        drop(query);
        drop(youtube);
        drop(config);
        drop(state);
        self.update_sidebar_active(&route);
        self.apply_footer_mode();
    }

    pub(crate) fn update_sidebar_active(&self, route: &BrowserRoute) {
        for button in [
            &self.sidebar_all,
            &self.sidebar_albums,
            &self.sidebar_artists,
            &self.sidebar_playlists,
            &self.sidebar_liked,
        ] {
            button.remove_css_class("active");
        }
        match route {
            BrowserRoute::All => self.sidebar_all.add_css_class("active"),
            BrowserRoute::Albums | BrowserRoute::Album(_) | BrowserRoute::YouTubeAlbum(_) => {
                self.sidebar_albums.add_css_class("active")
            }
            BrowserRoute::Artists | BrowserRoute::Artist(_) | BrowserRoute::YouTubeArtist(_) => {
                self.sidebar_artists.add_css_class("active")
            }
            BrowserRoute::Playlists
            | BrowserRoute::Playlist(_)
            | BrowserRoute::YouTubePlaylist { .. } => {
                self.sidebar_playlists.add_css_class("active")
            }
            BrowserRoute::Liked => self.sidebar_liked.add_css_class("active"),
        }
    }

    pub(crate) fn update_listening_history_context_from_route(&self) {
        let context = match self.browser.route() {
            BrowserRoute::Album(title) => listening_history::PlaybackHistoryContext {
                kind: "album".to_string(),
                id: title.to_lowercase(),
                title,
            },
            BrowserRoute::Playlist(title) => listening_history::PlaybackHistoryContext {
                kind: "playlist".to_string(),
                id: title.to_lowercase(),
                title,
            },
            BrowserRoute::YouTubeAlbum(collection) => listening_history::PlaybackHistoryContext {
                kind: "album".to_string(),
                id: collection.key,
                title: collection.title,
            },
            BrowserRoute::YouTubePlaylist { title, browse_id } => {
                listening_history::PlaybackHistoryContext {
                    kind: "playlist".to_string(),
                    id: if browse_id.is_empty() {
                        title.to_lowercase()
                    } else {
                        browse_id
                    },
                    title,
                }
            }
            _ => listening_history::PlaybackHistoryContext::default(),
        };
        self.listening_history_context.replace(context);
    }

    pub(crate) fn handle_browser_events(&self) {
        while let Some(event) = self.browser.try_recv() {
            match event {
                BrowserEvent::RefreshSearch => self.refresh_browser(),
                BrowserEvent::TrackActivated(index) => {
                    self.update_listening_history_context_from_route();
                    self.pending_resume_position_us.set(None);
                    self.prepare_playback_queue(index);
                    self.select_track(index, true);
                }
                BrowserEvent::ResumeLocalTrack {
                    index,
                    position_seconds,
                } => {
                    self.prepare_playback_queue(index);
                    self.select_track(index, true);
                    self.pending_resume_position_us.set(Some(
                        position_seconds
                            .saturating_mul(1_000_000)
                            .min(i64::MAX as u64) as i64,
                    ));
                }
                BrowserEvent::ResumeYouTubeTrack {
                    item,
                    position_seconds,
                } => {
                    self.pending_resume_position_us.set(Some(
                        position_seconds
                            .saturating_mul(1_000_000)
                            .min(i64::MAX as u64) as i64,
                    ));
                    self.resolve_youtube_track(item.clone(), vec![item], 0, false);
                }
                BrowserEvent::YouTubeTrackActivated { item, queue, index } => {
                    self.update_listening_history_context_from_route();
                    self.pending_resume_position_us.set(None);
                    self.resolve_youtube_track(item, queue, index, false);
                }
                BrowserEvent::QueueLocalPlayNext(index) => {
                    self.enqueue_local_track(index, true);
                }
                BrowserEvent::QueueLocalAppend(index) => {
                    self.enqueue_local_track(index, false);
                }
                BrowserEvent::QueueYouTubePlayNext(item) => {
                    self.enqueue_youtube_track(&item, true);
                }
                BrowserEvent::QueueYouTubeAppend(item) => {
                    self.enqueue_youtube_track(&item, false);
                }
                BrowserEvent::ToggleLocalTrackFavorite(index) => {
                    let path = self
                        .state
                        .borrow()
                        .tracks
                        .get(index)
                        .map(|track| track.path.clone());
                    if let Some(path) = path {
                        let liked = self.config.borrow_mut().toggle_liked(&path);
                        self.save_config();
                        if self.current_track_path().as_deref() == Some(path.as_path()) {
                            self.update_favorite_icon(&path);
                        }
                        self.refresh_browser();
                        self.show_toast(if liked {
                            self.tr(Message::AddedLiked)
                        } else {
                            self.tr(Message::RemovedLiked)
                        });
                    }
                }
                BrowserEvent::ToggleYouTubeTrackFavorite(item) => {
                    self.toggle_youtube_item_favorite(item);
                }
                BrowserEvent::DownloadYouTubeCollection { item, playlist } => {
                    self.download_youtube_collection(item, playlist);
                }
                BrowserEvent::QueueLocalCollection {
                    kind,
                    title,
                    play_next,
                } => {
                    self.enqueue_local_collection(&kind, &title, play_next);
                }
                BrowserEvent::QueueYouTubeCollection {
                    item,
                    playlist,
                    play_next,
                } => {
                    self.enqueue_youtube_collection(&item, playlist, play_next);
                }
                BrowserEvent::TogglePlayback => {
                    self.toggle_playback();
                }
                BrowserEvent::PlayLocalAlbum(title) => {
                    self.play_local_collection("album", &title);
                }
                BrowserEvent::PlayLocalPlaylist(title) => {
                    self.play_local_collection("playlist", &title);
                }
                BrowserEvent::PlayLocalMix { title, indices } => {
                    if let Some(first) = indices.first().copied() {
                        let artist = self
                            .state
                            .borrow()
                            .tracks
                            .get(first)
                            .map(|track| track.artist.clone())
                            .unwrap_or_default();

                        self.listening_history_context.replace(
                            listening_history::PlaybackHistoryContext {
                                kind: "mix".to_string(),
                                id: artist,
                                title,
                            },
                        );
                        self.pending_resume_position_us.set(None);
                        self.state.borrow_mut().playback_queue = indices;
                        self.select_track(first, true);
                    }
                }
                BrowserEvent::PlayYouTubeAlbum(item) => {
                    self.play_youtube_collection(item, false);
                }
                BrowserEvent::PlayYouTubePlaylist(item) => {
                    self.play_youtube_collection(item, true);
                }
                BrowserEvent::OpenYouTubePlaylist(item) => {
                    self.load_youtube_playlist_for_browser(item);
                }
                BrowserEvent::OpenYouTubeCollection(item) => {
                    self.load_youtube_collection_for_browser(item);
                }
                BrowserEvent::LoadMoreAlbums => {
                    self.browser.show_more_albums();
                    self.refresh_browser();
                }
                BrowserEvent::LoadMoreArtists => {
                    self.browser.show_more_artists();
                    self.prefetch_home_artist_profiles(true);
                    self.refresh_browser();
                }
                BrowserEvent::Navigate(route) => self.navigate_browser(route),
                BrowserEvent::CreatePlaylist(name) => {
                    let created = self.config.borrow_mut().create_playlist(&name);
                    if created {
                        self.save_config();
                        self.refresh_browser();
                        self.show_toast(&format!("Playlist ‘{name}’ criada"));
                    } else {
                        self.show_toast("Use um nome novo para a playlist");
                    }
                }
                BrowserEvent::AddCurrentToPlaylist(name) => {
                    let Some(path) = self.current_track_path() else {
                        self.show_toast("Selecione uma faixa primeiro");
                        continue;
                    };
                    let added = self.config.borrow_mut().add_to_playlist(&name, &path);
                    if added {
                        self.save_config();
                        self.refresh_browser();
                        self.show_toast(&format!("Faixa adicionada a ‘{name}’"));
                    } else {
                        self.show_toast("A faixa já está nessa playlist");
                    }
                }
                BrowserEvent::RemoveCurrentFromPlaylist(name) => {
                    let Some(path) = self.current_track_path() else {
                        self.show_toast("Selecione uma faixa primeiro");
                        continue;
                    };
                    let removed = self.config.borrow_mut().remove_from_playlist(&name, &path);
                    if removed {
                        self.save_config();
                        self.refresh_browser();
                        self.show_toast(&format!("Faixa removida de ‘{name}’"));
                    } else {
                        self.show_toast("A faixa não está nessa playlist");
                    }
                }
                BrowserEvent::DeletePlaylist(name) => {
                    if self.config.borrow_mut().delete_playlist(&name) {
                        self.save_config();
                        self.navigate_browser(BrowserRoute::Playlists);
                        self.show_toast(&format!("Playlist ‘{name}’ excluída"));
                    }
                }
                BrowserEvent::ToggleCollectionFavorite(key) => {
                    let added = self.config.borrow_mut().toggle_collection_favorite(&key);
                    self.save_config();
                    self.refresh_browser();
                    self.show_toast(if added {
                        "Coleção adicionada aos favoritos"
                    } else {
                        "Coleção removida dos favoritos"
                    });
                }
            }
        }
    }

    pub(crate) fn play_local_collection(&self, kind: &str, title: &str) {
        let mut indices = if kind == "playlist" {
            let paths = self
                .config
                .borrow()
                .playlist(title)
                .map(|playlist| playlist.tracks.clone())
                .unwrap_or_default();
            let state = self.state.borrow();
            paths
                .iter()
                .filter_map(|path| state.tracks.iter().position(|track| &track.path == path))
                .collect::<Vec<_>>()
        } else {
            let state = self.state.borrow();
            state
                .tracks
                .iter()
                .enumerate()
                .filter_map(|(index, track)| {
                    track.album.eq_ignore_ascii_case(title).then_some(index)
                })
                .collect::<Vec<_>>()
        };

        if kind == "album" {
            let state = self.state.borrow();
            indices.sort_by(|left, right| {
                let left = &state.tracks[*left];
                let right = &state.tracks[*right];
                left.disc_number
                    .unwrap_or(u32::MAX)
                    .cmp(&right.disc_number.unwrap_or(u32::MAX))
                    .then_with(|| {
                        left.track_number
                            .unwrap_or(u32::MAX)
                            .cmp(&right.track_number.unwrap_or(u32::MAX))
                    })
                    .then_with(|| left.title.to_lowercase().cmp(&right.title.to_lowercase()))
            });
        }

        let Some(first) = indices.first().copied() else {
            self.show_toast(if kind == "playlist" {
                "Esta playlist local ainda está vazia"
            } else {
                "Nenhuma faixa local foi encontrada para este álbum"
            });
            return;
        };

        self.listening_history_context
            .replace(listening_history::PlaybackHistoryContext {
                kind: kind.to_string(),
                id: title.to_lowercase(),
                title: title.to_string(),
            });
        self.pending_resume_position_us.set(None);
        self.state.borrow_mut().playback_queue = indices.clone();
        self.sync_local_queue_v2(&indices, first);
        self.select_track(first, true);
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

    pub(crate) fn request_lyrics(&self, index: usize, notify: bool, force: bool) {
        let (path, lookup) = {
            let state = self.state.borrow();
            let Some(track) = state.tracks.get(index) else {
                return;
            };
            if !force && !track.lyrics.is_empty() {
                return;
            }
            (
                track.path.clone(),
                lyrics_domain::provider::LyricsLookup {
                    title: track.title.clone(),
                    artist: track.artist.clone(),
                    album: track.album.clone(),
                    duration_seconds: track.duration_seconds,
                },
            )
        };

        if !self.lyrics_pending.borrow_mut().insert(path.clone()) {
            if notify {
                self.show_toast("As letras já estão sendo buscadas");
            }
            return;
        }

        if notify {
            self.show_toast("Buscando letras sincronizadas...");
        }
        let sender = self.background.sender();
        thread::spawn(move || {
            let result = lyrics_domain::provider::download_to_sidecar(&path, &lookup, force).map(
                |document| {
                    eprintln!(
                        "Lyrics loaded from {} ({})",
                        document.provider,
                        if document.synchronized {
                            "synchronized"
                        } else {
                            "plain fallback"
                        }
                    );
                },
            );
            let _ = sender.send(BackgroundMessage::LyricsDownloaded {
                path,
                result,
                notify,
            });
        });
    }

    pub(crate) fn refresh_current_lyrics(&self) {
        match self.playback_source.get() {
            PlaybackSource::Local => {
                let current = self.state.borrow().current;
                let Some(index) = current else {
                    self.show_toast("Selecione uma faixa primeiro");
                    return;
                };
                self.request_lyrics(index, true, true);
            }
            PlaybackSource::YouTube => {
                let item = self
                    .youtube_state
                    .borrow()
                    .as_ref()
                    .map(|state| state.item.clone());
                let Some(item) = item else {
                    self.show_toast("Selecione uma faixa primeiro");
                    return;
                };

                self.set_lyrics_message("Buscando novamente as letras sincronizadas…");
                self.show_toast("Buscando letras sincronizadas…");
                self.request_youtube_lyrics(&item, true);
            }
            PlaybackSource::None => {
                self.show_toast("Selecione uma faixa primeiro");
            }
        }
    }
}

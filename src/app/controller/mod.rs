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

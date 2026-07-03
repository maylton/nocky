//! Construction helpers for `AppController`.

use super::{AppController, ControllerRuntime};
use crate::{
    app::{
        sidebar::build_sidebar,
        state::{AppState, PlaybackSource},
    },
    background::BackgroundChannel,
    browser::{BrowserRoute, LibraryBrowser},
    config::{self, AppLanguage, StartupSource, VisualTheme},
    i18n::{self, Message},
    listening_history::{self, ListeningHistory},
    offline_store::OfflineStore,
    playback::{
        queue::{QueueSourceKind, ShuffleNavigator},
        transition::TransitionClock,
        PlaybackEngine,
    },
    reveal_bounce::RevealBounce,
    search_history::SearchHistory,
    theme,
    ui::{
        footer::{build_footer_view, FooterViewParts, FOOTER_ARTWORK_SOURCE_SIZE},
        player::PlayerView,
        settings::SettingsPage,
        widgets::{
            build_cover,
            material_button::{
                apply_material_button, apply_material_icon_button,
                set_material_icon_button_selected, MaterialButtonSemantic, MaterialButtonSize,
                MaterialButtonSpec, MaterialButtonVariant, MaterialIconButtonSpec,
                MaterialIconButtonVariant,
            },
            AnimatedPageSwitcher, TopPage,
        },
    },
    visual_theme,
    youtube::{
        diagnostics as youtube_diagnostics, load_library_cache, LikeMutationRegistry,
        YouTubeBridge, YouTubeHomePage, YouTubePage, YouTubeSearchCache, YouTubeSearchResults,
    },
    APP_ID, HOME_PLAYER_WIDTH,
};
use adw::prelude::*;
use gtk::glib;
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub(crate) fn build_application(app: &adw::Application) {
    youtube_diagnostics::start_background_checks();
    let controller = AppController::new(app);
    controller.setup_callbacks();
    controller.install_actions(app);
    controller.load_saved_library();
    controller.window.present();

    let startup_controller = controller.clone();
    glib::idle_add_local_once(move || {
        startup_controller.apply_startup_source();
        startup_controller.try_restore_playback_session();
    });

    // Keep the controller alive for as long as the application is running.
    let keep_alive = controller.clone();
    app.connect_shutdown(move |_| {
        // The regular checkpoints are asynchronous; shutdown performs one
        // serialized final snapshot so the latest playback session is kept.
        keep_alive.listening_history.borrow().flush();
        keep_alive.persist_queue_now();
        keep_alive.persist_playback_session_now();
        keep_alive.player.shutdown();
        keep_alive
            .mpris
            .send(crate::playback::mpris::MprisUpdate::Shutdown);
    });
}

impl AppController {
    pub(crate) fn new(app: &adw::Application) -> Rc<Self> {
        let theme = theme::ThemeBridge::install();
        let visual_theme_manager = visual_theme::VisualThemeManager::install();
        let config = config::AppConfig::load();
        let tr = |message: Message| i18n::text(config.language, message);
        theme.set_noctalia_enabled(
            config.visual_theme == VisualTheme::Noctalia
                && config.noctalia_theme_sync
                && theme.noctalia_shell_detected(),
        );
        theme.set_blur_preferences(config.blur_mode, config.blur_opacity);
        let player = PlaybackEngine::new(config.volume.clamp(0.0, 1.0))
            .unwrap_or_else(|error| panic!("Nocky playback initialization failed: {error}"));
        let background = BackgroundChannel::new();

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Nocky")
            .default_width(1080)
            .default_height(720)
            .width_request(680)
            .height_request(520)
            .build();
        window.set_icon_name(Some(APP_ID));
        window.add_css_class("noctalia-window");

        let toast_overlay = adw::ToastOverlay::new();
        window.set_content(Some(&toast_overlay));

        let shell = gtk::Box::new(gtk::Orientation::Vertical, 0);
        shell.add_css_class("app-shell");
        toast_overlay.set_child(Some(&shell));

        let views = adw::ViewStack::new();
        views.set_vexpand(true);
        views.set_hexpand(true);

        let header = adw::HeaderBar::new();
        header.add_css_class("noctalia-header");
        header.add_css_class("expressive-header");

        let sidebar_button = gtk::ToggleButton::builder()
            .icon_name("sidebar-show-symbolic")
            .active(false)
            .tooltip_text(tr(Message::SidebarToggle))
            .build();
        apply_material_icon_button(
            &sidebar_button,
            MaterialIconButtonSpec::new(MaterialIconButtonVariant::Standard),
        );
        sidebar_button.add_css_class("header-navigation-button");
        header.pack_start(&sidebar_button);

        let brand = gtk::Label::new(Some("NOCKY"));
        brand.add_css_class("brand-title");
        brand.add_css_class("header-brand");
        header.pack_start(&brand);

        let player_toggle_icon = gtk::Image::from_icon_name("view-grid-symbolic");
        player_toggle_icon.set_pixel_size(18);
        let player_toggle_button = gtk::Button::new();
        player_toggle_button.set_child(Some(&player_toggle_icon));
        apply_material_icon_button(
            &player_toggle_button,
            MaterialIconButtonSpec::new(MaterialIconButtonVariant::Standard),
        );
        player_toggle_button.add_css_class("header-action-button");
        player_toggle_button.add_css_class("home-player-toggle-button");
        header.pack_start(&player_toggle_button);

        let queue_tab_text = match config.language {
            AppLanguage::Portuguese => "Fila",
            AppLanguage::English => "Queue",
            AppLanguage::Spanish => "Cola",
        };
        let page_switcher = AnimatedPageSwitcher::new(
            tr(Message::MusicTab),
            tr(Message::LyricsTab),
            queue_tab_text,
        );
        header.set_title_widget(Some(page_switcher.root()));

        let search_button = gtk::ToggleButton::builder()
            .icon_name("system-search-symbolic")
            .tooltip_text(tr(Message::SearchLibrary))
            .build();
        apply_material_icon_button(
            &search_button,
            MaterialIconButtonSpec::new(MaterialIconButtonVariant::Standard),
        );
        search_button.add_css_class("header-action-button");
        header.pack_end(&search_button);

        let sync_button = gtk::Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text("Sincronizar biblioteca")
            .build();
        apply_material_icon_button(
            &sync_button,
            MaterialIconButtonSpec::new(MaterialIconButtonVariant::Standard),
        );
        sync_button.add_css_class("header-action-button");
        header.pack_end(&sync_button);

        let folder_button = gtk::Button::builder()
            .icon_name("folder-open-symbolic")
            .tooltip_text(tr(Message::ChooseMusicFolderTooltip))
            .build();
        apply_material_icon_button(
            &folder_button,
            MaterialIconButtonSpec::new(MaterialIconButtonVariant::Standard),
        );
        folder_button.add_css_class("header-action-button");
        header.pack_end(&folder_button);

        let settings_button = gtk::ToggleButton::builder()
            .icon_name("preferences-system-symbolic")
            .tooltip_text(tr(Message::SettingsTitle))
            .build();
        apply_material_icon_button(
            &settings_button,
            MaterialIconButtonSpec::new(MaterialIconButtonVariant::Standard),
        );
        settings_button.add_css_class("header-action-button");
        settings_button.add_css_class("settings-navigation-button");
        header.pack_end(&settings_button);

        shell.append(&header);

        let search_bar = gtk::SearchBar::new();
        search_bar.add_css_class("expressive-search-bar");
        let search_entry = gtk::SearchEntry::builder()
            .placeholder_text(tr(Message::SearchPlaceholder))
            .hexpand(true)
            .build();
        search_entry.add_css_class("expressive-search-entry");
        let search_history_revealer = gtk::Revealer::new();
        search_history_revealer.set_transition_type(gtk::RevealerTransitionType::SlideDown);
        search_history_revealer.set_transition_duration(180);
        search_history_revealer.set_reveal_child(false);
        search_history_revealer.set_hexpand(true);
        search_history_revealer.set_halign(gtk::Align::Fill);
        search_history_revealer.set_margin_start(12);
        search_history_revealer.set_margin_end(12);
        search_history_revealer.add_css_class("search-history-dropdown");

        // Keep the entry and its recent-query surface inside the same SearchBar
        // child. Both now share the exact content width before the close button.
        let search_surface = gtk::Box::new(gtk::Orientation::Vertical, 0);
        search_surface.set_hexpand(true);
        search_surface.add_css_class("search-surface-stack");
        search_surface.append(&search_entry);
        search_surface.append(&search_history_revealer);
        search_bar.set_child(Some(&search_surface));
        search_bar.connect_entry(&search_entry);
        search_bar.set_key_capture_widget(Some(&window));
        search_bar.set_show_close_button(true);
        shell.append(&search_bar);

        let body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        body.set_vexpand(true);
        body.set_hexpand(true);
        body.add_css_class("expressive-body");
        shell.append(&body);

        let sidebar_parts = build_sidebar(config.language);
        sidebar_parts
            .revealer
            .add_css_class("navigation-rail-revealer");
        body.append(&sidebar_parts.revealer);

        let PlayerView {
            handle: player_view,
            root: now_card,
            artist: player_artist,
            album,
            now_heading,
            favorite_button: favorite,
            previous_button: previous,
            hero_play_button,
            next_button: next,
            transport_motion: main_transport_motion,
            inline_lyrics_button,
            refresh_lyrics_button,
            hero_cover,
            hero_play_icon,
            favorite_icon,
            progress,
            home_progress_stack,
            home_wave_progress,
            elapsed,
            duration,
            repeat_button: repeat,
            shuffle_button: shuffle,
            visualizer,
            lyrics,
        } = PlayerView::new(
            config.language,
            config.expressive_transport_effects && config.visual_theme.is_expressive(),
        );

        // A viewport is a hard width constraint; size-request alone is only
        // a minimum and long local metadata can otherwise widen the card.
        let player_viewport = gtk::ScrolledWindow::new();
        player_viewport.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Never);
        player_viewport.set_propagate_natural_width(false);
        player_viewport.set_propagate_natural_height(true);
        player_viewport.set_min_content_width(HOME_PLAYER_WIDTH);
        player_viewport.set_max_content_width(HOME_PLAYER_WIDTH);
        player_viewport.set_size_request(HOME_PLAYER_WIDTH, -1);
        player_viewport.set_hexpand(false);
        player_viewport.set_halign(gtk::Align::Start);
        player_viewport.set_child(Some(&now_card));
        player_viewport.add_css_class("home-player-viewport");

        let player_revealer = gtk::Revealer::new();
        player_revealer.set_transition_type(gtk::RevealerTransitionType::SlideLeft);
        player_revealer.set_transition_duration(220);
        player_revealer.set_reveal_child(!config.home_player_collapsed);
        player_revealer.set_hexpand(false);
        player_revealer.set_halign(gtk::Align::Start);
        let player_motion = gtk::Fixed::new();
        player_motion.set_size_request(HOME_PLAYER_WIDTH, -1);
        player_motion.set_hexpand(false);
        player_motion.put(&player_viewport, 0.0, 0.0);
        player_revealer.set_child(Some(&player_motion));
        player_revealer.add_css_class("home-player-revealer");

        let browser = LibraryBrowser::new();

        let dashboard = gtk::Box::new(gtk::Orientation::Horizontal, 22);
        dashboard.set_margin_top(22);
        dashboard.set_margin_bottom(22);
        dashboard.set_margin_start(24);
        dashboard.set_margin_end(24);
        dashboard.set_vexpand(true);
        dashboard.set_valign(gtk::Align::Fill);
        dashboard.add_css_class("expressive-dashboard");
        dashboard.append(&player_revealer);
        dashboard.append(browser.root());

        let empty_state = gtk::Box::new(gtk::Orientation::Vertical, 12);
        empty_state.set_halign(gtk::Align::Center);
        empty_state.set_valign(gtk::Align::Center);
        empty_state.set_vexpand(true);
        empty_state.add_css_class("expressive-empty-state");
        let empty_icon = gtk::Image::from_icon_name("folder-music-symbolic");
        empty_icon.set_pixel_size(64);
        empty_icon.add_css_class("empty-icon");
        let empty_title = gtk::Label::new(Some(tr(Message::EmptyLibraryTitle)));
        empty_title.add_css_class("title-2");
        let empty_text = gtk::Label::new(Some(tr(Message::EmptyLibraryDescription)));
        empty_text.set_wrap(true);
        empty_text.set_justify(gtk::Justification::Center);
        empty_text.add_css_class("dim-label");
        let empty_add = gtk::Button::with_label(tr(Message::ChooseFolderAction));
        apply_material_button(
            &empty_add,
            MaterialButtonSpec::new(MaterialButtonVariant::Filled, MaterialButtonSize::Standard),
        );
        empty_add.add_css_class("expressive-empty-action");
        empty_state.append(&empty_icon);
        empty_state.append(&empty_title);
        empty_state.append(&empty_text);
        empty_state.append(&empty_add);

        let music_stack = gtk::Stack::new();
        music_stack.set_transition_type(gtk::StackTransitionType::Crossfade);
        music_stack.set_transition_duration(180);
        music_stack.add_named(&empty_state, Some("empty"));
        music_stack.add_named(&dashboard, Some("library"));
        music_stack.set_visible_child_name("empty");

        let music_page = views.add_titled_with_icon(
            &music_stack,
            Some("music"),
            tr(Message::MusicTab),
            "folder-music-symbolic",
        );
        let lyrics_page = views.add_titled_with_icon(
            lyrics.full_widget(),
            Some("lyrics"),
            tr(Message::LyricsTab),
            "audio-input-microphone-symbolic",
        );

        // Reuse the popup renderer so both surfaces share rows and actions.
        let queue_page_root = gtk::Box::new(gtk::Orientation::Vertical, 14);
        queue_page_root.set_margin_top(20);
        queue_page_root.set_margin_bottom(20);
        queue_page_root.set_margin_start(24);
        queue_page_root.set_margin_end(24);
        queue_page_root.set_vexpand(true);
        queue_page_root.set_hexpand(true);
        queue_page_root.add_css_class("queue2-page");

        let queue_page_header = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        queue_page_header.add_css_class("queue2-page-header");

        let queue_page_titles = gtk::Box::new(gtk::Orientation::Vertical, 6);
        queue_page_titles.set_hexpand(true);

        let queue_page_title_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        queue_page_title_row.add_css_class("queue2-page-title-row");

        let queue_page_icon = gtk::Image::from_icon_name("view-list-symbolic");
        queue_page_icon.set_pixel_size(18);
        queue_page_icon.add_css_class("queue2-page-title-icon");

        let queue_page_title = gtk::Label::new(Some(queue_tab_text));
        queue_page_title.set_xalign(0.0);
        queue_page_title.add_css_class("title-1");
        queue_page_title.add_css_class("queue2-page-title");

        let queue_page_summary = gtk::Label::new(None);
        queue_page_summary.set_visible(false);

        let queue_page_summary_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        queue_page_summary_row.add_css_class("queue2-page-summary-row");
        queue_page_summary_row.set_halign(gtk::Align::Start);

        let queue_page_source = gtk::Label::new(None);
        queue_page_source.set_xalign(0.0);
        queue_page_source.add_css_class("dim-label");
        queue_page_source.add_css_class("queue2-page-source");

        let queue_page_upcoming_badge = gtk::Label::new(None);
        queue_page_upcoming_badge.add_css_class("queue2-page-micro-badge");

        let queue_page_total_badge = gtk::Label::new(None);
        queue_page_total_badge.add_css_class("queue2-page-micro-badge");

        queue_page_title_row.append(&queue_page_icon);
        queue_page_title_row.append(&queue_page_title);
        queue_page_summary_row.append(&queue_page_source);
        queue_page_summary_row.append(&queue_page_upcoming_badge);
        queue_page_summary_row.append(&queue_page_total_badge);
        queue_page_titles.append(&queue_page_title_row);
        queue_page_titles.append(&queue_page_summary_row);
        queue_page_header.append(&queue_page_titles);

        let queue_page_actions = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        queue_page_actions.set_valign(gtk::Align::Center);
        queue_page_actions.set_halign(gtk::Align::End);
        queue_page_actions.add_css_class("queue2-page-actions");

        let queue_page_clear_upcoming = gtk::Button::with_label(match config.language {
            AppLanguage::Portuguese => "Limpar próximas",
            AppLanguage::English => "Clear upcoming",
            AppLanguage::Spanish => "Limpiar siguientes",
        });
        apply_material_button(
            &queue_page_clear_upcoming,
            MaterialButtonSpec::new(MaterialButtonVariant::Outlined, MaterialButtonSize::Compact),
        );

        let queue_page_clear_all = gtk::Button::with_label(match config.language {
            AppLanguage::Portuguese => "Limpar tudo",
            AppLanguage::English => "Clear all",
            AppLanguage::Spanish => "Limpiar todo",
        });
        apply_material_button(
            &queue_page_clear_all,
            MaterialButtonSpec::new(MaterialButtonVariant::Outlined, MaterialButtonSize::Compact)
                .with_semantic(MaterialButtonSemantic::Destructive),
        );

        queue_page_actions.append(&queue_page_clear_upcoming);
        queue_page_actions.append(&queue_page_clear_all);
        queue_page_header.append(&queue_page_actions);
        queue_page_root.append(&queue_page_header);

        let queue_page_list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        queue_page_list.add_css_class("queue2-list");
        queue_page_list.add_css_class("queue2-page-list");

        let queue_page_scroll = gtk::ScrolledWindow::new();
        queue_page_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        queue_page_scroll.set_vexpand(true);
        queue_page_scroll.set_hexpand(true);
        queue_page_scroll.set_child(Some(&queue_page_list));
        queue_page_scroll.add_css_class("queue2-page-scroll");
        queue_page_root.append(&queue_page_scroll);

        let queue_page_popover_proxy = gtk::Popover::new();

        views.add_titled_with_icon(
            &queue_page_root,
            Some("queue"),
            queue_tab_text,
            "view-list-symbolic",
        );

        let youtube_page = YouTubePage::new();
        let settings_page = SettingsPage::new(&config, theme.noctalia_shell_detected());

        let content_stack = gtk::Stack::new();
        content_stack.set_transition_type(gtk::StackTransitionType::Crossfade);
        content_stack.set_transition_duration(180);
        content_stack.set_vexpand(true);
        content_stack.set_hexpand(true);
        content_stack.add_named(&views, Some("main"));
        content_stack.add_named(settings_page.root(), Some("settings"));
        content_stack.set_visible_child_name("main");
        content_stack.add_css_class("application-content-stack");
        body.append(&content_stack);

        let mini_cover = build_cover(FOOTER_ARTWORK_SOURCE_SIZE);
        let FooterViewParts {
            root: player_bar,
            now_playing_button: footer_now_playing,
            title: mini_title,
            artist: mini_artist,
            source: footer_source,
            favorite_button: footer_favorite,
            favorite_icon: footer_favorite_icon,
            center: footer_center,
            progress_stack: footer_progress_stack,
            traditional_progress: footer_traditional_progress,
            wave_progress: footer_progress,
            elapsed: footer_elapsed,
            duration: footer_duration,
            previous: footer_previous,
            play_button: play,
            play_icon,
            transport_motion: footer_transport_motion,
            next: footer_next,
            repeat: footer_repeat,
            shuffle: footer_shuffle,
            right_controls,
            lyrics_button,
            mute_icon,
            mute_button,
            volume,
            volume_revealer,
        } = build_footer_view(
            config.language,
            config.volume,
            config.expressive_transport_effects && config.visual_theme.is_expressive(),
            &mini_cover.stack,
        );

        {
            let group = right_controls.clone();
            volume_revealer.connect_child_revealed_notify(move |revealer| {
                let reveal_child = revealer.property::<bool>("reveal-child");
                let child_revealed = revealer.property::<bool>("child-revealed");

                if !reveal_child && !child_revealed {
                    revealer.set_visible(false);
                    group.set_size_request(-1, 52);
                    group.queue_allocate();
                }
            });
        }

        shell.append(&player_bar);

        let mpris = crate::playback::mpris::MprisBridge::start(config.volume);
        let youtube_bridge = YouTubeBridge::discover().ok().map(Arc::new);

        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos() as u64)
            .unwrap_or(0x9e37_79b9_7f4a_7c15);

        let initial_queue_source = match config.startup_source {
            Some(StartupSource::YouTube) => QueueSourceKind::YouTube,
            Some(StartupSource::Local) | None => QueueSourceKind::Local,
        };
        let queue_load = crate::playback::queue::load_for(initial_queue_source);
        if queue_load.discarded_entries > 0 {
            eprintln!(
                "Queue 2.0 recovery discarded {} unavailable entr{}",
                queue_load.discarded_entries,
                if queue_load.discarded_entries == 1 {
                    "y"
                } else {
                    "ies"
                }
            );
        }
        let restored_queue = queue_load.queue;
        let restored_queue_snapshot = restored_queue.snapshot();
        let restored_playback_session = crate::playback::session::load_for(initial_queue_source);

        let initial_volume = config.volume.clamp(0.15, 1.0);
        let mut listening_history = ListeningHistory::load();
        listening_history.set_recording_enabled(config.collect_listening_history);
        let sidebar_bounce = RevealBounce::new(false);
        let player_bounce = RevealBounce::new(!config.home_player_collapsed);
        let controller = Rc::new(Self {
            window,
            toast_overlay,
            player,
            runtime: ControllerRuntime {
                state: RefCell::new(AppState::default()),
                playback_queue_v2: RefCell::new(restored_queue),
                active_queue_source: Cell::new(initial_queue_source),
                queue_last_saved_snapshot: RefCell::new(restored_queue_snapshot),
                queue_dragged_entry: Cell::new(None),
                queue_v2_pending_entry: Cell::new(None),
                config: RefCell::new(config),
                listening_history: RefCell::new(listening_history),
                listening_session_id: RefCell::new(None),
                listening_session_last_saved_seconds: Cell::new(0),
                listening_history_context: RefCell::new(
                    listening_history::PlaybackHistoryContext::default(),
                ),
                pending_resume_position_us: Cell::new(None),
                restored_playback_session: RefCell::new(restored_playback_session),
                startup_restore_autoplay: Cell::new(None),
                playback_session_last_position_seconds: Cell::new(0),
                playback_session_last_shuffle: Cell::new(false),
                playback_session_last_repeat: Cell::new(false),
                playback_session_restore_attempts: Cell::new(0),
                updating_progress: Cell::new(false),
                scanning: Cell::new(false),
                shuffle_enabled: Cell::new(false),
                shuffle_navigation: RefCell::new(ShuffleNavigator::default()),
                rng_state: Cell::new(seed),
                search_query: RefCell::new(String::new()),
                search_history: RefCell::new(SearchHistory::load()),
                lyrics_pending: RefCell::new(HashSet::new()),
                background,
                mpris,
                last_mpris_position: Cell::new(-1),
                playback_source: Cell::new(PlaybackSource::None),
                youtube_state: RefCell::new(None),
                youtube_request_id: Cell::new(0),
                youtube_search_request_id: Cell::new(0),
                youtube_search_cache: RefCell::new(YouTubeSearchCache::default()),
                youtube_home_request_id: Cell::new(0),
                youtube_home_loading: Cell::new(false),
                youtube_home_continuation_loading: Cell::new(false),
                youtube_home_previous_params: RefCell::new(String::new()),
                youtube_recovery_in_progress: Cell::new(false),
                youtube_recovery_attempted: Cell::new(false),
                youtube_recovery_retry_count: Cell::new(0),
                youtube_recovery_generation: Cell::new(0),
                youtube_recovery_resume_us: Cell::new(0),
                youtube_recovery_was_playing: Cell::new(false),
                youtube_playlist_request_id: Cell::new(0),
                youtube_collection_play_request_id: Cell::new(0),
                youtube_collection_queue_request_id: Cell::new(0),
                youtube_collection_prefetching: Cell::new(false),
                youtube_playlist_loading: Cell::new(false),
                youtube_playlist_prefetching: Cell::new(false),
                youtube_pending_playlist: RefCell::new(None),
                youtube_playlist_revalidation: RefCell::new(HashMap::new()),
                youtube_cache_first_cleanup: RefCell::new(None),
                youtube_bridge,
                youtube_home_page: RefCell::new(YouTubeHomePage::default()),
                youtube_library: RefCell::new(load_library_cache()),
                offline_store: RefCell::new(OfflineStore::load_default()),
                offline_download_pending: RefCell::new(HashSet::new()),
                youtube_like_request_id: Cell::new(0),
                youtube_like_pending: RefCell::new(HashMap::new()),
                youtube_like_mutations: RefCell::new(LikeMutationRegistry::default()),
                youtube_playlist_create_pending: Cell::new(false),
            },
            sidebar_motion: sidebar_parts.motion,
            sidebar_content: sidebar_parts.content,
            sidebar_bounce: sidebar_bounce.clone(),
            sidebar: sidebar_parts.revealer,
            sidebar_button: sidebar_button.clone(),
            sidebar_all: sidebar_parts.all_button,
            sidebar_all_label: sidebar_parts.all_label,
            sidebar_albums: sidebar_parts.albums_button,
            sidebar_albums_label: sidebar_parts.albums_label,
            sidebar_artists: sidebar_parts.artists_button,
            sidebar_artists_label: sidebar_parts.artists_label,
            sidebar_playlists: sidebar_parts.playlists_button,
            sidebar_playlists_label: sidebar_parts.playlists_label,
            sidebar_liked: sidebar_parts.liked_button,
            sidebar_liked_label: sidebar_parts.liked_label,
            sidebar_section_label: sidebar_parts.section_label,
            search_button: search_button.clone(),
            folder_button: folder_button.clone(),
            search_entry: search_entry.clone(),
            search_history_revealer: search_history_revealer.clone(),
            settings_button: settings_button.clone(),
            content_stack: content_stack.clone(),
            settings_page: settings_page.clone(),
            views,
            music_page,
            lyrics_page,
            queue_page_list: queue_page_list.clone(),
            queue_page_summary: queue_page_summary.clone(),
            queue_page_source: queue_page_source.clone(),
            queue_page_upcoming_badge: queue_page_upcoming_badge.clone(),
            queue_page_total_badge: queue_page_total_badge.clone(),
            queue_page_clear_upcoming: queue_page_clear_upcoming.clone(),
            queue_page_clear_all: queue_page_clear_all.clone(),
            queue_page_popover_proxy: queue_page_popover_proxy.clone(),
            queue_page_last_snapshot: RefCell::new(None),
            queue_page_last_source: Cell::new(None),
            page_switcher: page_switcher.clone(),
            browser,
            lyrics,
            youtube_page,
            player_view,
            player_revealer: player_revealer.clone(),
            player_motion: player_motion.clone(),
            player_viewport: player_viewport.clone(),
            player_bounce: player_bounce.clone(),
            player_toggle_button: player_toggle_button.clone(),
            player_toggle_icon: player_toggle_icon.clone(),
            player_artist,
            album,
            now_heading,
            favorite_button: favorite.clone(),
            previous_button: previous.clone(),
            hero_play_button: hero_play_button.clone(),
            main_transport_motion: main_transport_motion.clone(),
            next_button: next.clone(),
            mini_title,
            mini_artist,
            footer_source,
            footer_now_playing: footer_now_playing.clone(),
            footer_center,
            footer_right_controls: right_controls,
            volume_revealer: volume_revealer.clone(),
            music_stack,
            empty_title,
            empty_text,
            empty_add: empty_add.clone(),
            hero_cover,
            mini_cover,
            player_bar: player_bar.clone(),
            play_icon,
            hero_play_icon,
            favorite_icon,
            footer_favorite_icon,
            footer_favorite_button: footer_favorite.clone(),
            progress,
            home_progress_stack,
            home_wave_progress,
            elapsed,
            duration,
            footer_progress_stack,
            footer_traditional_progress,
            footer_progress,
            footer_elapsed,
            footer_duration,
            volume,
            mute_icon,
            mute_button: mute_button.clone(),
            volume_before_mute: Cell::new(initial_volume),
            compact_volume_expanded: Cell::new(false),
            compact_volume_spring_generation: Rc::new(Cell::new(0)),
            footer_metadata_transition: TransitionClock::new(),
            lyrics_button,
            footer_previous: footer_previous.clone(),
            footer_play_button: play.clone(),
            footer_transport_motion: footer_transport_motion.clone(),
            footer_next: footer_next.clone(),
            footer_repeat_button: footer_repeat.clone(),
            footer_shuffle_button: footer_shuffle.clone(),
            repeat_button: repeat.clone(),
            shuffle_button: shuffle.clone(),
            visualizer,
            visual_theme_manager,
            _theme: theme,
        });
        {
            let weak = Rc::downgrade(&controller);
            let click = gtk::GestureClick::new();
            click.set_button(1);
            click.connect_released(move |_, presses, _, _| {
                if presses == 1 {
                    if let Some(controller) = weak.upgrade() {
                        controller.open_current_artist_from_player();
                    }
                }
            });
            controller.player_artist.add_controller(click);
        }

        {
            let weak = Rc::downgrade(&controller);
            let click = gtk::GestureClick::new();
            click.set_button(1);
            click.connect_released(move |_, presses, _, _| {
                if presses == 1 {
                    if let Some(controller) = weak.upgrade() {
                        controller.open_current_album_from_player();
                    }
                }
            });
            controller.album.add_controller(click);
        }

        {
            let weak = Rc::downgrade(&controller);
            controller.lyrics.connect_seek(move |timestamp_us| {
                if let Some(controller) = weak.upgrade() {
                    controller.seek_to(timestamp_us, true);
                }
            });
        }

        {
            let weak = Rc::downgrade(&controller);
            page_switcher.connect_home_clicked(move || {
                if let Some(controller) = weak.upgrade() {
                    controller.open_library_home();
                }
            });
        }

        {
            let weak = Rc::downgrade(&controller);
            page_switcher.connect_lyrics_clicked(move || {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                controller.close_settings_page();
                controller.views.set_visible_child_name("lyrics");
                if !controller.lyrics_button.is_active() {
                    controller.lyrics_button.set_active(true);
                }
            });
        }

        {
            let weak = Rc::downgrade(&controller);
            page_switcher.connect_queue_clicked(move || {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                controller.close_settings_page();
                controller.views.set_visible_child_name("queue");
                controller.refresh_queue_page();
            });
        }

        {
            let page_switcher = page_switcher.clone();
            controller
                .views
                .connect_visible_child_name_notify(move |stack| {
                    let page = match stack.visible_child_name().as_deref() {
                        Some("lyrics") => TopPage::Lyrics,
                        Some("queue") => TopPage::Queue,
                        _ => TopPage::Home,
                    };
                    page_switcher.set_active_page(page, true);
                });
        }

        {
            let weak = Rc::downgrade(&controller);
            queue_page_clear_upcoming.connect_clicked(move |_| {
                if let Some(controller) = weak.upgrade() {
                    controller.playback_queue_v2.borrow_mut().clear_upcoming();
                    controller.refresh_queue_page();
                }
            });
        }

        {
            let weak = Rc::downgrade(&controller);
            queue_page_clear_all.connect_clicked(move |_| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                controller.clear_playback_queue();
                controller.refresh_queue_page();
            });
        }

        {
            let weak = Rc::downgrade(&controller);
            glib::timeout_add_local(Duration::from_secs(1), move || {
                let Some(controller) = weak.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                controller.persist_queue_if_changed();
                controller.persist_playback_session_if_changed();
                controller.try_restore_playback_session();
                if controller.views.visible_child_name().as_deref() == Some("queue") {
                    controller.refresh_queue_page();
                }
                glib::ControlFlow::Continue
            });
        }

        controller.apply_translations();
        controller.apply_home_preferences();
        controller.apply_home_player_visibility();
        controller.apply_volume_icon();
        controller.install_footer_adaptive();
        controller.apply_footer_mode();

        controller.sidebar_button.set_active(false);
        controller.sidebar.set_reveal_child(false);
        controller.sidebar.set_visible(false);
        controller.sidebar.add_css_class("sidebar-collapsed");

        {
            let weak = Rc::downgrade(&controller);
            controller
                .views
                .connect_visible_child_name_notify(move |stack| {
                    if stack.visible_child_name().as_deref() == Some("music") {
                        if let Some(controller) = weak.upgrade() {
                            controller.open_library_home();
                        }
                    }
                });
        }

        {
            let weak = Rc::downgrade(&controller);
            controller
                .views
                .connect_visible_child_name_notify(move |_| {
                    if let Some(controller) = weak.upgrade() {
                        controller.apply_footer_mode();
                    }
                });
        }

        {
            let weak = Rc::downgrade(&controller);
            sidebar_button.connect_toggled(move |button| {
                if let Some(controller) = weak.upgrade() {
                    let expanded = button.is_active();
                    set_material_icon_button_selected(button, expanded);
                    controller.sidebar.remove_css_class("sidebar-expanded");
                    controller.sidebar.remove_css_class("sidebar-collapsed");

                    if expanded {
                        controller.sidebar.add_css_class("sidebar-expanded");
                    } else {
                        controller.sidebar.add_css_class("sidebar-collapsed");
                    }

                    controller.sidebar_bounce.set_revealed(
                        &controller.sidebar,
                        &controller.sidebar_motion,
                        &controller.sidebar_content,
                        expanded,
                        true,
                    );
                }
            });
        }

        {
            let weak = Rc::downgrade(&controller);
            player_toggle_button.connect_clicked(move |_| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };

                let collapsed = !controller.config.borrow().home_player_collapsed;
                controller.config.borrow_mut().home_player_collapsed = collapsed;
                controller.save_config();
                controller.apply_home_player_visibility();
                controller.apply_footer_mode();
            });
        }

        {
            let search_bar = search_bar.clone();
            let weak = Rc::downgrade(&controller);
            search_button.connect_toggled(move |button| {
                set_material_icon_button_selected(button, button.is_active());
                search_bar.set_search_mode(button.is_active());
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                if button.is_active() {
                    controller.search_entry.grab_focus();
                    if controller.search_entry.text().trim().is_empty() {
                        controller.refresh_recent_searches(true);
                    }
                } else {
                    controller.search_history_revealer.set_reveal_child(false);
                }
            });
        }

        {
            let search_button = search_button.clone();
            search_bar.connect_search_mode_enabled_notify(move |bar| {
                if search_button.is_active() != bar.is_search_mode() {
                    search_button.set_active(bar.is_search_mode());
                }
                set_material_icon_button_selected(&search_button, bar.is_search_mode());
            });
        }

        {
            let weak = Rc::downgrade(&controller);
            let pending_search = Rc::new(RefCell::new(None::<glib::SourceId>));
            let pending_history = Rc::new(RefCell::new(None::<glib::SourceId>));
            search_entry.connect_search_changed(move |entry| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };

                if let Some(source) = pending_search.borrow_mut().take() {
                    source.remove();
                }
                if let Some(source) = pending_history.borrow_mut().take() {
                    source.remove();
                }

                let query = entry.text().trim().to_string();
                controller.search_query.replace(query.clone());
                let youtube_only =
                    controller.config.borrow().startup_source == Some(StartupSource::YouTube);

                if query.is_empty() {
                    controller.refresh_recent_searches(true);
                    controller
                        .youtube_search_request_id
                        .set(controller.youtube_search_request_id.get().wrapping_add(1));
                    controller.youtube_library.borrow_mut().search =
                        YouTubeSearchResults::default();
                    controller.navigate_browser(BrowserRoute::All);
                    return;
                }

                controller.search_history_revealer.set_reveal_child(false);
                if youtube_only {
                    let mut cached = controller
                        .youtube_library
                        .borrow()
                        .cached_search_results(&query);
                    cached.loading = true;
                    controller.youtube_library.borrow_mut().search = cached;
                }
                controller.navigate_browser(BrowserRoute::All);

                let history_controller = Rc::downgrade(&controller);
                let history_pending = pending_history.clone();
                let history_query = query.clone();
                let history_source =
                    glib::timeout_add_local_once(Duration::from_millis(800), move || {
                        history_pending.borrow_mut().take();
                        if let Some(controller) = history_controller.upgrade() {
                            controller.record_recent_search(&history_query);
                        }
                    });
                pending_history.borrow_mut().replace(history_source);

                if !youtube_only {
                    return;
                }

                let delayed_controller = Rc::downgrade(&controller);
                let delayed_pending = pending_search.clone();
                let source = glib::timeout_add_local_once(Duration::from_millis(350), move || {
                    delayed_pending.borrow_mut().take();
                    if let Some(controller) = delayed_controller.upgrade() {
                        controller.request_global_youtube_search(query);
                    }
                });
                pending_search.borrow_mut().replace(source);
            });
        }

        {
            let weak = Rc::downgrade(&controller);
            search_entry.connect_activate(move |entry| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                let query = entry.text().trim().to_string();
                if !query.is_empty() {
                    controller.record_recent_search(&query);
                    controller.search_history_revealer.set_reveal_child(false);
                }
            });
        }

        {
            let weak = Rc::downgrade(&controller);
            let focus = gtk::EventControllerFocus::new();
            focus.connect_enter(move |_| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                if controller.search_entry.text().trim().is_empty() {
                    controller.refresh_recent_searches(true);
                }
            });
            search_entry.add_controller(focus);
        }

        {
            let weak = Rc::downgrade(&controller);
            settings_button.connect_toggled(move |button| {
                set_material_icon_button_selected(button, button.is_active());
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                if button.is_active() {
                    controller.open_settings_page();
                } else {
                    controller.close_settings_page();
                }
            });
        }

        for button in [&folder_button, &empty_add] {
            let weak = Rc::downgrade(&controller);
            button.connect_clicked(move |_| {
                if let Some(controller) = weak.upgrade() {
                    controller.choose_library_folder();
                }
            });
        }

        {
            let weak = Rc::downgrade(&controller);
            hero_play_button.connect_clicked(move |_| {
                if let Some(controller) = weak.upgrade() {
                    controller.toggle_playback();
                }
            });
        }

        {
            let weak = Rc::downgrade(&controller);
            play.connect_clicked(move |_| {
                if let Some(controller) = weak.upgrade() {
                    controller.toggle_playback();
                }
            });
        }

        {
            let weak = Rc::downgrade(&controller);
            footer_previous.connect_clicked(move |_| {
                if let Some(controller) = weak.upgrade() {
                    controller.previous_track();
                }
            });
        }

        {
            let weak = Rc::downgrade(&controller);
            footer_next.connect_clicked(move |_| {
                if let Some(controller) = weak.upgrade() {
                    controller.next_track();
                }
            });
        }

        {
            let weak = Rc::downgrade(&controller);
            footer_now_playing.connect_clicked(move |_| {
                if let Some(controller) = weak.upgrade() {
                    controller.show_footer_playback_queue();
                }
            });
        }

        {
            let weak = Rc::downgrade(&controller);
            mute_button.connect_clicked(move |_| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };

                // Compact mode uses the icon as a disclosure control. Full
                // mode keeps the familiar mute/unmute behavior.
                if controller.player_bar.has_css_class("footer-mode-compact") {
                    controller
                        .compact_volume_expanded
                        .set(!controller.compact_volume_expanded.get());
                    controller.apply_compact_volume_expansion();
                    return;
                }

                let current = controller.volume.value();
                if current > 0.001 {
                    controller.volume_before_mute.set(current);
                    controller.volume.set_value(0.0);
                } else {
                    controller
                        .volume
                        .set_value(controller.volume_before_mute.get().clamp(0.15, 1.0));
                }
            });
        }

        {
            let weak = Rc::downgrade(&controller);
            controller.footer_progress.connect_seek(move |fraction| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                if !controller.player.is_seekable() {
                    return;
                }
                let duration = controller.player.duration_us();
                if duration > 0 {
                    controller.seek_to((fraction * duration as f64) as i64, true);
                }
            });
        }

        {
            let weak = Rc::downgrade(&controller);
            controller.home_wave_progress.connect_seek(move |fraction| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                if !controller.player.is_seekable() {
                    return;
                }
                let duration = controller.player.duration_us();
                if duration > 0 {
                    controller.seek_to((fraction * duration as f64) as i64, true);
                }
            });
        }

        {
            let weak = Rc::downgrade(&controller);
            previous.connect_clicked(move |_| {
                if let Some(controller) = weak.upgrade() {
                    controller.previous_track();
                }
            });
        }

        {
            let weak = Rc::downgrade(&controller);
            next.connect_clicked(move |_| {
                if let Some(controller) = weak.upgrade() {
                    controller.next_track();
                }
            });
        }

        {
            let weak = Rc::downgrade(&controller);
            repeat.connect_toggled(move |button| {
                if let Some(controller) = weak.upgrade() {
                    let enabled = button.is_active();
                    if controller.footer_repeat_button.is_active() != enabled {
                        controller.footer_repeat_button.set_active(enabled);
                    }
                    controller
                        .mpris
                        .send(crate::playback::mpris::MprisUpdate::Loop(enabled));
                }
            });
        }
        {
            let weak = Rc::downgrade(&controller);
            footer_repeat.connect_toggled(move |button| {
                if let Some(controller) = weak.upgrade() {
                    let enabled = button.is_active();
                    if controller.repeat_button.is_active() != enabled {
                        controller.repeat_button.set_active(enabled);
                    }
                }
            });
        }

        {
            let weak = Rc::downgrade(&controller);
            shuffle.connect_toggled(move |button| {
                if let Some(controller) = weak.upgrade() {
                    let enabled = button.is_active();
                    if controller.footer_shuffle_button.is_active() != enabled {
                        controller.footer_shuffle_button.set_active(enabled);
                    }
                    controller.shuffle_enabled.set(enabled);
                    controller.reset_shuffle_navigation(enabled);
                    controller
                        .mpris
                        .send(crate::playback::mpris::MprisUpdate::Shuffle(enabled));
                }
            });
        }
        {
            let weak = Rc::downgrade(&controller);
            footer_shuffle.connect_toggled(move |button| {
                if let Some(controller) = weak.upgrade() {
                    let enabled = button.is_active();
                    if controller.shuffle_button.is_active() != enabled {
                        controller.shuffle_button.set_active(enabled);
                    }
                }
            });
        }

        for button in [&favorite, &footer_favorite] {
            let weak = Rc::downgrade(&controller);
            button.connect_clicked(move |_| {
                if let Some(controller) = weak.upgrade() {
                    controller.toggle_favorite();
                }
            });
        }

        {
            let weak = Rc::downgrade(&controller);
            controller.lyrics_button.connect_toggled(move |button| {
                set_material_icon_button_selected(button, button.is_active());
                if let Some(controller) = weak.upgrade() {
                    controller
                        .views
                        .set_visible_child_name(if button.is_active() {
                            "lyrics"
                        } else {
                            "music"
                        });
                    if button.is_active() {
                        let lyrics = controller.lyrics.clone();
                        glib::idle_add_local_once(move || lyrics.recenter(false));
                    }
                }
            });
        }

        for (button, route) in [
            (&controller.sidebar_all, BrowserRoute::All),
            (&controller.sidebar_albums, BrowserRoute::Albums),
            (&controller.sidebar_artists, BrowserRoute::Artists),
            (&controller.sidebar_playlists, BrowserRoute::Playlists),
            (&controller.sidebar_liked, BrowserRoute::Liked),
        ] {
            let weak = Rc::downgrade(&controller);
            button.connect_clicked(move |_| {
                if let Some(controller) = weak.upgrade() {
                    controller.navigate_browser(route.clone());
                }
            });
        }

        {
            let weak = Rc::downgrade(&controller);
            sync_button.connect_clicked(move |_| {
                if let Some(controller) = weak.upgrade() {
                    controller.sync_active_library();
                }
            });
        }

        {
            let weak = Rc::downgrade(&controller);
            inline_lyrics_button.connect_toggled(move |button| {
                set_material_icon_button_selected(button, button.is_active());
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                let visible = button.is_active();
                controller.player_view.set_lyrics_visible(visible);

                let changed = controller.config.borrow().show_home_lyrics != visible;
                if changed {
                    controller.config.borrow_mut().show_home_lyrics = visible;
                    controller.save_config();
                }
            });
        }

        {
            let weak = Rc::downgrade(&controller);
            refresh_lyrics_button.connect_clicked(move |_| {
                if let Some(controller) = weak.upgrade() {
                    controller.refresh_current_lyrics();
                }
            });
        }

        controller.apply_visual_theme();
        controller.refresh_browser();
        controller.refresh_youtube_status();
        controller
    }
}

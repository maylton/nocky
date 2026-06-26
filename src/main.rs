// local_artist_index_foundation_v3
// stable_artist_directory_refresh_v1
// stable_collection_identity_and_deferred_cache_v2
// clickable_player_artist_album_navigation_v1
// artist_profile_revalidation_v5
// youtube_like_reconciliation_and_request_guard_v1
// youtube_like_button_and_track_menu_v2
// youtube_real_like_sync_v5
// source_aware_liked_songs_page_v1
// clickable_lyrics_seek_v3
// fix_resume_seek_oscillation_v1
// fix_shutdown_save_order_v1
// youtube_resume_seek_convergence_v1
// lyrics_2_v2
// playback_resume_preferences_fix_v1
// playback_persistence_resume_2_v1
// queue_collection_cover_fallback_v1
// preserve_home_carousel_scroll_v1
// collection_card_inline_loading_fix_v2
// youtube_collection_background_playback_v1
// collection_card_loading_spinner_v3\n// youtube_collection_queue_background_load_v1
// collection_card_overflow_and_play_state_v2
// youtube_playlist_background_autoplay_v1
// contextual_collection_controls_v5
// recent_activity_exact_fix_v1
// personalized_home_resume_v2
mod animated_page_switcher;
mod artist_index;
mod background;
mod background_handler;
mod browser;
mod compact_volume_motion;
mod config;
mod dialogs;
mod expressive_transport;
mod footer_layout;
mod footer_now_playing;
mod footer_progress;
mod footer_transport;
mod footer_utilities;
mod footer_view;
mod i18n;
mod library;
mod listening_history;
mod local_mix_cover;
// material_dynamic_palette_v1
mod lyrics;
mod lyrics_provider;
mod lyrics_view;
mod material_palette;
mod md3_volume;
mod mode_toggle;
mod model;
mod mpris;
mod onboarding;
mod playback;
mod playback_session;
mod player_view;
pub mod queue_model;
mod queue_store;
mod reveal_bounce;
mod settings_page;
mod theme;
mod theme_css;
mod track_transition;
mod visual_theme;
mod visualizer;
mod wave_progress;
mod youtube;
mod youtube_diagnostics;
mod youtube_error;
mod youtube_playback;

use adw::prelude::*;
use animated_page_switcher::{AnimatedPageSwitcher, TopPage};
use background::{BackgroundChannel, BackgroundMessage};
use browser::{
    BrowserEvent, BrowserPlaybackState, BrowserRenderContext, BrowserRoute, LibraryBrowser,
    YouTubeCollectionRoute,
};
use compact_volume_motion::{run_compact_volume_spring, CompactVolumeSpring};
use config::{AppLanguage, BlurMode, StartupSource, VisualTheme};
use dialogs::SettingsEvent;
use expressive_transport::ExpressiveTransport;
use footer_layout::{
    footer_full_artwork_size_for_card_height, footer_mode_plan, AdaptiveFooterTier,
    FOOTER_ARTWORK_SOURCE_SIZE,
};
use footer_view::{build_footer_view, FooterViewParts};
use gtk::prelude::FileExt;
use gtk::{gdk, gio, glib};
use i18n::Message;
use listening_history::{ListeningHistory, ListeningSource};
use lyrics::LyricLine;
use lyrics_view::LyricsPresenter;
use model::{Track, TrackData};
use playback::{PlaybackEngine, PlaybackEvent};
use playback_session::PlaybackSession;
use player_view::{PlayerView, PlayerViewHandle};
use queue_model::{
    queue_end_action, PlaybackQueue, QueueEndAction, QueueEntryId, QueueMedia, QueueSnapshot,
    QueueSource, QueueSourceKind, ShuffleNavigator,
};
use reveal_bounce::RevealBounce;
use settings_page::SettingsPage;
use std::{
    cell::{Cell, RefCell},
    collections::{hash_map::DefaultHasher, HashMap, HashSet, VecDeque},
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    rc::Rc,
    sync::{mpsc, Arc, Mutex},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use track_transition::TransitionClock;
use visualizer::SpectrumVisualizer;
use wave_progress::WaveProgress;
use youtube::{
    cache_items_for_browser, credited_artists, load_library_cache, youtube_collection_cache_key,
    youtube_collection_key, YouTubeBridge, YouTubeItem, YouTubeLibraryCache, YouTubePage,
    YouTubePageEvent, YouTubeSearchResults, YouTubeStatus,
};

const APP_ID: &str = "io.github.maylton.Nocky";
const HOME_PLAYER_WIDTH: i32 = 454;
const SIDEBAR_WIDTH: i32 = 252;

#[derive(Default)]
struct AppState {
    tracks: Vec<Track>,
    current: Option<usize>,
    playback_queue: Vec<usize>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum PlaybackSource {
    #[default]
    None,
    Local,
    YouTube,
}

#[derive(Clone, Debug)]
struct YouTubePlaybackState {
    queue: Vec<YouTubeItem>,
    current: usize,
    item: YouTubeItem,
    cover_path: Option<PathBuf>,
    lyrics: Vec<LyricLine>,
}

struct SidebarParts {
    revealer: gtk::Revealer,
    motion: gtk::Fixed,
    content: gtk::Box,
    all_button: gtk::Button,
    all_label: gtk::Label,
    albums_button: gtk::Button,
    albums_label: gtk::Label,
    artists_button: gtk::Button,
    artists_label: gtk::Label,
    playlists_button: gtk::Button,
    playlists_label: gtk::Label,
    liked_button: gtk::Button,
    liked_label: gtk::Label,
    section_label: gtk::Label,
}

struct AppController {
    window: adw::ApplicationWindow,
    toast_overlay: adw::ToastOverlay,
    player: PlaybackEngine,
    state: RefCell<AppState>,
    // queue2_playback_bridge_v1
    // queue2_browser_actions_dnd_v1
    // queue2_persistence_v1
    playback_queue_v2: RefCell<PlaybackQueue>,
    active_queue_source: Cell<QueueSourceKind>,
    queue_last_saved_snapshot: RefCell<QueueSnapshot>,
    queue_dragged_entry: Cell<Option<QueueEntryId>>,
    queue_v2_pending_entry: Cell<Option<QueueEntryId>>,
    config: RefCell<config::AppConfig>,
    listening_history: RefCell<ListeningHistory>,
    listening_session_id: RefCell<Option<String>>,
    listening_session_last_saved_seconds: Cell<u64>,
    listening_history_context: RefCell<listening_history::PlaybackHistoryContext>,
    pending_resume_position_us: Cell<Option<i64>>,
    restored_playback_session: RefCell<Option<PlaybackSession>>,
    startup_restore_autoplay: Cell<Option<bool>>,
    playback_session_last_position_seconds: Cell<u64>,
    playback_session_last_shuffle: Cell<bool>,
    playback_session_last_repeat: Cell<bool>,
    playback_session_restore_attempts: Cell<u8>,
    updating_progress: Cell<bool>,
    scanning: Cell<bool>,
    shuffle_enabled: Cell<bool>,
    shuffle_navigation: RefCell<ShuffleNavigator>,
    rng_state: Cell<u64>,
    search_query: RefCell<String>,
    lyrics_pending: RefCell<HashSet<PathBuf>>,
    background: BackgroundChannel,
    mpris: mpris::MprisBridge,
    last_mpris_position: Cell<i64>,
    playback_source: Cell<PlaybackSource>,
    youtube_state: RefCell<Option<YouTubePlaybackState>>,
    youtube_request_id: Cell<u64>,
    youtube_search_request_id: Cell<u64>,
    youtube_recovery_in_progress: Cell<bool>,
    youtube_recovery_attempted: Cell<bool>,
    youtube_recovery_retry_count: Cell<u8>,
    youtube_recovery_generation: Cell<u64>,
    youtube_recovery_resume_us: Cell<i64>,
    youtube_recovery_was_playing: Cell<bool>,
    youtube_playlist_request_id: Cell<u64>,
    youtube_collection_play_request_id: Cell<u64>,
    youtube_collection_queue_request_id: Cell<u64>,
    youtube_collection_prefetching: Cell<bool>,
    youtube_playlist_loading: Cell<bool>,
    youtube_playlist_prefetching: Cell<bool>,
    youtube_pending_playlist: RefCell<Option<YouTubeItem>>,
    youtube_bridge: Option<Arc<YouTubeBridge>>,
    youtube_library: RefCell<YouTubeLibraryCache>,
    youtube_like_request_id: Cell<u64>,
    youtube_like_pending: RefCell<HashMap<String, u64>>,

    sidebar: gtk::Revealer,
    // reveal_bounce_and_release_0_3_0_v2
    sidebar_motion: gtk::Fixed,
    sidebar_content: gtk::Box,
    sidebar_bounce: Rc<RevealBounce>,
    sidebar_button: gtk::ToggleButton,
    sidebar_all: gtk::Button,
    sidebar_all_label: gtk::Label,
    sidebar_albums: gtk::Button,
    sidebar_albums_label: gtk::Label,
    sidebar_artists: gtk::Button,
    sidebar_artists_label: gtk::Label,
    sidebar_playlists: gtk::Button,
    sidebar_playlists_label: gtk::Label,
    sidebar_liked: gtk::Button,
    sidebar_liked_label: gtk::Label,
    sidebar_section_label: gtk::Label,
    search_button: gtk::ToggleButton,
    folder_button: gtk::Button,
    search_entry: gtk::SearchEntry,
    // navigable_settings_page_v1
    settings_button: gtk::ToggleButton,
    content_stack: gtk::Stack,
    settings_page: Rc<SettingsPage>,
    views: adw::ViewStack,
    music_page: adw::ViewStackPage,
    lyrics_page: adw::ViewStackPage,
    // animated_top_page_switcher_v2
    page_switcher: Rc<AnimatedPageSwitcher>,
    browser: LibraryBrowser,
    lyrics: LyricsPresenter,
    youtube_page: Rc<YouTubePage>,
    player_view: PlayerViewHandle,
    // home_player_collapse_and_dialog_fix_v2
    player_revealer: gtk::Revealer,
    player_motion: gtk::Fixed,
    player_viewport: gtk::ScrolledWindow,
    player_bounce: Rc<RevealBounce>,
    player_toggle_button: gtk::Button,
    player_toggle_icon: gtk::Image,
    player_artist: gtk::Label,
    album: gtk::Label,
    now_heading: gtk::Label,
    favorite_button: gtk::Button,
    previous_button: gtk::Button,
    hero_play_button: gtk::Button,
    main_transport_motion: Rc<ExpressiveTransport>,
    next_button: gtk::Button,
    mini_title: gtk::Label,
    mini_artist: gtk::Label,
    footer_source: gtk::Label,
    footer_now_playing: gtk::Button,
    footer_center: gtk::Box,
    footer_right_controls: gtk::Box,
    volume_revealer: gtk::Revealer,
    music_stack: gtk::Stack,
    empty_title: gtk::Label,
    empty_text: gtk::Label,
    empty_add: gtk::Button,
    hero_cover: CoverView,
    mini_cover: CoverView,
    player_bar: gtk::CenterBox,

    play_icon: gtk::Image,
    hero_play_icon: gtk::Image,
    favorite_icon: gtk::Image,
    footer_favorite_icon: gtk::Image,
    footer_favorite_button: gtk::Button,
    progress: gtk::Scale,
    home_progress_stack: gtk::Stack,
    home_wave_progress: WaveProgress,
    elapsed: gtk::Label,
    duration: gtk::Label,
    footer_progress_stack: gtk::Stack,
    footer_traditional_progress: gtk::Scale,
    footer_progress: WaveProgress,
    footer_elapsed: gtk::Label,
    footer_duration: gtk::Label,
    volume: gtk::Adjustment,
    mute_icon: gtk::Image,
    mute_button: gtk::Button,
    volume_before_mute: Cell<f64>,
    compact_volume_expanded: Cell<bool>,
    compact_volume_spring_generation: Rc<Cell<u64>>,
    footer_metadata_transition: TransitionClock,
    lyrics_button: gtk::ToggleButton,
    footer_previous: gtk::Button,
    footer_play_button: gtk::Button,
    footer_transport_motion: Rc<ExpressiveTransport>,
    footer_next: gtk::Button,
    footer_repeat_button: gtk::ToggleButton,
    footer_shuffle_button: gtk::ToggleButton,
    repeat_button: gtk::ToggleButton,
    shuffle_button: gtk::ToggleButton,
    visualizer: SpectrumVisualizer,

    visual_theme_manager: Rc<visual_theme::VisualThemeManager>,
    _theme: Rc<theme::ThemeBridge>,
}

fn resolve_youtube_collection_item(
    bridge: &YouTubeBridge,
    item: &YouTubeItem,
    filter: &str,
) -> Result<YouTubeItem, String> {
    if !item.browse_id.trim().is_empty() {
        return Ok(item.clone());
    }

    let query = item.title.trim();
    if query.is_empty() {
        return Err("The YouTube Music collection has no title".to_string());
    }

    let mut candidates = bridge.search(query, filter)?;
    candidates.retain(|candidate| {
        candidate
            .result_type
            .eq_ignore_ascii_case(item.result_type.as_str())
            || candidate
                .result_type
                .eq_ignore_ascii_case(filter.trim_end_matches('s'))
    });

    candidates
        .iter()
        .position(|candidate| {
            candidate.title.eq_ignore_ascii_case(query)
                && (item.artist.trim().is_empty()
                    || candidate.artist.eq_ignore_ascii_case(item.artist.trim()))
        })
        .or_else(|| {
            candidates
                .iter()
                .position(|candidate| candidate.title.eq_ignore_ascii_case(query))
        })
        .map(|index| candidates.remove(index))
        .or_else(|| candidates.into_iter().next())
        .ok_or_else(|| {
            format!(
                "No YouTube Music {} could be resolved for '{}'",
                item.result_type, item.title
            )
        })
}

fn scanned_library_matches(tracks: &[Track], data: &[TrackData]) -> bool {
    tracks.len() == data.len()
        && tracks.iter().zip(data).all(|(track, incoming)| {
            track.path == incoming.path
                && track.title == incoming.title
                && track.artist == incoming.artist
                && track.album == incoming.album
                && track.duration_seconds == incoming.duration_seconds
                && track.disc_number == incoming.disc_number
                && track.track_number == incoming.track_number
                && track.cover_path == incoming.cover_path
        })
}

fn main() -> glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_application);
    app.run()
}

fn build_application(app: &adw::Application) {
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
        // expressive_home_card_motion_stability_v1: flush
        // The regular checkpoints are asynchronous; shutdown performs one
        // serialized final snapshot so the latest playback session is kept.
        keep_alive.listening_history.borrow().flush();
        keep_alive.persist_queue_now();
        keep_alive.persist_playback_session_now();
        keep_alive.player.shutdown();
        keep_alive.mpris.send(mpris::MprisUpdate::Shutdown);
    });
}

impl AppController {
    fn new(app: &adw::Application) -> Rc<Self> {
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
        // material_expressive_navigation_v1
        sidebar_button.add_css_class("header-navigation-button");
        header.pack_start(&sidebar_button);

        let brand = gtk::Label::new(Some("NOCKY"));
        brand.add_css_class("brand-title");
        brand.add_css_class("header-brand");
        header.pack_start(&brand);

        // home_player_collapse_and_dialog_fix_v2
        let player_toggle_icon = gtk::Image::from_icon_name("view-grid-symbolic");
        player_toggle_icon.set_pixel_size(18);
        let player_toggle_button = gtk::Button::new();
        player_toggle_button.set_child(Some(&player_toggle_icon));
        player_toggle_button.add_css_class("flat");
        player_toggle_button.add_css_class("header-action-button");
        player_toggle_button.add_css_class("home-player-toggle-button");
        header.pack_start(&player_toggle_button);

        let page_switcher =
            AnimatedPageSwitcher::new(tr(Message::MusicTab), tr(Message::LyricsTab));
        header.set_title_widget(Some(page_switcher.root()));

        let search_button = gtk::ToggleButton::builder()
            .icon_name("system-search-symbolic")
            .tooltip_text(tr(Message::SearchLibrary))
            .build();
        search_button.add_css_class("header-action-button");
        header.pack_end(&search_button);

        let sync_button = gtk::Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text("Sincronizar biblioteca")
            .build();
        sync_button.add_css_class("flat");
        sync_button.add_css_class("header-action-button");
        header.pack_end(&sync_button);

        let folder_button = gtk::Button::builder()
            .icon_name("folder-open-symbolic")
            .tooltip_text(tr(Message::ChooseMusicFolderTooltip))
            .build();
        folder_button.add_css_class("header-action-button");
        header.pack_end(&folder_button);

        let settings_button = gtk::ToggleButton::builder()
            .icon_name("preferences-system-symbolic")
            .tooltip_text(tr(Message::SettingsTitle))
            .build();
        settings_button.add_css_class("flat");
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
        search_bar.set_child(Some(&search_entry));
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
            config.expressive_transport_effects
                && config.visual_theme == VisualTheme::MaterialExpressive,
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
        empty_add.add_css_class("suggested-action");
        empty_add.add_css_class("pill");
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

        // nocky_rust_ui_phase3g_footer_view_assembly_v1
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
            config.expressive_transport_effects
                && config.visual_theme == VisualTheme::MaterialExpressive,
            &mini_cover.stack,
        );

        // nocky_custom_md3_volume_canvas_v2
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

        let mpris = mpris::MprisBridge::start(config.volume);
        let youtube_bridge = YouTubeBridge::discover().ok().map(Arc::new);

        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos() as u64)
            .unwrap_or(0x9e37_79b9_7f4a_7c15);

        let initial_queue_source = match config.startup_source {
            Some(StartupSource::YouTube) => QueueSourceKind::YouTube,
            Some(StartupSource::Local) | None => QueueSourceKind::Local,
        };
        let queue_load = queue_store::load_for(initial_queue_source);
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
        let restored_playback_session = playback_session::load_for(initial_queue_source);

        let initial_volume = config.volume.clamp(0.15, 1.0);
        let mut listening_history = ListeningHistory::load();
        listening_history.set_recording_enabled(config.collect_listening_history);
        let sidebar_bounce = RevealBounce::new(false);
        let player_bounce = RevealBounce::new(!config.home_player_collapsed);
        let controller = Rc::new(Self {
            window,
            toast_overlay,
            player,
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
            lyrics_pending: RefCell::new(HashSet::new()),
            background,
            mpris,
            last_mpris_position: Cell::new(-1),
            playback_source: Cell::new(PlaybackSource::None),
            youtube_state: RefCell::new(None),
            youtube_request_id: Cell::new(0),
            youtube_search_request_id: Cell::new(0),
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
            youtube_bridge,
            youtube_library: RefCell::new(load_library_cache()),
            youtube_like_request_id: Cell::new(0),
            youtube_like_pending: RefCell::new(HashMap::new()),
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
            settings_button: settings_button.clone(),
            content_stack: content_stack.clone(),
            settings_page: settings_page.clone(),
            views,
            music_page,
            lyrics_page,
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
            let page_switcher = page_switcher.clone();
            controller
                .views
                .connect_visible_child_name_notify(move |stack| {
                    let page = if stack.visible_child_name().as_deref() == Some("lyrics") {
                        TopPage::Lyrics
                    } else {
                        TopPage::Home
                    };
                    page_switcher.set_active_page(page, true);
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

        // home_tab_navigation_v1
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
            search_button.connect_toggled(move |button| {
                search_bar.set_search_mode(button.is_active());
            });
        }

        {
            let search_button = search_button.clone();
            search_bar.connect_search_mode_enabled_notify(move |bar| {
                if search_button.is_active() != bar.is_search_mode() {
                    search_button.set_active(bar.is_search_mode());
                }
            });
        }

        {
            let weak = Rc::downgrade(&controller);
            let pending_search = Rc::new(RefCell::new(None::<glib::SourceId>));
            search_entry.connect_search_changed(move |entry| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };

                if let Some(source) = pending_search.borrow_mut().take() {
                    source.remove();
                }

                let query = entry.text().trim().to_string();
                controller.search_query.replace(query.clone());
                let youtube_only =
                    controller.config.borrow().startup_source == Some(StartupSource::YouTube);

                if query.is_empty() {
                    controller
                        .youtube_search_request_id
                        .set(controller.youtube_search_request_id.get().wrapping_add(1));
                    controller.youtube_library.borrow_mut().search =
                        YouTubeSearchResults::default();
                    controller.navigate_browser(BrowserRoute::All);
                    return;
                }

                if youtube_only {
                    controller.youtube_library.borrow_mut().search = YouTubeSearchResults {
                        query: query.clone(),
                        loading: true,
                        ..YouTubeSearchResults::default()
                    };
                }
                controller.navigate_browser(BrowserRoute::All);

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
            settings_button.connect_toggled(move |button| {
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

                // nocky_compact_volume_expand_and_flat_modes_v1
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
                    controller.mpris.send(mpris::MprisUpdate::Loop(enabled));
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
                    controller.mpris.send(mpris::MprisUpdate::Shuffle(enabled));
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

    fn setup_callbacks(self: &Rc<Self>) {
        self.mpris
            .send(mpris::MprisUpdate::Volume(self.volume.value()));
        self.mpris
            .send(mpris::MprisUpdate::Loop(self.repeat_button.is_active()));
        self.mpris
            .send(mpris::MprisUpdate::Shuffle(self.shuffle_button.is_active()));
        self.publish_mpris_capabilities();

        {
            self.window
                .connect_close_request(move |_| glib::Propagation::Proceed);
        }

        {
            let weak = Rc::downgrade(self);
            let pending_save = Rc::new(RefCell::new(None::<glib::SourceId>));
            self.volume.connect_value_changed(move |scale| {
                if let Some(controller) = weak.upgrade() {
                    let value = scale.value().clamp(0.0, 1.0);
                    controller.player.set_volume(value);
                    controller.config.borrow_mut().volume = value;
                    if value > 0.001 {
                        controller.volume_before_mute.set(value);
                    }
                    controller.apply_volume_icon();
                    controller.mpris.send(mpris::MprisUpdate::Volume(value));

                    if let Some(source) = pending_save.borrow_mut().take() {
                        source.remove();
                    }
                    let weak = weak.clone();
                    let pending = pending_save.clone();
                    let source =
                        glib::timeout_add_local_once(Duration::from_millis(350), move || {
                            pending.borrow_mut().take();
                            if let Some(controller) = weak.upgrade() {
                                controller.save_config();
                            }
                        });
                    pending_save.borrow_mut().replace(source);
                }
            });
        }

        {
            let weak = Rc::downgrade(self);
            self.progress.connect_value_changed(move |scale| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                if controller.updating_progress.get() || !controller.player.is_seekable() {
                    return;
                }
                let duration = controller.player.duration_us();
                if duration > 0 {
                    controller.seek_to((scale.value() * duration as f64) as i64, true);
                }
            });
        }

        {
            let weak = Rc::downgrade(self);
            self.footer_traditional_progress
                .connect_value_changed(move |scale| {
                    let Some(controller) = weak.upgrade() else {
                        return;
                    };
                    if controller.updating_progress.get() || !controller.player.is_seekable() {
                        return;
                    }
                    let duration = controller.player.duration_us();
                    if duration > 0 {
                        controller.seek_to((scale.value() * duration as f64) as i64, true);
                    }
                });
        }

        {
            let weak = Rc::downgrade(self);
            let mut progress_ticks = 0_u8;
            glib::timeout_add_local(Duration::from_millis(50), move || {
                let Some(controller) = weak.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                controller.handle_background_messages();
                controller.handle_browser_events();
                controller.handle_youtube_events();
                controller.handle_settings_events();
                controller.handle_mpris_commands();
                controller.handle_playback_events();

                progress_ticks = progress_ticks.wrapping_add(1);
                let cadence = if controller.player.is_playing() {
                    2
                } else {
                    10
                };
                if progress_ticks.is_multiple_of(cadence) {
                    controller.refresh_progress();
                }
                glib::ControlFlow::Continue
            });
        }

        {
            let weak = Rc::downgrade(self);
            glib::timeout_add_local(Duration::from_secs(10 * 60), move || {
                let Some(controller) = weak.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                if controller.config.borrow().youtube_auto_sync
                    && controller.youtube_library.borrow().connected
                {
                    let _ = controller.sync_youtube_library(true, false);
                }
                glib::ControlFlow::Continue
            });
        }
    }

    fn open_library_home(&self) {
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

    // functional_carousel_queue_blur_fix_v1
    // queue2_interface_v1
    fn rebuild_queue_popover(
        self: &Rc<Self>,
        list: &gtk::Box,
        summary: &gtk::Label,
        clear_upcoming: &gtk::Button,
        popover: &gtk::Popover,
    ) {
        while let Some(child) = list.first_child() {
            list.remove(&child);
        }

        let (entries, current_id, current_index) = {
            let queue = self.playback_queue_v2.borrow();
            (
                queue.entries().to_vec(),
                queue.current_id(),
                queue.current_index(),
            )
        };

        let language = self.config.borrow().language;
        let count = entries.len();
        let summary_text = match language {
            AppLanguage::Portuguese => {
                format!("{count} {}", if count == 1 { "faixa" } else { "faixas" })
            }
            AppLanguage::English => {
                format!("{count} {}", if count == 1 { "track" } else { "tracks" })
            }
            AppLanguage::Spanish => {
                format!("{count} {}", if count == 1 { "pista" } else { "pistas" })
            }
        };
        summary.set_text(&summary_text);
        clear_upcoming.set_sensitive(
            current_index.is_some_and(|position| position.saturating_add(1) < entries.len()),
        );

        if entries.is_empty() {
            // queue2_interface_polish_v1: richer empty state
            let empty = gtk::Box::new(gtk::Orientation::Vertical, 7);
            empty.set_margin_top(18);
            empty.set_margin_bottom(18);
            empty.set_margin_start(12);
            empty.set_margin_end(12);
            empty.set_halign(gtk::Align::Fill);
            empty.set_valign(gtk::Align::Center);
            empty.add_css_class("queue2-state");
            empty.add_css_class("queue2-empty-state");

            let icon = gtk::Image::from_icon_name("view-list-symbolic");
            icon.set_pixel_size(34);
            icon.add_css_class("queue2-state-icon");

            let title = gtk::Label::new(Some(match language {
                AppLanguage::Portuguese => "A fila está vazia",
                AppLanguage::English => "The queue is empty",
                AppLanguage::Spanish => "La cola está vacía",
            }));
            title.add_css_class("queue2-state-title");

            let description = gtk::Label::new(Some(match language {
                AppLanguage::Portuguese => {
                    "Use “Reproduzir em seguida” ou “Adicionar ao fim” nas faixas."
                }
                AppLanguage::English => "Use “Play next” or “Add to end” from any track.",
                AppLanguage::Spanish => {
                    "Usa “Reproducir después” o “Añadir al final” en una pista."
                }
            }));
            description.set_wrap(true);
            description.set_justify(gtk::Justification::Center);
            description.add_css_class("dim-label");
            description.add_css_class("queue2-state-description");

            empty.append(&icon);
            empty.append(&title);
            empty.append(&description);
            list.append(&empty);
            return;
        }

        for (position, entry) in entries.into_iter().enumerate() {
            let is_current = current_id == Some(entry.id);

            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.set_margin_top(4);
            row.set_margin_bottom(4);
            row.set_margin_start(4);
            row.set_margin_end(4);
            row.add_css_class("queue2-row");
            if is_current {
                row.add_css_class("active");
            }

            // queue2_drag_indicator_v1
            // Keep the widget that owns GestureDrag parented and intact.
            // A compact accent marker moves through the list to show the
            // destination without duplicating the whole track row.
            let drag_icon = gtk::Image::from_icon_name("list-drag-handle-symbolic");
            drag_icon.set_pixel_size(18);
            drag_icon.set_can_target(false);

            // queue2_interface_polish_v1: semantic drag handle with keyboard operation
            let drag_handle = gtk::Button::new();
            drag_handle.set_size_request(34, 34);
            drag_handle.set_halign(gtk::Align::Center);
            drag_handle.set_valign(gtk::Align::Center);
            drag_handle.set_focusable(true);
            drag_handle.set_cursor_from_name(Some("grab"));
            drag_handle.set_tooltip_text(Some(match language {
                AppLanguage::Portuguese => "Arraste ou use Alt+↑ / Alt+↓ para reordenar",
                AppLanguage::English => "Drag or use Alt+↑ / Alt+↓ to reorder",
                AppLanguage::Spanish => "Arrastra o usa Alt+↑ / Alt+↓ para reordenar",
            }));
            drag_handle.add_css_class("flat");
            drag_handle.add_css_class("circular");
            drag_handle.add_css_class("queue2-drag-handle");
            drag_handle.set_child(Some(&drag_icon));

            let drag_origin = Rc::new(Cell::new(position));
            let drag_target = Rc::new(Cell::new(position));
            let drag_indicator: Rc<RefCell<Option<gtk::Box>>> = Rc::new(RefCell::new(None));

            let drag_gesture = gtk::GestureDrag::new();
            drag_gesture.set_button(gdk::BUTTON_PRIMARY);
            drag_gesture.set_propagation_phase(gtk::PropagationPhase::Capture);

            {
                let weak = Rc::downgrade(self);
                let handle = drag_handle.clone();
                let row = row.clone();
                let list = list.clone();
                let drag_origin = drag_origin.clone();
                let drag_target = drag_target.clone();
                let drag_indicator = drag_indicator.clone();
                let id = entry.id;

                drag_gesture.connect_drag_begin(move |_, _, _| {
                    let Some(controller) = weak.upgrade() else {
                        return;
                    };

                    drag_origin.set(position);
                    drag_target.set(position);
                    controller.queue_dragged_entry.set(Some(id));
                    handle.set_cursor_from_name(Some("grabbing"));
                    row.set_opacity(0.48);
                    row.add_css_class("queue2-live-dragging");

                    let indicator = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                    indicator.set_height_request(9);
                    indicator.set_margin_start(38);
                    indicator.set_margin_end(16);
                    indicator.set_can_target(false);
                    indicator.add_css_class("queue2-drop-indicator");

                    let accent_line = gtk::ProgressBar::new();
                    accent_line.set_fraction(1.0);
                    accent_line.set_height_request(3);
                    accent_line.set_hexpand(true);
                    accent_line.set_valign(gtk::Align::Center);
                    accent_line.set_can_target(false);
                    accent_line.add_css_class("queue2-drop-indicator-line");

                    indicator.append(&accent_line);
                    list.insert_child_after(&indicator, Some(&row));
                    drag_indicator.replace(Some(indicator));
                });
            }

            {
                let list = list.clone();
                let row = row.clone();
                let drag_origin = drag_origin.clone();
                let drag_target = drag_target.clone();
                let drag_indicator = drag_indicator.clone();

                drag_gesture.connect_drag_update(move |_, _, offset_y| {
                    let indicator = {
                        let stored = drag_indicator.borrow();
                        stored.as_ref().cloned()
                    };
                    let Some(indicator) = indicator else {
                        return;
                    };

                    let row_height = row.height().max(1) as f64;
                    let delta = (offset_y / row_height).round() as isize;
                    let last_index = count.saturating_sub(1) as isize;
                    let target = (drag_origin.get() as isize + delta).clamp(0, last_index) as usize;

                    if target == drag_target.get() {
                        return;
                    }

                    let row_widget: gtk::Widget = row.clone().upcast();
                    let indicator_widget: gtk::Widget = indicator.clone().upcast();
                    let mut stable_rows = Vec::with_capacity(count.saturating_sub(1));
                    let mut child = list.first_child();
                    while let Some(widget) = child {
                        let next = widget.next_sibling();
                        if widget != row_widget && widget != indicator_widget {
                            stable_rows.push(widget);
                        }
                        child = next;
                    }

                    let previous_sibling = if target == 0 {
                        None
                    } else {
                        stable_rows.get(target - 1)
                    };
                    list.reorder_child_after(&indicator, previous_sibling);
                    drag_target.set(target);
                });
            }

            {
                let weak = Rc::downgrade(self);
                let handle = drag_handle.clone();
                let row = row.clone();
                let list = list.clone();
                let summary = summary.clone();
                let clear_upcoming = clear_upcoming.clone();
                let queue_popover = popover.clone();
                let drag_origin = drag_origin.clone();
                let drag_target = drag_target.clone();
                let drag_indicator = drag_indicator.clone();
                let fallback_id = entry.id;

                drag_gesture.connect_drag_end(move |_, _, _| {
                    handle.set_cursor_from_name(Some("grab"));
                    row.set_opacity(1.0);
                    row.remove_css_class("queue2-live-dragging");

                    let Some(controller) = weak.upgrade() else {
                        return;
                    };
                    let id = controller
                        .queue_dragged_entry
                        .replace(None)
                        .unwrap_or(fallback_id);
                    let origin = drag_origin.get();
                    let target = drag_target.get();
                    let indicator = drag_indicator.borrow_mut().take();

                    let idle_list = list.clone();
                    let idle_summary = summary.clone();
                    let idle_clear_upcoming = clear_upcoming.clone();
                    let idle_queue_popover = queue_popover.clone();

                    glib::idle_add_local_once(move || {
                        if let Some(indicator) = indicator {
                            if indicator.parent().is_some() {
                                idle_list.remove(&indicator);
                            }
                        }

                        if target != origin {
                            if let Err(error) = controller
                                .playback_queue_v2
                                .borrow_mut()
                                .move_entry(id, target)
                            {
                                controller.show_toast(&error.to_string());
                            }
                        }

                        controller.rebuild_queue_popover(
                            &idle_list,
                            &idle_summary,
                            &idle_clear_upcoming,
                            &idle_queue_popover,
                        );
                    });
                });
            }

            {
                let weak = Rc::downgrade(self);
                let handle = drag_handle.clone();
                let row = row.clone();
                let list = list.clone();
                let summary = summary.clone();
                let clear_upcoming = clear_upcoming.clone();
                let queue_popover = popover.clone();
                let drag_indicator = drag_indicator.clone();

                drag_gesture.connect_cancel(move |_, _| {
                    handle.set_cursor_from_name(Some("grab"));
                    row.set_opacity(1.0);
                    row.remove_css_class("queue2-live-dragging");

                    let Some(controller) = weak.upgrade() else {
                        return;
                    };
                    controller.queue_dragged_entry.set(None);
                    let indicator = drag_indicator.borrow_mut().take();

                    let idle_list = list.clone();
                    let idle_summary = summary.clone();
                    let idle_clear_upcoming = clear_upcoming.clone();
                    let idle_queue_popover = queue_popover.clone();

                    glib::idle_add_local_once(move || {
                        if let Some(indicator) = indicator {
                            if indicator.parent().is_some() {
                                idle_list.remove(&indicator);
                            }
                        }
                        controller.rebuild_queue_popover(
                            &idle_list,
                            &idle_summary,
                            &idle_clear_upcoming,
                            &idle_queue_popover,
                        );
                    });
                });
            }

            drag_handle.add_controller(drag_gesture);

            // queue2_interface_polish_v1: Alt+Up / Alt+Down mirrors pointer reordering.
            let key_controller = gtk::EventControllerKey::new();
            {
                let weak = Rc::downgrade(self);
                let list = list.clone();
                let summary = summary.clone();
                let clear_upcoming = clear_upcoming.clone();
                let queue_popover = popover.clone();
                let id = entry.id;

                key_controller.connect_key_pressed(move |_, key, _, state| {
                    if !state.contains(gdk::ModifierType::ALT_MASK) {
                        return glib::Propagation::Proceed;
                    }

                    let target = match key {
                        gdk::Key::Up if position > 0 => Some(position - 1),
                        gdk::Key::Down if position + 1 < count => Some(position + 1),
                        _ => None,
                    };
                    let Some(target) = target else {
                        return glib::Propagation::Proceed;
                    };

                    let Some(controller) = weak.upgrade() else {
                        return glib::Propagation::Proceed;
                    };

                    if let Err(error) = controller
                        .playback_queue_v2
                        .borrow_mut()
                        .move_entry(id, target)
                    {
                        controller.show_toast(&error.to_string());
                        return glib::Propagation::Stop;
                    }

                    controller.rebuild_queue_popover(
                        &list,
                        &summary,
                        &clear_upcoming,
                        &queue_popover,
                    );

                    let focus_list = list.clone();
                    glib::idle_add_local_once(move || {
                        let mut child = focus_list.first_child();
                        for _ in 0..target {
                            child = child.and_then(|widget| widget.next_sibling());
                        }
                        if let Some(row) = child {
                            if let Some(handle) = row.first_child() {
                                handle.grab_focus();
                            }
                        }
                    });

                    glib::Propagation::Stop
                });
            }
            drag_handle.add_controller(key_controller);
            row.append(&drag_handle);

            let play_area = gtk::Button::new();
            play_area.set_hexpand(true);
            play_area.set_halign(gtk::Align::Fill);
            play_area.add_css_class("flat");
            play_area.add_css_class("queue-popover-row");
            play_area.set_tooltip_text(Some(match language {
                AppLanguage::Portuguese => "Reproduzir esta faixa",
                AppLanguage::English => "Play this track",
                AppLanguage::Spanish => "Reproducir esta pista",
            }));

            let information = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            information.set_margin_top(8);
            information.set_margin_bottom(8);
            information.set_margin_start(10);
            information.set_margin_end(8);

            // queue2_completion_core_v1: real artwork with fixed natural size.
            let artwork = build_cover(42);
            artwork.stack.add_css_class("queue2-cover");
            artwork.set_path_immediate(entry.media.cover_path.as_deref());
            information.append(&artwork.stack);

            let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
            text.set_hexpand(true);

            let title = gtk::Label::new(Some(&entry.media.title));
            title.set_xalign(0.0);
            title.set_ellipsize(gtk::pango::EllipsizeMode::End);
            title.add_css_class("heading");

            let artist_text = if entry.media.artist.trim().is_empty() {
                match &entry.media.source {
                    QueueSource::Local { .. } => match language {
                        AppLanguage::Portuguese => "Artista desconhecido",
                        AppLanguage::English => "Unknown artist",
                        AppLanguage::Spanish => "Artista desconocido",
                    },
                    QueueSource::YouTube { .. } => "YouTube Music",
                }
            } else {
                entry.media.artist.as_str()
            };
            let artist = gtk::Label::new(Some(artist_text));
            artist.set_xalign(0.0);
            artist.set_ellipsize(gtk::pango::EllipsizeMode::End);
            artist.add_css_class("dim-label");

            text.append(&title);
            text.append(&artist);
            information.append(&text);

            let source = gtk::Label::new(Some(match &entry.media.source {
                QueueSource::Local { .. } => "LOCAL",
                QueueSource::YouTube { .. } => "YOUTUBE",
            }));
            source.add_css_class("caption");
            source.add_css_class("dim-label");
            information.append(&source);

            if is_current {
                let playing = gtk::Image::from_icon_name("audio-volume-high-symbolic");
                playing.set_pixel_size(16);
                playing.add_css_class("accent");
                playing.add_css_class("queue-playing-indicator");
                information.append(&playing);
                play_area.add_css_class("active");
                play_area.set_can_target(false);
                play_area.set_focusable(false);
            }

            play_area.set_child(Some(&information));
            if !is_current {
                let weak = Rc::downgrade(self);
                let queue_popover = popover.clone();
                let id = entry.id;
                play_area.connect_clicked(move |_| {
                    if let Some(controller) = weak.upgrade() {
                        controller.play_queue_entry(id, true);
                        queue_popover.popdown();
                    }
                });
            }
            row.append(&play_area);

            let move_up = gtk::Button::builder()
                .icon_name("go-up-symbolic")
                .tooltip_text(match language {
                    AppLanguage::Portuguese => "Mover para cima",
                    AppLanguage::English => "Move up",
                    AppLanguage::Spanish => "Mover hacia arriba",
                })
                .build();
            move_up.add_css_class("flat");
            move_up.add_css_class("circular");
            move_up.set_sensitive(position > 0);
            {
                let weak = Rc::downgrade(self);
                let list = list.clone();
                let summary = summary.clone();
                let clear_upcoming = clear_upcoming.clone();
                let queue_popover = popover.clone();
                let id = entry.id;
                move_up.connect_clicked(move |_| {
                    let Some(controller) = weak.upgrade() else {
                        return;
                    };
                    let result = controller
                        .playback_queue_v2
                        .borrow_mut()
                        .move_entry(id, position.saturating_sub(1));
                    if let Err(error) = result {
                        controller.show_toast(&error.to_string());
                        return;
                    }
                    controller.rebuild_queue_popover(
                        &list,
                        &summary,
                        &clear_upcoming,
                        &queue_popover,
                    );
                });
            }
            row.append(&move_up);

            let move_down = gtk::Button::builder()
                .icon_name("go-down-symbolic")
                .tooltip_text(match language {
                    AppLanguage::Portuguese => "Mover para baixo",
                    AppLanguage::English => "Move down",
                    AppLanguage::Spanish => "Mover hacia abajo",
                })
                .build();
            move_down.add_css_class("flat");
            move_down.add_css_class("circular");
            move_down.set_sensitive(position + 1 < count);
            {
                let weak = Rc::downgrade(self);
                let list = list.clone();
                let summary = summary.clone();
                let clear_upcoming = clear_upcoming.clone();
                let queue_popover = popover.clone();
                let id = entry.id;
                move_down.connect_clicked(move |_| {
                    let Some(controller) = weak.upgrade() else {
                        return;
                    };
                    let result = controller
                        .playback_queue_v2
                        .borrow_mut()
                        .move_entry(id, position.saturating_add(1));
                    if let Err(error) = result {
                        controller.show_toast(&error.to_string());
                        return;
                    }
                    controller.rebuild_queue_popover(
                        &list,
                        &summary,
                        &clear_upcoming,
                        &queue_popover,
                    );
                });
            }
            row.append(&move_down);

            let remove = gtk::Button::builder()
                .icon_name("user-trash-symbolic")
                .tooltip_text(match language {
                    AppLanguage::Portuguese => "Remover da fila",
                    AppLanguage::English => "Remove from queue",
                    AppLanguage::Spanish => "Quitar de la cola",
                })
                .build();
            remove.add_css_class("flat");
            remove.add_css_class("circular");
            remove.set_sensitive(!is_current);
            {
                let weak = Rc::downgrade(self);
                let row = row.clone();
                let list = list.clone();
                let summary = summary.clone();
                let clear_upcoming = clear_upcoming.clone();
                let queue_popover = popover.clone();
                let id = entry.id;
                remove.connect_clicked(move |button| {
                    button.set_sensitive(false);
                    row.add_css_class("queue2-row-leaving");

                    let weak = weak.clone();
                    let list = list.clone();
                    let summary = summary.clone();
                    let clear_upcoming = clear_upcoming.clone();
                    let queue_popover = queue_popover.clone();

                    glib::timeout_add_local_once(Duration::from_millis(150), move || {
                        let Some(controller) = weak.upgrade() else {
                            return;
                        };
                        let result = controller.playback_queue_v2.borrow_mut().remove(id);
                        if let Err(error) = result {
                            controller.show_toast(&error.to_string());
                            return;
                        }
                        controller.rebuild_queue_popover(
                            &list,
                            &summary,
                            &clear_upcoming,
                            &queue_popover,
                        );
                    });
                });
            }
            row.append(&remove);

            row.add_css_class("queue2-row-entering");
            list.append(&row);
            let entering_row = row.clone();
            glib::idle_add_local_once(move || {
                entering_row.remove_css_class("queue2-row-entering");
            });
        }

        if current_index.is_some_and(|position| position.saturating_add(1) >= count) {
            // queue2_interface_polish_v1: explicit end-of-queue state
            let end_state = gtk::Box::new(gtk::Orientation::Horizontal, 9);
            end_state.set_halign(gtk::Align::Fill);
            end_state.set_valign(gtk::Align::Center);
            end_state.add_css_class("queue2-end-state");

            let icon = gtk::Image::from_icon_name("emblem-ok-symbolic");
            icon.set_pixel_size(18);
            icon.add_css_class("queue2-end-icon");

            let label = gtk::Label::new(Some(match language {
                AppLanguage::Portuguese => "Fim da fila",
                AppLanguage::English => "End of queue",
                AppLanguage::Spanish => "Fin de la cola",
            }));
            label.set_xalign(0.0);
            label.set_hexpand(true);
            label.add_css_class("dim-label");

            end_state.append(&icon);
            end_state.append(&label);
            list.append(&end_state);
        }
    }

    fn show_footer_playback_queue(self: &Rc<Self>) {
        self.ensure_active_queue_v2();

        let popover = gtk::Popover::new();
        popover.set_has_arrow(true);
        popover.set_autohide(true);
        popover.set_position(gtk::PositionType::Top);
        popover.set_parent(&self.footer_now_playing);
        popover.add_css_class("queue-popover");
        popover.add_css_class("queue2-popover");
        self.apply_popup_visual_theme(&popover);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 10);
        content.set_margin_top(12);
        content.set_margin_bottom(12);
        content.set_margin_start(12);
        content.set_margin_end(12);
        content.set_size_request(520, -1);
        content.add_css_class("queue-popover-content");

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 10);

        let heading_text = gtk::Box::new(gtk::Orientation::Vertical, 2);
        heading_text.set_hexpand(true);

        let heading = gtk::Label::new(Some(match self.config.borrow().language {
            AppLanguage::Portuguese => "Fila de reprodução",
            AppLanguage::English => "Playback queue",
            AppLanguage::Spanish => "Cola de reproducción",
        }));
        heading.set_xalign(0.0);
        heading.add_css_class("title-3");

        let summary = gtk::Label::new(None);
        summary.set_xalign(0.0);
        summary.add_css_class("dim-label");
        summary.set_tooltip_text(Some(match self.config.borrow().language {
            AppLanguage::Portuguese => "Atalho de reordenação: Alt+↑ / Alt+↓",
            AppLanguage::English => "Reorder shortcut: Alt+↑ / Alt+↓",
            AppLanguage::Spanish => "Atajo para reordenar: Alt+↑ / Alt+↓",
        }));

        heading_text.append(&heading);
        heading_text.append(&summary);
        header.append(&heading_text);

        let clear_upcoming = gtk::Button::builder()
            .icon_name("edit-clear-all-symbolic")
            .tooltip_text(match self.config.borrow().language {
                AppLanguage::Portuguese => "Limpar próximas",
                AppLanguage::English => "Clear upcoming",
                AppLanguage::Spanish => "Limpiar próximas",
            })
            .build();
        clear_upcoming.add_css_class("flat");
        clear_upcoming.add_css_class("circular");
        header.append(&clear_upcoming);
        content.append(&header);

        let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        list.add_css_class("queue-popover-list");
        list.add_css_class("queue2-list");

        let scroll = gtk::ScrolledWindow::new();
        scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scroll.set_min_content_width(520);
        scroll.set_max_content_height(480);
        scroll.set_propagate_natural_height(true);
        scroll.set_child(Some(&list));
        scroll.add_css_class("queue-popover-scroll");
        content.append(&scroll);

        {
            let weak = Rc::downgrade(self);
            let list = list.clone();
            let summary = summary.clone();
            let clear_button = clear_upcoming.clone();
            let queue_popover = popover.clone();
            clear_upcoming.connect_clicked(move |_| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                controller.playback_queue_v2.borrow_mut().clear_upcoming();
                controller.rebuild_queue_popover(&list, &summary, &clear_button, &queue_popover);
            });
        }

        self.rebuild_queue_popover(&list, &summary, &clear_upcoming, &popover);
        popover.set_child(Some(&content));
        popover.popup();
    }

    fn sync_active_library(&self) {
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

    fn refresh_youtube_status(&self) {
        let Some(bridge) = self.youtube_bridge.clone() else {
            self.youtube_page.set_status(&YouTubeStatus::default());
            self.youtube_page.show_error(
                "YouTube Music runtime is missing. Run ./scripts/setup-youtube-runtime.sh for cargo run, or reinstall with ./install.sh --install-youtube.",
            );
            return;
        };
        let sender = self.background.sender();
        thread::spawn(move || {
            let _ = sender.send(BackgroundMessage::YouTubeStatus(bridge.status()));
        });
    }

    fn sync_youtube_library(&self, force: bool, notify: bool) -> bool {
        let Some(bridge) = self.youtube_bridge.clone() else {
            return false;
        };
        {
            let mut library = self.youtube_library.borrow_mut();
            if !library.connected || library.syncing || (library.synced && !force) {
                return false;
            }
            library.syncing = true;
        }
        let sender = self.background.sender();
        thread::spawn(move || {
            let _ = sender.send(BackgroundMessage::YouTubeLibrarySynced {
                notify,
                result: bridge.sync_library(),
            });
        });
        true
    }

    fn prefetch_youtube_playlist_cache(&self) {
        let Some(bridge) = self.youtube_bridge.clone() else {
            return;
        };
        if self.youtube_playlist_prefetching.get() {
            return;
        }
        let playlists = {
            let library = self.youtube_library.borrow();
            youtube_home_prefetch_candidates(&library)
        };
        if playlists.is_empty() {
            return;
        }

        self.youtube_playlist_prefetching.set(true);
        let sender = self.background.sender();
        thread::spawn(move || {
            // Playlist requests are independent. A small worker pool prevents the
            // previous sequential 10s + 10s + 10s startup behavior without
            // flooding YouTube or spawning an unbounded number of helpers.
            let worker_count = playlists.len().min(3);
            let work = Arc::new(Mutex::new(playlists.into_iter().collect::<VecDeque<_>>()));
            let (result_tx, result_rx) = mpsc::channel();
            let mut workers = Vec::with_capacity(worker_count);

            for _ in 0..worker_count {
                let bridge = bridge.clone();
                let work = work.clone();
                let result_tx = result_tx.clone();
                workers.push(thread::spawn(move || loop {
                    let playlist = match work.lock() {
                        Ok(mut queue) => queue.pop_front(),
                        Err(_) => None,
                    };
                    let Some(playlist) = playlist else {
                        break;
                    };

                    let browse_id = playlist.browse_id.clone();
                    let result = bridge.playlist(&playlist).map(|mut items| {
                        cache_items_for_browser(&mut items);
                        items
                    });
                    let _ = result_tx.send((playlist, browse_id, result));
                }));
            }
            drop(result_tx);

            let mut cached = HashMap::new();
            for (playlist, browse_id, result) in result_rx {
                match result {
                    Ok(items) if !items.is_empty() => {
                        cached.insert(browse_id, items);
                    }
                    Ok(_) => {}
                    Err(error)
                        if error.contains(
                            "No playable tracks were returned for this YouTube Music playlist",
                        ) => {}
                    Err(error) => {
                        eprintln!(
                            "Could not pre-cache YouTube playlist '{}': {error}",
                            playlist.title
                        );
                    }
                }
            }
            for worker in workers {
                let _ = worker.join();
            }

            let _ = sender.send(BackgroundMessage::YouTubePlaylistsCached(Ok(cached)));
        });
    }

    fn load_youtube_playlist_for_browser(&self, playlist: YouTubeItem) {
        let Some(bridge) = self.youtube_bridge.clone() else {
            self.show_toast("As dependências do YouTube Music não estão instaladas");
            return;
        };
        let browse_id = playlist.browse_id.clone();
        if browse_id.is_empty() {
            return;
        }

        let route = BrowserRoute::YouTubePlaylist {
            title: playlist.title.clone(),
            browse_id: browse_id.clone(),
        };
        let cached = self
            .youtube_library
            .borrow()
            .playlist_tracks
            .get(&browse_id)
            .map(|items| !items.is_empty())
            .unwrap_or(false);
        if cached {
            self.navigate_browser(route);
            return;
        }

        {
            let mut library = self.youtube_library.borrow_mut();
            library.playlist_tracks.remove(&browse_id);
            library.playlist_loading.insert(browse_id.clone());
        }
        // Change pages before starting or queueing the network request. The user
        // immediately sees the playlist title and a loading row instead of
        // remaining on the previous page for several seconds.
        self.navigate_browser(route);

        if self.youtube_playlist_loading.get() {
            self.youtube_pending_playlist.replace(Some(playlist));
            return;
        }

        let request_id = self.youtube_playlist_request_id.get().wrapping_add(1);
        self.youtube_playlist_request_id.set(request_id);
        self.youtube_playlist_loading.set(true);
        let sender = self.background.sender();
        thread::spawn(move || {
            let result = bridge.playlist(&playlist).map(|mut items| {
                cache_items_for_browser(&mut items);
                items
            });
            let _ = sender.send(BackgroundMessage::YouTubeBrowserPlaylist {
                request_id,
                playlist,
                result,
            });
        });
    }

    fn is_open_youtube_playlist(&self, browse_id: &str) -> bool {
        matches!(
            self.browser.route(),
            BrowserRoute::YouTubePlaylist {
                browse_id: current,
                ..
            } if current == browse_id
        )
    }

    fn load_youtube_collection_for_browser(&self, item: YouTubeItem) {
        let collection = YouTubeCollectionRoute::from_item(&item);
        let key = collection.key.clone();
        let route = if item.result_type == "artist" {
            BrowserRoute::YouTubeArtist(collection)
        } else {
            BrowserRoute::YouTubeAlbum(collection)
        };

        if item.result_type == "artist" {
            self.navigate_browser(route);

            let already_loading = self.youtube_library.borrow().artist_loading.contains(&key);
            if already_loading {
                return;
            }

            let Some(bridge) = self.youtube_bridge.clone() else {
                self.show_toast("As dependências do YouTube Music não estão instaladas");
                return;
            };

            self.youtube_library
                .borrow_mut()
                .artist_loading
                .insert(key.clone());

            let sender = self.background.sender();
            thread::spawn(move || {
                let result = resolve_youtube_collection_item(&bridge, &item, "artists")
                    .and_then(|resolved| bridge.artist_overview(&resolved))
                    .map(|mut overview| {
                        cache_items_for_browser(std::slice::from_mut(&mut overview.profile));
                        cache_items_for_browser(&mut overview.albums);
                        overview
                    });
                let _ = sender.send(BackgroundMessage::YouTubeArtistOverview { key, result });
            });
            return;
        }

        let cached = self
            .youtube_library
            .borrow()
            .collection_tracks
            .get(&key)
            .map(|items| !items.is_empty())
            .unwrap_or(false);
        if cached {
            self.navigate_browser(route);
            return;
        }

        let Some(bridge) = self.youtube_bridge.clone() else {
            self.show_toast("As dependências do YouTube Music não estão instaladas");
            return;
        };

        self.youtube_library
            .borrow_mut()
            .collection_loading
            .insert(key.clone());
        self.navigate_browser(route);

        let sender = self.background.sender();
        thread::spawn(move || {
            let result = resolve_youtube_collection_item(&bridge, &item, "albums")
                .and_then(|resolved| bridge.collection(&resolved))
                .map(|mut items| {
                    cache_items_for_browser(&mut items);
                    items
                });
            let _ = sender.send(BackgroundMessage::YouTubeBrowserCollection { item, key, result });
        });
    }

    fn is_open_youtube_collection(&self, key: &str) -> bool {
        match self.browser.route() {
            BrowserRoute::YouTubeAlbum(collection) | BrowserRoute::YouTubeArtist(collection) => {
                collection.key == key
            }
            _ => false,
        }
    }

    fn prefetch_youtube_collection_cache(&self) {
        let Some(bridge) = self.youtube_bridge.clone() else {
            return;
        };
        if self.youtube_collection_prefetching.get() {
            return;
        }

        let collections = {
            let library = self.youtube_library.borrow();
            let mut seen = HashSet::new();
            library
                .suggested_albums
                .iter()
                .take(6)
                .chain(library.suggested_artists.iter().take(6))
                .filter(|item| !item.browse_id.is_empty())
                .filter(|item| {
                    let key = youtube_collection_cache_key(item);
                    seen.insert(key.clone()) && !library.collection_tracks.contains_key(&key)
                })
                .cloned()
                .collect::<Vec<_>>()
        };
        if collections.is_empty() {
            return;
        }

        self.youtube_collection_prefetching.set(true);
        let sender = self.background.sender();
        thread::spawn(move || {
            let worker_count = collections.len().min(3);
            let work = Arc::new(Mutex::new(collections.into_iter().collect::<VecDeque<_>>()));
            let (result_tx, result_rx) = mpsc::channel();
            let mut workers = Vec::with_capacity(worker_count);

            for _ in 0..worker_count {
                let bridge = bridge.clone();
                let work = work.clone();
                let result_tx = result_tx.clone();
                workers.push(thread::spawn(move || loop {
                    let item = match work.lock() {
                        Ok(mut queue) => queue.pop_front(),
                        Err(_) => None,
                    };
                    let Some(item) = item else {
                        break;
                    };

                    let key = youtube_collection_cache_key(&item);
                    let result = bridge.collection(&item).map(|mut items| {
                        cache_items_for_browser(&mut items);
                        items
                    });
                    let _ = result_tx.send((item, key, result));
                }));
            }
            drop(result_tx);

            let mut cached = HashMap::new();
            for (item, key, result) in result_rx {
                match result {
                    Ok(items) if !items.is_empty() => {
                        cached.insert(key, items);
                    }
                    Ok(_) => {}
                    Err(error) => {
                        eprintln!(
                            "Could not pre-cache YouTube {} '{}': {error}",
                            item.result_type, item.title
                        );
                    }
                }
            }
            for worker in workers {
                let _ = worker.join();
            }

            let _ = sender.send(BackgroundMessage::YouTubeCollectionsCached(Ok(cached)));
        });
    }

    fn prefetch_home_artist_profiles(&self, force: bool) {
        let Some(bridge) = self.youtube_bridge.clone() else {
            return;
        };

        let limit = if force {
            self.browser.artist_display_limit()
        } else {
            12
        };

        let artists = {
            let mut library = self.youtube_library.borrow_mut();
            let mut entries = library.artists.iter().collect::<Vec<_>>();
            if force {
                entries.sort_by(|left, right| {
                    left.title.to_lowercase().cmp(&right.title.to_lowercase())
                });
            }

            let candidates = entries
                .into_iter()
                .take(limit)
                .filter_map(|entry| {
                    let key = youtube_collection_cache_key(&entry.source);
                    let missing = !library.artist_profiles.contains_key(&key);
                    let idle = !library.artist_loading.contains(&key);

                    ((force || missing) && idle).then(|| (key, entry.source.clone()))
                })
                .collect::<Vec<_>>();

            for (key, _) in &candidates {
                library.artist_loading.insert(key.clone());
            }

            candidates
        };

        if artists.is_empty() {
            return;
        }

        let sender = self.background.sender();
        thread::spawn(move || {
            let worker_count = artists.len().min(3);
            let work = Arc::new(Mutex::new(artists.into_iter().collect::<VecDeque<_>>()));
            let mut workers = Vec::with_capacity(worker_count);

            for _ in 0..worker_count {
                let bridge = bridge.clone();
                let work = work.clone();
                let sender = sender.clone();

                workers.push(thread::spawn(move || loop {
                    let next = match work.lock() {
                        Ok(mut queue) => queue.pop_front(),
                        Err(_) => None,
                    };
                    let Some((key, item)) = next else {
                        break;
                    };

                    let result = resolve_youtube_collection_item(&bridge, &item, "artists")
                        .and_then(|resolved| bridge.artist_overview(&resolved))
                        .map(|mut overview| {
                            cache_items_for_browser(std::slice::from_mut(&mut overview.profile));
                            cache_items_for_browser(&mut overview.albums);
                            overview
                        });

                    let _ = sender.send(BackgroundMessage::YouTubeArtistOverview { key, result });
                }));
            }

            for worker in workers {
                let _ = worker.join();
            }
        });
    }

    fn request_global_youtube_search(&self, query: String) {
        if query.trim().is_empty()
            || self.config.borrow().startup_source != Some(StartupSource::YouTube)
            || self.search_query.borrow().trim() != query.as_str()
        {
            return;
        }

        let Some(bridge) = self.youtube_bridge.clone() else {
            self.youtube_library.borrow_mut().search = YouTubeSearchResults {
                query,
                error: "As dependências do YouTube Music não estão instaladas".to_string(),
                ..YouTubeSearchResults::default()
            };
            self.refresh_browser();
            return;
        };

        let request_id = self.youtube_search_request_id.get().wrapping_add(1);
        self.youtube_search_request_id.set(request_id);
        self.youtube_library.borrow_mut().search = YouTubeSearchResults {
            query: query.clone(),
            loading: true,
            ..YouTubeSearchResults::default()
        };
        self.refresh_browser();

        let sender = self.background.sender();
        thread::spawn(move || {
            let filters = ["songs", "albums", "artists", "playlists"];
            let expected = filters.len();
            let (result_tx, result_rx) = mpsc::channel();
            let mut workers = Vec::with_capacity(expected);

            for filter in filters {
                let bridge = bridge.clone();
                let result_tx = result_tx.clone();
                let worker_query = query.clone();
                workers.push(thread::spawn(move || {
                    let result = bridge.search(&worker_query, filter);
                    let _ = result_tx.send((filter, result));
                }));
            }
            drop(result_tx);

            let mut categorized = YouTubeSearchResults {
                query: query.clone(),
                ..YouTubeSearchResults::default()
            };
            let mut errors = Vec::new();

            for (filter, result) in result_rx {
                match result {
                    Ok(items) => match filter {
                        "songs" => {
                            categorized.songs =
                                items.into_iter().filter(YouTubeItem::playable).collect()
                        }
                        "albums" => categorized.albums = items,
                        "artists" => categorized.artists = items,
                        "playlists" => categorized.playlists = items,
                        _ => {}
                    },
                    Err(error) => errors.push(format!("{filter}: {error}")),
                }
            }

            for worker in workers {
                let _ = worker.join();
            }

            let result = if errors.len() == expected {
                Err(errors.join(" | "))
            } else {
                if !errors.is_empty() {
                    categorized.error = errors.join(" | ");
                }
                Ok(categorized)
            };

            let _ = sender.send(BackgroundMessage::YouTubeGlobalSearch {
                request_id,
                query,
                result,
            });
        });
    }

    fn handle_youtube_events(&self) {
        while let Some(event) = self.youtube_page.try_recv() {
            let Some(bridge) = self.youtube_bridge.clone() else {
                self.youtube_page.show_error(
                    "YouTube Music runtime is missing. Run ./scripts/setup-youtube-runtime.sh for cargo run, or reinstall with ./install.sh --install-youtube.",
                );
                continue;
            };

            match event {
                YouTubePageEvent::SyncLibrary => {
                    if self.sync_youtube_library(true, true) {
                        self.youtube_page
                            .set_loading(true, "Sincronizando com o Nocky...");
                    } else {
                        self.show_toast("A biblioteca já está sendo sincronizada");
                    }
                }
                YouTubePageEvent::Search { query, filter } => {
                    self.youtube_page
                        .set_loading(true, "Buscando no YouTube Music...");
                    let sender = self.background.sender();
                    thread::spawn(move || {
                        let result = bridge.search(&query, &filter);
                        let _ = sender.send(BackgroundMessage::YouTubeItems {
                            title: format!("Resultados para \"{query}\""),
                            result,
                        });
                    });
                }
                YouTubePageEvent::Connect(raw) => {
                    self.youtube_page
                        .set_loading(true, "Validando sessão do navegador...");
                    let sender = self.background.sender();
                    thread::spawn(move || {
                        let _ =
                            sender.send(BackgroundMessage::YouTubeConnected(bridge.connect(&raw)));
                    });
                }
                YouTubePageEvent::Disconnect => {
                    self.youtube_page
                        .set_loading(true, "Desconectando conta...");
                    let sender = self.background.sender();
                    thread::spawn(move || {
                        let _ = sender
                            .send(BackgroundMessage::YouTubeDisconnected(bridge.disconnect()));
                    });
                }
                YouTubePageEvent::LoadLibrary => {
                    self.youtube_page
                        .set_loading(true, "Carregando sua biblioteca...");
                    let sender = self.background.sender();
                    thread::spawn(move || {
                        let _ = sender.send(BackgroundMessage::YouTubeItems {
                            title: "Sua biblioteca do YouTube Music".to_string(),
                            result: bridge.library(),
                        });
                    });
                }
                YouTubePageEvent::LoadLiked => {
                    self.youtube_page
                        .set_loading(true, "Carregando músicas curtidas...");
                    let sender = self.background.sender();
                    thread::spawn(move || {
                        let _ = sender.send(BackgroundMessage::YouTubeItems {
                            title: "Músicas curtidas".to_string(),
                            result: bridge.liked(),
                        });
                    });
                }
                YouTubePageEvent::LoadPlaylists => {
                    self.youtube_page
                        .set_loading(true, "Carregando playlists...");
                    let sender = self.background.sender();
                    thread::spawn(move || {
                        let _ = sender.send(BackgroundMessage::YouTubeItems {
                            title: "Suas playlists".to_string(),
                            result: bridge.playlists(),
                        });
                    });
                }
                YouTubePageEvent::OpenPlaylist(item) => {
                    let title = item.title.clone();
                    self.youtube_page
                        .set_loading(true, &format!("Carregando {title}..."));
                    let sender = self.background.sender();
                    thread::spawn(move || {
                        let _ = sender.send(BackgroundMessage::YouTubeItems {
                            title,
                            result: bridge.playlist(&item),
                        });
                    });
                }
                YouTubePageEvent::Activate { item, queue, index } => {
                    self.resolve_youtube_track(item, queue, index, false)
                }
            }
        }
    }

    fn set_lyrics_message(&self, message: &str) {
        self.lyrics.show_message(message, None);
    }

    fn install_actions(self: &Rc<Self>, app: &adw::Application) {
        let choose = gio::SimpleAction::new("choose-library", None);
        {
            let weak = Rc::downgrade(self);
            choose.connect_activate(move |_, _| {
                if let Some(controller) = weak.upgrade() {
                    controller.choose_library_folder();
                }
            });
        }
        app.add_action(&choose);

        let rescan = gio::SimpleAction::new("rescan", None);
        {
            let weak = Rc::downgrade(self);
            rescan.connect_activate(move |_, _| {
                if let Some(controller) = weak.upgrade() {
                    controller.scan_library();
                }
            });
        }
        app.add_action(&rescan);

        let download = gio::SimpleAction::new("download-lyrics", None);
        {
            let weak = Rc::downgrade(self);
            download.connect_activate(move |_, _| {
                if let Some(controller) = weak.upgrade() {
                    if let Some(item) = controller
                        .youtube_state
                        .borrow()
                        .as_ref()
                        .map(|state| state.item.clone())
                    {
                        controller.set_lyrics_message(
                            "Searching synchronized lyrics for this YouTube track…",
                        );
                        controller.request_youtube_lyrics(&item, true);
                        return;
                    }
                    let current = controller.state.borrow().current;
                    if let Some(index) = current {
                        controller.request_lyrics(index, true, true);
                    } else {
                        controller.show_toast("Selecione uma faixa primeiro");
                    }
                }
            });
        }
        app.add_action(&download);

        let toggle_auto = gio::SimpleAction::new("toggle-auto-lyrics", None);
        {
            let weak = Rc::downgrade(self);
            toggle_auto.connect_activate(move |_, _| {
                if let Some(controller) = weak.upgrade() {
                    let enabled = {
                        let mut config = controller.config.borrow_mut();
                        config.auto_download_lyrics = !config.auto_download_lyrics;
                        config.auto_download_lyrics
                    };
                    controller.save_config();
                    controller.show_toast(if enabled {
                        controller.tr(Message::AutomaticLyricsEnabled)
                    } else {
                        controller.tr(Message::AutomaticLyricsDisabled)
                    });
                    if enabled {
                        if let Some(item) = controller
                            .youtube_state
                            .borrow()
                            .as_ref()
                            .map(|state| state.item.clone())
                        {
                            controller.request_youtube_lyrics(&item, false);
                        } else if let Some(index) = controller.state.borrow().current {
                            controller.request_lyrics(index, false, false);
                        }
                    }
                }
            });
        }
        app.add_action(&toggle_auto);

        let focus_search = gio::SimpleAction::new("focus-search", None);
        {
            let weak = Rc::downgrade(self);
            focus_search.connect_activate(move |_, _| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                controller.close_settings_page();
                controller.search_button.set_active(true);
                controller.search_entry.grab_focus();
            });
        }
        app.add_action(&focus_search);

        let settings = gio::SimpleAction::new("settings", None);
        {
            let weak = Rc::downgrade(self);
            settings.connect_activate(move |_, _| {
                if let Some(controller) = weak.upgrade() {
                    controller.open_settings_page();
                }
            });
        }
        app.add_action(&settings);

        let shortcuts = gio::SimpleAction::new("shortcuts", None);
        {
            let weak = Rc::downgrade(self);
            shortcuts.connect_activate(move |_, _| {
                if let Some(controller) = weak.upgrade() {
                    controller.show_shortcuts_window();
                }
            });
        }
        app.add_action(&shortcuts);

        let about = gio::SimpleAction::new("about", None);
        {
            let weak = Rc::downgrade(self);
            about.connect_activate(move |_, _| {
                if let Some(controller) = weak.upgrade() {
                    controller.show_about_window();
                }
            });
        }
        app.add_action(&about);

        let quit = gio::SimpleAction::new("quit", None);
        {
            let app = app.clone();
            quit.connect_activate(move |_, _| app.quit());
        }
        app.add_action(&quit);

        app.set_accels_for_action("app.focus-search", &["<Primary>F"]);
        app.set_accels_for_action("app.settings", &["<Primary>comma"]);
        app.set_accels_for_action("app.choose-library", &["<Primary>O"]);
        app.set_accels_for_action("app.rescan", &["F5"]);
        app.set_accels_for_action("app.download-lyrics", &["<Primary>L"]);
        app.set_accels_for_action("app.quit", &["<Primary>Q"]);
    }

    fn queue_source_kind(source: StartupSource) -> QueueSourceKind {
        match source {
            StartupSource::Local => QueueSourceKind::Local,
            StartupSource::YouTube => QueueSourceKind::YouTube,
        }
    }

    fn report_queue_recovery(&self, source: QueueSourceKind, discarded_entries: usize) {
        if discarded_entries == 0 {
            return;
        }

        eprintln!(
            "Queue 2.0 recovery for {source:?} discarded {discarded_entries} unavailable entr{}",
            if discarded_entries == 1 { "y" } else { "ies" }
        );
    }

    fn persist_active_queue_to_source(&self, context: &str) -> bool {
        let source = self.active_queue_source.get();
        let snapshot = self.playback_queue_v2.borrow().snapshot();

        match queue_store::save_for(source, &snapshot) {
            Ok(()) => {
                self.queue_last_saved_snapshot.replace(snapshot);
                true
            }
            Err(error) => {
                eprintln!("Could not save {context} Queue 2.0 state for {source:?}: {error}");
                false
            }
        }
    }

    fn switch_active_queue_source(&self, source: QueueSourceKind) {
        if self.active_queue_source.get() == source {
            return;
        }

        if !self.persist_active_queue_to_source("outgoing") {
            self.show_toast("Não foi possível salvar a fila atual antes de trocar de fonte");
            return;
        }

        self.persist_playback_session_now();
        self.maybe_record_listening();
        let _ = self.player.pause();
        self.update_play_icons(false);
        self.playback_source.set(PlaybackSource::None);
        self.state.borrow_mut().current = None;
        self.youtube_state.borrow_mut().take();
        self.queue_v2_pending_entry.set(None);
        self.queue_dragged_entry.set(None);

        let queue_load = queue_store::load_for(source);
        self.report_queue_recovery(source, queue_load.discarded_entries);
        let snapshot = queue_load.queue.snapshot();

        self.playback_queue_v2.replace(queue_load.queue);
        self.queue_last_saved_snapshot.replace(snapshot);
        self.active_queue_source.set(source);

        let restored_session = playback_session::load_for(source);
        let restored_seconds = restored_session
            .as_ref()
            .map(|session| (session.position_us.max(0) as u64) / 1_000_000)
            .unwrap_or_default();
        let restored_shuffle = restored_session
            .as_ref()
            .is_some_and(|session| session.shuffle_enabled);
        let restored_repeat = restored_session
            .as_ref()
            .is_some_and(|session| session.repeat_enabled);
        self.restored_playback_session.replace(restored_session);
        self.playback_session_last_position_seconds
            .set(restored_seconds);
        self.playback_session_last_shuffle.set(restored_shuffle);
        self.playback_session_last_repeat.set(restored_repeat);
        self.shuffle_enabled.set(false);
        self.shuffle_button.set_active(false);
        self.footer_shuffle_button.set_active(false);
        self.repeat_button.set_active(false);
        self.footer_repeat_button.set_active(false);
        self.shuffle_navigation.borrow_mut().clear();
        self.playback_session_restore_attempts.set(0);
        self.pending_resume_position_us.set(None);
        self.startup_restore_autoplay.set(None);

        self.reset_shuffle_navigation(self.shuffle_enabled.get());
        self.publish_mpris_capabilities();
        self.update_footer_source();
        self.try_restore_playback_session();
    }

    fn apply_startup_source(self: &Rc<Self>) {
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

    fn set_startup_source(&self, source: StartupSource) {
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

    fn apply_source_aware_library_navigation(&self) {
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

    fn tr(&self, message: Message) -> &'static str {
        i18n::text(self.config.borrow().language, message)
    }

    // nocky_real_metadata_transition_v1
    fn set_footer_metadata(&self, title: &str, artist: &str) {
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

    fn open_current_artist_from_player(&self) {
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

    fn open_current_album_from_player(&self) {
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

    fn update_footer_source(&self) {
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

    fn apply_volume_icon(&self) {
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
    fn apply_compact_volume_expansion(&self) {
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

    fn apply_expressive_transport_effects(&self) {
        let enabled = {
            let config = self.config.borrow();
            config.expressive_transport_effects
                && config.visual_theme == VisualTheme::MaterialExpressive
        };

        self.main_transport_motion.set_effects_enabled(enabled);
        self.footer_transport_motion.set_effects_enabled(enabled);
    }

    fn apply_progress_style(&self) {
        let use_m3 = self.config.borrow().visual_theme == VisualTheme::MaterialExpressive;
        let child = if use_m3 { "m3" } else { "classic" };
        self.home_progress_stack.set_visible_child_name(child);
        self.footer_progress_stack.set_visible_child_name(child);

        let animate = use_m3 && self.player.is_playing();
        self.home_wave_progress.set_playing(animate);
        self.footer_progress.set_playing(animate);
    }

    fn apply_translations(&self) {
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
        self.page_switcher
            .set_labels(tr(Message::MusicTab), tr(Message::LyricsTab));
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

    fn apply_visual_theme(&self) {
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

    fn apply_footer_mode(&self) {
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
            footer_layout::FOOTER_COMPACT_CARD_MARGIN
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

    fn install_footer_adaptive(&self) {
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

    fn apply_home_player_visibility(&self) {
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

    fn apply_home_preferences(&self) {
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

    fn open_settings_page(&self) {
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

    fn close_settings_page(&self) {
        if self.content_stack.visible_child_name().as_deref() != Some("settings") {
            return;
        }
        self.content_stack.set_visible_child_name("main");
        if self.settings_button.is_active() {
            self.settings_button.set_active(false);
        }
        self.apply_footer_mode();
    }

    fn handle_settings_events(self: &Rc<Self>) {
        while let Some(event) = self.settings_page.try_recv() {
            self.apply_settings_event(event);
        }
    }

    fn apply_settings_event(self: &Rc<Self>, event: SettingsEvent) {
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
            SettingsEvent::NoctaliaThemeSync(active) => {
                self.config.borrow_mut().noctalia_theme_sync = active;
                self.save_config();
                self.apply_home_preferences();
            }
            SettingsEvent::ManageYouTube => self.show_youtube_settings_dialog(),
        }
    }

    fn show_youtube_settings_dialog(self: &Rc<Self>) {
        dialogs::present_youtube_settings(&self.window, self.youtube_page.root());
    }

    // themed_about_and_shortcuts_windows_v2
    fn apply_popup_visual_theme<W>(&self, widget: &W)
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

    fn show_about_window(&self) {
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

    fn show_shortcuts_window(&self) {
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

    fn show_onboarding_wizard(self: &Rc<Self>) {
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

    fn show_startup_source_dialog(self: &Rc<Self>, first_run: bool) {
        let language = self.config.borrow().language;
        let weak = Rc::downgrade(self);

        dialogs::present_startup_source(&self.window, language, first_run, move |source| {
            if let Some(controller) = weak.upgrade() {
                controller.set_startup_source(source);
            }
        });
    }

    fn load_saved_library(self: &Rc<Self>) {
        if self.config.borrow().music_directory.is_some() {
            self.scan_library();
        }
    }

    fn choose_library_folder(self: &Rc<Self>) {
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

    fn scan_library(&self) {
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

    fn apply_scanned_library(&self, data: Vec<TrackData>) {
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

    fn browser_playback_state(&self) -> BrowserPlaybackState {
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

    fn refresh_artist_directory(&self) {
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

    fn refresh_browser(&self) {
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

    fn navigate_browser(&self, route: BrowserRoute) {
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

    fn update_sidebar_active(&self, route: &BrowserRoute) {
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

    fn update_listening_history_context_from_route(&self) {
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

    fn handle_browser_events(&self) {
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

    fn play_local_collection(&self, kind: &str, title: &str) {
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

    fn load_youtube_collection_for_playback(&self, item: YouTubeItem, playlist: bool) {
        let Some(bridge) = self.youtube_bridge.clone() else {
            self.show_toast("As dependências do YouTube Music não estão instaladas");
            return;
        };

        let request_id = self
            .youtube_collection_play_request_id
            .get()
            .wrapping_add(1);
        self.youtube_collection_play_request_id.set(request_id);

        if playlist {
            if !item.browse_id.trim().is_empty() {
                self.youtube_library
                    .borrow_mut()
                    .playlist_loading
                    .insert(item.browse_id.clone());
            }
        } else {
            self.youtube_library
                .borrow_mut()
                .collection_loading
                .insert(youtube_collection_key("album", &item.title));
        }

        self.show_toast(if playlist {
            "Carregando playlist do YouTube Music…"
        } else {
            "Carregando álbum do YouTube Music…"
        });

        let sender = self.background.sender();
        thread::spawn(move || {
            let result = if playlist {
                bridge.playlist(&item)
            } else {
                bridge.collection(&item)
            }
            .map(|mut items| {
                cache_items_for_browser(&mut items);
                items
            });

            let _ = sender.send(BackgroundMessage::YouTubeCollectionPlaybackLoaded {
                request_id,
                item,
                playlist,
                result,
            });
        });
    }

    pub(crate) fn play_youtube_collection(&self, item: YouTubeItem, playlist: bool) {
        let kind = if playlist { "playlist" } else { "album" };
        let id = if item.browse_id.trim().is_empty() {
            item.title.to_lowercase()
        } else {
            item.browse_id.clone()
        };

        let items = {
            let library = self.youtube_library.borrow();
            if playlist {
                library
                    .playlist_tracks
                    .get(&item.browse_id)
                    .cloned()
                    .unwrap_or_default()
            } else {
                let key = youtube_collection_key("album", &item.title);
                library
                    .collection_tracks
                    .get(&key)
                    .cloned()
                    .unwrap_or_default()
            }
        };

        if items.is_empty() {
            self.load_youtube_collection_for_playback(item, playlist);
            return;
        }

        self.listening_history_context
            .replace(listening_history::PlaybackHistoryContext {
                kind: kind.to_string(),
                id,
                title: item.title.clone(),
            });
        self.pending_resume_position_us.set(None);
        self.resolve_youtube_track(items[0].clone(), items, 0, false);
    }

    fn playback_session_snapshot(&self) -> Option<PlaybackSession> {
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

    fn persist_playback_session_if_changed(&self) {
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
        if let Err(error) = playback_session::save_for(source, &session) {
            eprintln!("Could not save playback session for {source:?}: {error}");
        }
    }

    fn persist_playback_session_now(&self) {
        let source = self.active_queue_source.get();
        if let Some(session) = self.playback_session_snapshot() {
            if let Err(error) = playback_session::save_for(source, &session) {
                eprintln!("Could not save playback session for {source:?}: {error}");
            }
        } else if let Err(error) = playback_session::clear_for(source) {
            eprintln!("Could not clear playback session for {source:?}: {error}");
        }
    }

    fn try_restore_playback_session(&self) {
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

    fn apply_pending_resume_position(&self) {
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
                    .send(mpris::MprisUpdate::Position(position.max(0)));
            }
            Err(error) => {
                eprintln!("Could not restore playback position: {error}");
            }
        }
    }

    fn current_track_path(&self) -> Option<PathBuf> {
        let state = self.state.borrow();
        state
            .current
            .and_then(|index| state.tracks.get(index))
            .map(|track| track.path.clone())
    }

    fn select_track(&self, index: usize, autoplay: bool) {
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
        self.mpris.send(mpris::MprisUpdate::Position(0));
        self.mpris.send(mpris::MprisUpdate::Playback(if autoplay {
            mpris::MprisPlayback::Playing
        } else {
            mpris::MprisPlayback::Paused
        }));

        self.browser.select_track(index);

        if track.lyrics.is_empty() && self.config.borrow().auto_download_lyrics {
            self.request_lyrics(index, false, false);
        }
    }

    fn request_lyrics(&self, index: usize, notify: bool, force: bool) {
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
                lyrics_provider::LyricsLookup {
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
            let result =
                lyrics_provider::download_to_sidecar(&path, &lookup, force).map(|document| {
                    eprintln!(
                        "Lyrics loaded from {} ({})",
                        document.provider,
                        if document.synchronized {
                            "synchronized"
                        } else {
                            "plain fallback"
                        }
                    );
                });
            let _ = sender.send(BackgroundMessage::LyricsDownloaded {
                path,
                result,
                notify,
            });
        });
    }

    fn refresh_current_lyrics(&self) {
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

    fn set_youtube_favorite_visual_state(&self, active: bool) {
        self.favorite_icon
            .set_icon_name(Some("emblem-favorite-symbolic"));
        self.favorite_icon
            .set_opacity(if active { 0.98 } else { 0.28 });
        self.footer_favorite_icon
            .set_icon_name(Some("emblem-favorite-symbolic"));
        self.footer_favorite_icon
            .set_opacity(if active { 0.98 } else { 0.28 });

        for button in [&self.favorite_button, &self.footer_favorite_button] {
            if active {
                button.add_css_class("active");
            } else {
                button.remove_css_class("active");
            }
        }
    }

    fn current_youtube_item(&self) -> Option<YouTubeItem> {
        self.youtube_state
            .borrow()
            .as_ref()
            .map(|state| state.item.clone())
    }

    fn youtube_item_is_liked(&self, video_id: &str) -> bool {
        self.youtube_library
            .borrow()
            .liked
            .iter()
            .any(|item| item.video_id == video_id)
    }

    fn apply_youtube_like_cache(&self, item: &YouTubeItem, liked: bool) {
        let mut library = self.youtube_library.borrow_mut();
        library
            .liked
            .retain(|candidate| candidate.video_id != item.video_id);

        if liked {
            let mut stored = item.clone();
            if stored.result_type.is_empty() {
                stored.result_type = "song".to_string();
            }
            library.liked.insert(0, stored);
        }

        library.rebuild_collections();
        if let Err(error) = youtube::save_library_cache(&library) {
            eprintln!("Could not persist YouTube liked songs: {error}");
        }
    }

    fn toggle_youtube_favorite(&self) {
        let Some(item) = self.current_youtube_item() else {
            self.show_toast("Nenhuma música do YouTube Music está selecionada");
            return;
        };
        self.toggle_youtube_item_favorite(item);
    }

    fn toggle_youtube_item_favorite(&self, item: YouTubeItem) {
        if item.video_id.trim().is_empty() {
            self.show_toast("Esta música não possui um identificador válido do YouTube");
            return;
        }

        if !self.youtube_library.borrow().connected {
            self.show_toast("Conecte sua conta do YouTube Music para curtir músicas");
            return;
        }

        if self
            .youtube_like_pending
            .borrow()
            .contains_key(&item.video_id)
        {
            self.show_toast("Aguarde a confirmação da curtida anterior");
            return;
        }

        let Some(bridge) = self.youtube_bridge.clone() else {
            self.show_toast("As dependências do YouTube Music não estão instaladas");
            return;
        };

        let request_id = self.youtube_like_request_id.get().wrapping_add(1);
        self.youtube_like_request_id.set(request_id);
        self.youtube_like_pending
            .borrow_mut()
            .insert(item.video_id.clone(), request_id);

        let liked = !self.youtube_item_is_liked(&item.video_id);
        self.apply_youtube_like_cache(&item, liked);

        if self
            .current_youtube_item()
            .is_some_and(|current| current.video_id == item.video_id)
        {
            self.set_youtube_favorite_visual_state(liked);
        }
        self.refresh_browser();

        let sender = self.background.sender();
        thread::spawn(move || {
            let result = bridge.rate(&item.video_id, liked);
            let _ = sender.send(BackgroundMessage::YouTubeRatingChanged {
                request_id,
                item,
                liked,
                result,
            });
        });
    }

    fn toggle_favorite(&self) {
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

    fn update_favorite_icon(&self, path: &std::path::Path) {
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

    // queue2_playback_bridge_v1
    fn enqueue_browser_media(&self, media: QueueMedia, play_next: bool) {
        let expected = self.active_queue_source.get();
        if media.source.kind() != expected {
            eprintln!(
                "Rejected Queue 2.0 enqueue: active source is {expected:?}, media source is {:?}",
                media.source.kind()
            );
            self.show_toast("Esta faixa pertence a outra fonte de reprodução");
            return;
        }

        self.ensure_active_queue_v2();
        let title = media.title.clone();

        if play_next {
            self.playback_queue_v2.borrow_mut().insert_next(media);
        } else {
            self.playback_queue_v2.borrow_mut().append(media);
        }

        let message = match (self.config.borrow().language, play_next) {
            (AppLanguage::Portuguese, true) => format!("‘{title}’ será reproduzida em seguida"),
            (AppLanguage::Portuguese, false) => format!("‘{title}’ foi adicionada ao fim da fila"),
            (AppLanguage::English, true) => format!("‘{title}’ will play next"),
            (AppLanguage::English, false) => format!("‘{title}’ was added to the queue"),
            (AppLanguage::Spanish, true) => format!("‘{title}’ se reproducirá a continuación"),
            (AppLanguage::Spanish, false) => {
                format!("‘{title}’ se añadió al final de la cola")
            }
        };
        self.show_toast(&message);
    }

    fn enqueue_local_track(&self, index: usize, play_next: bool) {
        let media = self
            .state
            .borrow()
            .tracks
            .get(index)
            .map(Self::local_queue_media);
        if let Some(media) = media {
            self.enqueue_browser_media(media, play_next);
        }
    }

    fn enqueue_youtube_track(&self, item: &YouTubeItem, play_next: bool) {
        if item.playable() {
            self.enqueue_browser_media(Self::youtube_queue_media(item), play_next);
        }
    }

    fn enqueue_media_collection(&self, media: Vec<QueueMedia>, play_next: bool, title: &str) {
        if media.is_empty() {
            return;
        }

        let expected = self.active_queue_source.get();
        if media.iter().any(|item| item.source.kind() != expected) {
            eprintln!("Rejected Queue 2.0 collection enqueue: active source is {expected:?}");
            self.show_toast("Esta coleção pertence a outra fonte de reprodução");
            return;
        }

        self.ensure_active_queue_v2();
        let count = media.len();

        if play_next {
            let mut queue = self.playback_queue_v2.borrow_mut();
            for item in media.into_iter().rev() {
                queue.insert_next(item);
            }
        } else {
            let mut queue = self.playback_queue_v2.borrow_mut();
            for item in media {
                queue.append(item);
            }
        }

        let message = match (self.config.borrow().language, play_next) {
            (AppLanguage::Portuguese, true) => {
                format!("‘{title}’ ({count} faixas) será reproduzido em seguida")
            }
            (AppLanguage::Portuguese, false) => {
                format!("‘{title}’ ({count} faixas) foi adicionado ao fim da fila")
            }
            (AppLanguage::English, true) => {
                format!("‘{title}’ ({count} tracks) will play next")
            }
            (AppLanguage::English, false) => {
                format!("‘{title}’ ({count} tracks) was added to the queue")
            }
            (AppLanguage::Spanish, true) => {
                format!("‘{title}’ ({count} pistas) se reproducirá a continuación")
            }
            (AppLanguage::Spanish, false) => {
                format!("‘{title}’ ({count} pistas) se añadió al final de la cola")
            }
        };
        self.show_toast(&message);
    }

    fn enqueue_local_collection(&self, kind: &str, title: &str, play_next: bool) {
        let indices = if kind == "playlist" {
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
            let mut indices = state
                .tracks
                .iter()
                .enumerate()
                .filter_map(|(index, track)| {
                    track.album.eq_ignore_ascii_case(title).then_some(index)
                })
                .collect::<Vec<_>>();
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
            indices
        };

        let media = {
            let state = self.state.borrow();
            indices
                .iter()
                .filter_map(|index| state.tracks.get(*index))
                .map(Self::local_queue_media)
                .collect::<Vec<_>>()
        };

        if media.is_empty() {
            self.show_toast(if kind == "playlist" {
                "Esta playlist local ainda está vazia"
            } else {
                "Nenhuma faixa local foi encontrada para este álbum"
            });
            return;
        }

        self.enqueue_media_collection(media, play_next, title);
    }

    fn enqueue_youtube_collection(&self, item: &YouTubeItem, playlist: bool, play_next: bool) {
        let (items, collection_cover) = {
            let library = self.youtube_library.borrow();
            let items = if playlist {
                library
                    .playlist_tracks
                    .get(&item.browse_id)
                    .cloned()
                    .unwrap_or_default()
            } else {
                let key = youtube_collection_key("album", &item.title);
                library
                    .collection_tracks
                    .get(&key)
                    .cloned()
                    .unwrap_or_default()
            };

            let collection_cover = item.cached_cover().map(Path::to_path_buf).or_else(|| {
                (!playlist)
                    .then(|| {
                        library
                            .albums
                            .iter()
                            .find(|entry| {
                                (!item.browse_id.trim().is_empty()
                                    && entry.source.browse_id == item.browse_id)
                                    || entry.title.eq_ignore_ascii_case(&item.title)
                            })
                            .and_then(|entry| entry.cached_cover().map(Path::to_path_buf))
                    })
                    .flatten()
            });

            (items, collection_cover)
        };

        let media = items
            .iter()
            .filter(|track| track.playable())
            .map(|track| {
                Self::youtube_queue_media_with_fallback(track, collection_cover.as_deref())
            })
            .collect::<Vec<_>>();

        if media.is_empty() {
            self.load_youtube_collection_for_queue(item.clone(), playlist, play_next);
            return;
        }

        self.enqueue_media_collection(media, play_next, &item.title);
    }

    fn load_youtube_collection_for_queue(
        &self,
        item: YouTubeItem,
        playlist: bool,
        play_next: bool,
    ) {
        let Some(bridge) = self.youtube_bridge.clone() else {
            self.show_toast("As dependências do YouTube Music não estão instaladas");
            return;
        };

        let request_id = self
            .youtube_collection_queue_request_id
            .get()
            .wrapping_add(1);
        self.youtube_collection_queue_request_id.set(request_id);

        if playlist {
            if !item.browse_id.trim().is_empty() {
                self.youtube_library
                    .borrow_mut()
                    .playlist_loading
                    .insert(item.browse_id.clone());
            }
        } else {
            self.youtube_library
                .borrow_mut()
                .collection_loading
                .insert(youtube_collection_key("album", &item.title));
        }

        let message = match (self.config.borrow().language, play_next) {
            (AppLanguage::Portuguese, true) => "Carregando coleção para reproduzir em seguida…",
            (AppLanguage::Portuguese, false) => "Carregando coleção para adicionar à fila…",
            (AppLanguage::English, true) => "Loading collection to play next…",
            (AppLanguage::English, false) => "Loading collection to add to queue…",
            (AppLanguage::Spanish, true) => "Cargando colección para reproducir a continuación…",
            (AppLanguage::Spanish, false) => "Cargando colección para añadirla a la cola…",
        };
        self.show_toast(message);
        self.refresh_browser();

        let sender = self.background.sender();
        thread::spawn(move || {
            let result = if playlist {
                bridge.playlist(&item)
            } else {
                bridge.collection(&item)
            }
            .map(|mut items| {
                cache_items_for_browser(&mut items);
                items
            });

            let _ = sender.send(BackgroundMessage::YouTubeCollectionQueueLoaded {
                request_id,
                item,
                playlist,
                play_next,
                result,
            });
        });
    }

    fn local_queue_media(track: &Track) -> QueueMedia {
        QueueMedia::local(
            track.path.clone(),
            track.title.clone(),
            track.artist.clone(),
            track.album.clone(),
            track.duration_seconds,
            track.cover_path.clone(),
        )
    }

    fn youtube_queue_media(item: &YouTubeItem) -> QueueMedia {
        Self::youtube_queue_media_with_fallback(item, None)
    }

    fn youtube_queue_media_with_fallback(
        item: &YouTubeItem,
        fallback_cover: Option<&Path>,
    ) -> QueueMedia {
        let cover_path = item
            .cached_cover()
            .map(Path::to_path_buf)
            .or_else(|| fallback_cover.map(Path::to_path_buf));

        QueueMedia::youtube(
            item.video_id.clone(),
            item.title.clone(),
            item.artist.clone(),
            item.album.clone(),
            item.duration_seconds,
            cover_path,
        )
    }

    fn sync_local_queue_v2(&self, sequence: &[usize], selected: usize) {
        let (media, selected_position) = {
            let state = self.state.borrow();
            let media = sequence
                .iter()
                .filter_map(|index| state.tracks.get(*index))
                .map(Self::local_queue_media)
                .collect::<Vec<_>>();
            let selected_position = sequence.iter().position(|index| *index == selected);
            (media, selected_position)
        };

        let incoming_keys = media
            .iter()
            .map(|item| item.source.stable_key())
            .collect::<Vec<_>>();
        let current_keys = self
            .playback_queue_v2
            .borrow()
            .entries()
            .iter()
            .map(|entry| entry.media.source.stable_key())
            .collect::<Vec<_>>();

        let mut queue = self.playback_queue_v2.borrow_mut();
        if incoming_keys != current_keys {
            queue.replace(media, selected_position);
        } else if let Some(position) = selected_position {
            queue.select_index(position);
        }
    }

    fn sync_youtube_queue_v2(&self, items: &[YouTubeItem], selected: usize) {
        let media = items
            .iter()
            .filter(|item| item.playable())
            .map(Self::youtube_queue_media)
            .collect::<Vec<_>>();
        let selected_video_id = items.get(selected).map(|item| item.video_id.as_str());
        let selected_position = selected_video_id.and_then(|video_id| {
            media.iter().position(|item| {
                matches!(
                    &item.source,
                    QueueSource::YouTube {
                        video_id: candidate
                    } if candidate == video_id
                )
            })
        });

        let incoming_keys = media
            .iter()
            .map(|item| item.source.stable_key())
            .collect::<Vec<_>>();
        let current_keys = self
            .playback_queue_v2
            .borrow()
            .entries()
            .iter()
            .map(|entry| entry.media.source.stable_key())
            .collect::<Vec<_>>();

        let mut queue = self.playback_queue_v2.borrow_mut();
        if incoming_keys != current_keys {
            queue.replace(media, selected_position);
        } else if let Some(position) = selected_position {
            queue.select_index(position);
        }
    }

    fn ensure_local_queue_v2(&self, selected: usize) {
        let selected_path = {
            let state = self.state.borrow();
            state.tracks.get(selected).map(|track| track.path.clone())
        };
        let Some(selected_path) = selected_path else {
            return;
        };

        let matching_id = self
            .playback_queue_v2
            .borrow()
            .entries()
            .iter()
            .find_map(|entry| match &entry.media.source {
                QueueSource::Local { path } if path == &selected_path => Some(entry.id),
                _ => None,
            });

        if let Some(id) = matching_id {
            let _ = self.playback_queue_v2.borrow_mut().select(id);
            return;
        }

        let sequence = self.playback_sequence();
        self.sync_local_queue_v2(&sequence, selected);
    }

    fn ensure_active_queue_v2(&self) {
        let playback_kind = match self.playback_source.get() {
            PlaybackSource::Local => Some(QueueSourceKind::Local),
            PlaybackSource::YouTube => Some(QueueSourceKind::YouTube),
            PlaybackSource::None => None,
        };
        if playback_kind.is_some_and(|kind| kind != self.active_queue_source.get()) {
            return;
        }

        match self.playback_source.get() {
            PlaybackSource::Local => {
                if let Some(selected) = self.state.borrow().current {
                    self.ensure_local_queue_v2(selected);
                }
            }
            PlaybackSource::YouTube => {
                let snapshot = {
                    let state = self.youtube_state.borrow();
                    state.as_ref().map(|state| {
                        (
                            state.queue.clone(),
                            state.current,
                            state.item.video_id.clone(),
                        )
                    })
                };
                let Some((items, current, video_id)) = snapshot else {
                    return;
                };

                let matching_id =
                    self.playback_queue_v2
                        .borrow()
                        .entries()
                        .iter()
                        .find_map(|entry| match &entry.media.source {
                            QueueSource::YouTube {
                                video_id: candidate,
                            } if candidate == &video_id => Some(entry.id),
                            _ => None,
                        });

                if let Some(id) = matching_id {
                    let _ = self.playback_queue_v2.borrow_mut().select(id);
                } else {
                    self.sync_youtube_queue_v2(&items, current);
                }
            }
            PlaybackSource::None => {}
        }
    }

    fn reset_shuffle_navigation(&self, enabled: bool) {
        let mut rng = self.rng_state.get();

        if enabled {
            let queue = self.playback_queue_v2.borrow();
            self.shuffle_navigation.borrow_mut().reset(
                queue.entries(),
                queue.current_id(),
                &mut rng,
            );
        } else {
            self.shuffle_navigation.borrow_mut().clear();
        }

        self.rng_state.set(rng);
    }

    fn next_queue_entry_id(&self) -> Option<QueueEntryId> {
        let queue = self.playback_queue_v2.borrow();

        if !self.shuffle_enabled.get() {
            return match queue.current_index() {
                Some(position) => queue.entries().get(position + 1).map(|entry| entry.id),
                None => queue.entries().first().map(|entry| entry.id),
            };
        }

        let mut rng = self.rng_state.get();
        let next = self.shuffle_navigation.borrow_mut().next(
            queue.entries(),
            queue.current_id(),
            &mut rng,
        );
        self.rng_state.set(rng);
        next
    }

    fn previous_queue_entry_id(&self) -> Option<QueueEntryId> {
        let queue = self.playback_queue_v2.borrow();

        if !self.shuffle_enabled.get() {
            return queue
                .current_index()
                .and_then(|position| position.checked_sub(1))
                .and_then(|position| queue.entries().get(position))
                .map(|entry| entry.id);
        }

        let mut rng = self.rng_state.get();
        let previous = self.shuffle_navigation.borrow_mut().previous(
            queue.entries(),
            queue.current_id(),
            &mut rng,
        );
        self.rng_state.set(rng);
        previous
    }

    fn play_queue_entry(&self, id: QueueEntryId, autoplay: bool) {
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

    fn prepare_playback_queue(&self, selected: usize) {
        let mut sequence = self.browser.visible_indices();
        if sequence.is_empty() || !sequence.contains(&selected) {
            sequence = (0..self.state.borrow().tracks.len()).collect();
        }
        self.state.borrow_mut().playback_queue = sequence.clone();
        self.sync_local_queue_v2(&sequence, selected);
    }

    fn playback_sequence(&self) -> Vec<usize> {
        let state = self.state.borrow();
        if !state.playback_queue.is_empty()
            && state
                .current
                .is_none_or(|current| state.playback_queue.contains(&current))
        {
            return state.playback_queue.clone();
        }
        drop(state);

        let visible = self.browser.visible_indices();
        if !visible.is_empty() {
            return visible;
        }
        match self.browser.route() {
            BrowserRoute::Albums
            | BrowserRoute::Artists
            | BrowserRoute::Playlists
            | BrowserRoute::YouTubeAlbum(_)
            | BrowserRoute::YouTubeArtist(_)
            | BrowserRoute::YouTubePlaylist { .. } => {
                (0..self.state.borrow().tracks.len()).collect()
            }
            _ => visible,
        }
    }

    fn toggle_playback(&self) {
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

    fn next_track(&self) -> bool {
        self.ensure_active_queue_v2();

        let Some(next) = self.next_queue_entry_id() else {
            return false;
        };

        self.play_queue_entry(next, true);
        true
    }

    fn previous_track(&self) {
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

    fn play_current(&self) {
        match self.player.play() {
            Ok(()) => {
                self.update_play_icons(true);
                self.mpris
                    .send(mpris::MprisUpdate::Playback(mpris::MprisPlayback::Playing));
            }
            Err(error) => self.show_error(&error),
        }
    }

    fn pause_current(&self) {
        self.maybe_record_listening();

        match self.player.pause() {
            Ok(()) => {
                self.update_play_icons(false);
                self.mpris
                    .send(mpris::MprisUpdate::Playback(mpris::MprisPlayback::Paused));
            }
            Err(error) => self.show_error(&error),
        }
    }

    fn handle_playback_events(&self) {
        while let Some(event) = self.player.try_recv() {
            match event {
                PlaybackEvent::EndOfStream => self.handle_end_of_stream(),
                PlaybackEvent::DurationChanged => {
                    self.publish_mpris_capabilities();
                    self.resume_youtube_after_recovery();
                    self.apply_pending_resume_position();
                }
                PlaybackEvent::ClockLost => {
                    if let Err(error) = self.player.recover_clock() {
                        eprintln!("Could not recover GStreamer clock after resume: {error}");
                    }
                }
                PlaybackEvent::Spectrum(values) => self.visualizer.set_values(&values),
                PlaybackEvent::Error(error) => {
                    if self.youtube_recovery_in_progress.get() {
                        eprintln!(
                            "Ignoring follow-up GStreamer error during stream refresh: {}",
                            redact_stream_url(&error)
                        );
                        continue;
                    }
                    if self.try_recover_youtube_stream(&error) {
                        continue;
                    }

                    eprintln!("Nocky playback error: {}", redact_stream_url(&error));
                    self.update_play_icons(false);
                    self.mpris
                        .send(mpris::MprisUpdate::Playback(mpris::MprisPlayback::Stopped));
                    self.show_error(playback_error_message(&error));
                }
            }
        }
    }

    fn handle_end_of_stream(&self) {
        self.maybe_record_listening();
        self.ensure_active_queue_v2();

        let repeat_one = self.repeat_button.is_active();
        let next = if repeat_one {
            None
        } else {
            self.next_queue_entry_id()
        };

        match queue_end_action(repeat_one, next) {
            QueueEndAction::RepeatCurrent => {
                self.seek_to(0, true);
                self.play_current();
            }
            QueueEndAction::Play(id) => self.play_queue_entry(id, true),
            QueueEndAction::Stop => {
                let _ = self.player.pause();
                self.update_play_icons(false);
                self.mpris
                    .send(mpris::MprisUpdate::Playback(mpris::MprisPlayback::Stopped));
            }
        }
    }

    fn handle_mpris_commands(&self) {
        while let Ok(command) = self.mpris.commands.try_recv() {
            match command {
                mpris::MprisCommand::Ready => {}
                mpris::MprisCommand::Error(error) => {
                    eprintln!("Nocky MPRIS bridge error: {error}");
                }
                mpris::MprisCommand::Raise => self.window.present(),
                mpris::MprisCommand::Quit => {
                    if let Some(application) = self.window.application() {
                        application.quit();
                    }
                }
                mpris::MprisCommand::Play => {
                    if self.playback_source.get() == PlaybackSource::YouTube
                        && self.youtube_state.borrow().is_some()
                    {
                        self.play_current();
                    } else if self.state.borrow().current.is_none() {
                        let queued = self.initial_queue_entry_id();
                        if let Some(id) = queued {
                            self.play_queue_entry(id, true);
                            continue;
                        }

                        let sequence = self.playback_sequence();
                        if let Some(index) = sequence.first().copied() {
                            self.state.borrow_mut().playback_queue = sequence;
                            self.select_track(index, true);
                        } else if !self.state.borrow().tracks.is_empty() {
                            let sequence =
                                (0..self.state.borrow().tracks.len()).collect::<Vec<_>>();
                            self.state.borrow_mut().playback_queue = sequence;
                            self.select_track(0, true);
                        }
                    } else {
                        self.play_current();
                    }
                }
                mpris::MprisCommand::Pause => self.pause_current(),
                mpris::MprisCommand::PlayPause => self.toggle_playback(),
                mpris::MprisCommand::Stop => {
                    self.pause_current();
                    self.seek_to(0, true);
                    self.mpris
                        .send(mpris::MprisUpdate::Playback(mpris::MprisPlayback::Stopped));
                }
                mpris::MprisCommand::Next => {
                    self.next_track();
                }
                mpris::MprisCommand::Previous => self.previous_track(),
                mpris::MprisCommand::Seek(offset) => {
                    let position = self.player.position_us().saturating_add(offset);
                    self.seek_to(position, true);
                }
                mpris::MprisCommand::SetPosition { track_id, position } => {
                    if self.current_mpris_track_id().as_deref() == Some(track_id.as_str()) {
                        self.seek_to(position, true);
                    }
                }
                mpris::MprisCommand::SetLoop(enabled) => {
                    if self.repeat_button.is_active() != enabled {
                        self.repeat_button.set_active(enabled);
                    }
                }
                mpris::MprisCommand::SetShuffle(enabled) => {
                    if self.shuffle_button.is_active() != enabled {
                        self.shuffle_button.set_active(enabled);
                    }
                }
                mpris::MprisCommand::SetVolume(value) => {
                    let value = value.clamp(0.0, 1.0);
                    if (self.volume.value() - value).abs() > f64::EPSILON {
                        self.volume.set_value(value);
                    }
                }
            }
        }
    }

    fn seek_to(&self, position: i64, announce: bool) {
        if !self.player.is_seekable() {
            return;
        }

        let duration = self.player.duration_us().max(0);
        let position = if duration > 0 {
            position.clamp(0, duration)
        } else {
            position.max(0)
        };

        if let Err(error) = self.player.seek(position) {
            self.show_error(&error);
            return;
        }
        self.last_mpris_position.set(position);
        if announce {
            self.mpris.send(mpris::MprisUpdate::Seeked(position));
        } else {
            self.mpris.send(mpris::MprisUpdate::Position(position));
        }
    }

    fn current_mpris_track_id(&self) -> Option<String> {
        if self.playback_source.get() == PlaybackSource::YouTube {
            return self
                .youtube_state
                .borrow()
                .as_ref()
                .map(|state| mpris_youtube_track_id(&state.item.video_id));
        }
        let state = self.state.borrow();
        state
            .current
            .and_then(|index| state.tracks.get(index))
            .map(|track| mpris_track_id(&track.path))
    }

    fn publish_mpris_track(&self, track: &Track) {
        let length_us = track
            .duration_seconds
            .saturating_mul(1_000_000)
            .min(i64::MAX as u64) as i64;
        let art_url = track
            .cover_path
            .as_ref()
            .map(|path| gio::File::for_path(path).uri().to_string());
        let url = Some(track.file.uri().to_string());

        self.mpris
            .send(mpris::MprisUpdate::Metadata(mpris::MprisTrack {
                track_id: mpris_track_id(&track.path),
                title: track.title.clone(),
                artist: track.artist.clone(),
                album: track.album.clone(),
                length_us,
                art_url,
                url,
            }));
        self.publish_mpris_capabilities();
    }

    fn publish_mpris_capabilities(&self) {
        let state = self.state.borrow();
        let has_youtube = self.youtube_state.borrow().is_some();
        let has_tracks = !state.tracks.is_empty() || has_youtube;
        let can_seek = state
            .current
            .and_then(|index| state.tracks.get(index))
            .is_some_and(|track| track.duration_seconds > 0)
            || has_youtube
            || self.player.is_seekable();
        drop(state);

        self.mpris.send(mpris::MprisUpdate::Capabilities {
            has_tracks,
            can_seek,
        });
    }

    fn update_play_icons(&self, playing: bool) {
        self.player_view.set_playing(playing);
        let icon = if playing {
            "media-playback-pause-symbolic"
        } else {
            "media-playback-start-symbolic"
        };
        self.play_icon.set_icon_name(Some(icon));
        self.hero_play_icon.set_icon_name(Some(icon));
        self.player_view
            .set_visualizer_active(playing && self.visualizer.widget().is_visible());
        let animate_m3 =
            playing && self.config.borrow().visual_theme == VisualTheme::MaterialExpressive;
        self.home_wave_progress.set_playing(animate_m3);
        self.footer_progress.set_playing(animate_m3);

        if matches!(self.browser.route(), BrowserRoute::All) {
            self.refresh_browser();
        }
    }

    fn begin_listening_session(&self, id: String) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        self.listening_session_id
            .replace(Some(format!("{id}:{nonce}")));
        self.listening_session_last_saved_seconds.set(0);
    }

    fn maybe_record_listening(&self) {
        let listened_seconds = (self.player.position_us().max(0) / 1_000_000) as u64;
        let duration_seconds = (self.player.duration_us().max(0) / 1_000_000) as u64;
        let completed = duration_seconds > 0
            && listened_seconds.saturating_mul(100) >= duration_seconds.saturating_mul(90);

        if listened_seconds < 30 && !completed {
            return;
        }

        let previous = self.listening_session_last_saved_seconds.get();
        let first_checkpoint = previous == 0;
        let checkpoint_due = first_checkpoint || listened_seconds >= previous.saturating_add(15);
        if !completed && !checkpoint_due {
            return;
        }

        let Some(session_id) = self.listening_session_id.borrow().clone() else {
            return;
        };

        let updated = match self.playback_source.get() {
            PlaybackSource::Local => {
                let state = self.state.borrow();
                let Some(index) = state.current else {
                    return;
                };
                let Some(track) = state.tracks.get(index) else {
                    return;
                };
                self.listening_history
                    .borrow_mut()
                    .record_playback_progress(
                        session_id,
                        track.path.to_string_lossy().into_owned(),
                        track.title.clone(),
                        track.artist.clone(),
                        track.album.clone(),
                        ListeningSource::Local,
                        listened_seconds,
                        listened_seconds,
                        duration_seconds.max(track.duration_seconds),
                        self.listening_history_context.borrow().clone(),
                        completed,
                    )
            }
            PlaybackSource::YouTube => {
                let state = self.youtube_state.borrow();
                let Some(state) = state.as_ref() else {
                    return;
                };
                self.listening_history
                    .borrow_mut()
                    .record_playback_progress(
                        session_id,
                        state.item.video_id.clone(),
                        state.item.title.clone(),
                        state.item.artist.clone(),
                        state.item.album.clone(),
                        ListeningSource::YouTube,
                        listened_seconds,
                        listened_seconds,
                        duration_seconds,
                        self.listening_history_context.borrow().clone(),
                        completed,
                    )
            }
            PlaybackSource::None => false,
        };

        if updated {
            self.listening_session_last_saved_seconds
                .set(listened_seconds);

            if first_checkpoint || completed {
                self.refresh_browser();
            }
        }
    }

    fn refresh_progress(&self) {
        self.apply_pending_resume_position();

        self.maybe_record_listening();
        let timestamp = self.player.position_us().max(0);
        let duration = self.player.duration_us().max(0);
        let fraction = if duration > 0 {
            timestamp as f64 / duration as f64
        } else {
            0.0
        };

        self.updating_progress.set(true);
        self.progress.set_value(fraction.clamp(0.0, 1.0));
        self.footer_traditional_progress
            .set_value(fraction.clamp(0.0, 1.0));
        self.home_wave_progress.set_fraction(fraction);
        self.footer_progress.set_fraction(fraction);
        self.updating_progress.set(false);
        let elapsed = format_time(timestamp);
        let duration_text = format_time(duration);
        self.elapsed.set_text(&elapsed);
        self.duration.set_text(&duration_text);
        self.footer_elapsed.set_text(&elapsed);
        self.footer_duration.set_text(&duration_text);
        self.highlight_lyric(timestamp);

        let previous = self.last_mpris_position.get();
        if previous < 0 || (timestamp - previous).abs() >= 500_000 {
            self.last_mpris_position.set(timestamp);
            self.mpris.send(mpris::MprisUpdate::Position(timestamp));
        }
    }

    fn rebuild_lyrics(&self, track: &Track) {
        if track.lyrics.is_empty() {
            let automatic = self.config.borrow().auto_download_lyrics;
            self.lyrics.show_state(
                "Nenhuma letra sincronizada disponível ainda",
                Some(if automatic {
                    "Automatic LRCLIB lookup is enabled. Use the menu to retry whenever needed."
                } else {
                    "Use the menu to download lyrics, or place a matching .lrc file beside the song."
                }),
                "No synchronized lyrics available yet",
                Some(if automatic {
                    "Automatic LRCLIB lookup is enabled. You can also open the Lyrics page for the full view."
                } else {
                    "Use the menu to download lyrics, or open the Lyrics page for the full view."
                }),
            );
            return;
        }

        self.lyrics.set_lines(&track.lyrics);
    }

    fn rebuild_youtube_lyrics(&self, lyrics: &[LyricLine]) {
        if lyrics.is_empty() {
            self.set_lyrics_message("No synchronized lyrics available for this YouTube track yet.");
            return;
        }

        self.lyrics.set_lines(lyrics);
    }

    fn highlight_lyric(&self, timestamp: i64) {
        self.lyrics.update_timestamp(timestamp);
    }

    fn reset_now_playing(&self, message: &str) {
        let _ = self.player.stop();
        self.playback_source.set(PlaybackSource::None);
        self.youtube_state.replace(None);
        self.playback_queue_v2.borrow_mut().clear();
        self.queue_v2_pending_entry.set(None);
        self.reset_youtube_recovery();
        self.player_view.set_metadata(
            self.tr(Message::IntegratedMusic),
            self.tr(Message::NoTrackSelected),
            message,
        );
        self.set_footer_metadata(self.tr(Message::NothingPlaying), "Nocky");
        self.update_footer_source();
        self.lyrics.show_state(
            "As letras aparecerão aqui",
            Some("Reproduza uma música com letras sincronizadas para acompanhar cada verso."),
            "As letras aparecerão aqui",
            Some("Reproduza uma música com letras sincronizadas para ver o contexto."),
        );
        self.hero_cover.set_path(None);
        self.visual_theme_manager.update_artwork(None);
        self.mini_cover.set_path(None);
        self.elapsed.set_text("0:00");
        self.duration.set_text("0:00");
        self.footer_elapsed.set_text("0:00");
        self.footer_duration.set_text("0:00");
        self.progress.set_value(0.0);
        self.footer_traditional_progress.set_value(0.0);
        self.home_wave_progress.set_fraction(0.0);
        self.footer_progress.set_fraction(0.0);
        self.update_play_icons(false);
        self.last_mpris_position.set(0);
        self.mpris.send(mpris::MprisUpdate::ClearMetadata);
        self.mpris
            .send(mpris::MprisUpdate::Playback(mpris::MprisPlayback::Stopped));
        self.mpris.send(mpris::MprisUpdate::Position(0));
        self.publish_mpris_capabilities();
    }

    fn initial_queue_entry_id(&self) -> Option<QueueEntryId> {
        let queue = self.playback_queue_v2.borrow();
        queue
            .current_id()
            .or_else(|| queue.entries().first().map(|entry| entry.id))
    }

    fn persist_queue_if_changed(&self) {
        let snapshot = self.playback_queue_v2.borrow().snapshot();
        if *self.queue_last_saved_snapshot.borrow() == snapshot {
            return;
        }

        let source = self.active_queue_source.get();
        match queue_store::save_for(source, &snapshot) {
            Ok(()) => {
                self.queue_last_saved_snapshot.replace(snapshot);
            }
            Err(error) => {
                eprintln!("Could not save Queue 2.0 state for {source:?}: {error}");
            }
        }
    }

    fn persist_queue_now(&self) {
        let _ = self.persist_active_queue_to_source("final");
    }

    fn save_config(&self) {
        if let Err(error) = self.config.borrow().save() {
            eprintln!("Could not save Nocky settings: {error}");
        }
    }

    fn show_toast(&self, message: &str) {
        let toast = adw::Toast::new(message);
        toast.set_use_markup(false);
        self.toast_overlay.add_toast(toast);
    }

    fn show_error(&self, message: &str) {
        if let Some(detail) = message.strip_prefix("__NOCKY_STREAM_RECOVERY_FAILED__") {
            self.youtube_recovery_in_progress.set(false);
            self.youtube_recovery_resume_us.set(0);
            self.youtube_recovery_was_playing.set(false);
            eprintln!(
                "Nocky stream recovery failed: {}",
                redact_stream_url(detail)
            );
            let friendly =
                "Não foi possível renovar o stream desta faixa. Tente reproduzi-la novamente.";
            self.album.set_text(friendly);
            self.show_toast(friendly);
            return;
        }

        eprintln!("Nocky error: {}", redact_stream_url(message));
        self.album.set_text(&format!("Error: {message}"));
        self.show_toast(message);
    }
}

fn is_refreshable_stream_error(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    let network_source = message.contains("gstsouphttpsrc")
        || message.contains("souphttpsrc")
        || message.contains("googlevideo.com");
    let rejected = message.contains("forbidden")
        || message.contains("(403)")
        || message.contains("http 403")
        || message.contains("unauthorized")
        || message.contains("(401)")
        || message.contains("gone")
        || message.contains("(410)");
    let transient_network = message.contains("connection reset")
        || message.contains("connection timed out")
        || message.contains("timed out")
        || message.contains("temporary failure")
        || message.contains("network is unreachable")
        || message.contains("host is unreachable")
        || message.contains("could not connect")
        || message.contains("internal data stream error")
        || message.contains("resource not found");

    network_source && (rejected || transient_network)
}

fn youtube_home_prefetch_candidates(library: &YouTubeLibraryCache) -> Vec<YouTubeItem> {
    let mut seen = HashSet::new();
    let mut candidates = Vec::new();
    for playlist in library
        .playlists
        .iter()
        .filter(|playlist| youtube_playlist_is_mix(playlist))
        .chain(
            library
                .playlists
                .iter()
                .filter(|playlist| !youtube_playlist_is_mix(playlist)),
        )
        .filter(|playlist| !playlist.browse_id.is_empty())
        .filter(|playlist| {
            library
                .playlist_tracks
                .get(&playlist.browse_id)
                .map(|items| items.is_empty())
                .unwrap_or(true)
        })
    {
        if seen.insert(playlist.browse_id.clone()) {
            candidates.push(playlist.clone());
        }
        if candidates.len() >= 24 {
            break;
        }
    }
    candidates
}

fn youtube_playlist_is_mix(playlist: &YouTubeItem) -> bool {
    if playlist.playlist_kind == "mix" {
        return true;
    }
    let title = playlist.title.to_lowercase();
    title.contains("mix") || title.contains("radio") || title.contains("supermix")
}

fn playback_error_message(message: &str) -> &'static str {
    youtube_error::classify_youtube_playback_error(message)
        .message(config::AppConfig::load().language)
}

fn redact_stream_url(message: &str) -> String {
    let Some(url_marker) = message.find("URL: http") else {
        return message.to_string();
    };
    let url_start = url_marker + "URL: ".len();
    let tail = &message[url_start..];
    let url_end = tail
        .find(", Redirect")
        .or_else(|| tail.find(char::is_whitespace))
        .unwrap_or(tail.len());

    let mut redacted = String::with_capacity(message.len().min(512));
    redacted.push_str(&message[..url_start]);
    redacted.push_str("<redacted>");
    redacted.push_str(&tail[url_end..]);
    redacted
}

fn build_sidebar(language: AppLanguage) -> SidebarParts {
    let tr = |message| i18n::text(language, message);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
    content.set_size_request(SIDEBAR_WIDTH, -1);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(10);
    content.set_margin_end(10);
    content.add_css_class("sidebar-content");

    let (all_button, all_label) = sidebar_row("view-list-symbolic", tr(Message::Library), true);
    let (albums_button, albums_label) =
        sidebar_row("folder-music-symbolic", tr(Message::Albums), false);
    let (artists_button, artists_label) =
        sidebar_row("avatar-default-symbolic", tr(Message::Artists), false);
    let (playlists_button, playlists_label) =
        sidebar_row("view-list-symbolic", tr(Message::Playlists), false);
    content.append(&all_button);
    content.append(&albums_button);
    content.append(&artists_button);
    content.append(&playlists_button);

    let section = gtk::Label::new(Some(tr(Message::LocalCollection)));
    section.set_xalign(0.0);
    section.set_margin_top(18);
    section.set_margin_start(10);
    section.add_css_class("section-title");
    content.append(&section);

    let (liked_button, liked_label) =
        sidebar_row("emblem-favorite-symbolic", tr(Message::LikedSongs), false);
    content.append(&liked_button);

    let spacer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    spacer.set_vexpand(true);
    content.append(&spacer);

    let revealer = gtk::Revealer::new();
    revealer.set_transition_type(gtk::RevealerTransitionType::SlideRight);
    revealer.set_transition_duration(240);
    revealer.set_reveal_child(true);
    let sidebar_motion = gtk::Fixed::new();
    sidebar_motion.set_size_request(SIDEBAR_WIDTH, -1);
    sidebar_motion.set_hexpand(false);
    sidebar_motion.set_vexpand(true);
    sidebar_motion.put(&content, 0.0, 0.0);
    revealer.set_child(Some(&sidebar_motion));
    revealer.add_css_class("sidebar");

    SidebarParts {
        revealer,
        motion: sidebar_motion,
        content,
        all_button,
        all_label,
        albums_button,
        albums_label,
        artists_button,
        artists_label,
        playlists_button,
        playlists_label,
        liked_button,
        liked_label,
        section_label: section,
    }
}

fn sidebar_row(icon_name: &str, text: &str, active: bool) -> (gtk::Button, gtk::Label) {
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    content.set_margin_top(7);
    content.set_margin_bottom(7);
    content.set_margin_start(10);
    content.set_margin_end(10);
    content.append(&gtk::Image::from_icon_name(icon_name));
    let label = gtk::Label::new(Some(text));
    label.set_xalign(0.0);
    content.append(&label);

    let button = gtk::Button::new();
    button.set_child(Some(&content));
    button.add_css_class("flat");
    button.add_css_class("sidebar-row");
    if active {
        button.add_css_class("active");
    }
    (button, label)
}

// settings_about_and_remove_overflow_v1

#[derive(Clone)]
pub(crate) struct CoverView {
    pub(crate) stack: gtk::Stack,
    picture: gtk::Picture,
    placeholder: gtk::Box,
    icon: gtk::Image,
    // nocky_cover_texture_tracks_display_size_v1
    display_size: Rc<Cell<i32>>,
    current_path: Rc<RefCell<Option<PathBuf>>>,
    transition: TransitionClock,
}

impl CoverView {
    fn set_display_size(&self, size: i32) {
        let size = size.max(1);
        let previous_size = self.display_size.replace(size);

        if self.stack.width_request() != size || self.stack.height_request() != size {
            self.stack.set_size_request(size, size);
            self.picture.set_size_request(size, size);
            self.placeholder.set_size_request(size, size);
            self.icon.set_pixel_size((f64::from(size) * 0.30) as i32);
        }

        if previous_size != size {
            let current_path = self.current_path.borrow().clone();
            self.set_path_immediate(current_path.as_deref());
        }
    }

    fn set_path(&self, path: Option<&Path>) {
        let path = path.map(Path::to_path_buf);
        self.current_path.replace(path.clone());

        if !adw::is_animations_enabled(&self.stack) {
            self.set_path_immediate(path.as_deref());
            self.stack.set_opacity(1.0);
            return;
        }

        let token = self.transition.next();
        self.transition
            .fade(token, &self.stack, self.stack.opacity(), 0.0, 0, 105);

        let cover = self.clone();
        self.transition.after(token, 116, move || {
            cover.set_path_immediate(path.as_deref());
            cover.transition.fade(token, &cover.stack, 0.0, 1.0, 0, 205);
        });
    }

    fn set_path_immediate(&self, path: Option<&Path>) {
        let Some(path) = path.filter(|path| path.is_file()) else {
            self.picture.set_paintable(None::<&gdk::Texture>);
            self.stack.set_visible_child_name("placeholder");
            return;
        };

        match square_cover_pixbuf(path, self.display_size.get()) {
            Some(pixbuf) => {
                let texture = gdk::Texture::for_pixbuf(&pixbuf);
                self.picture.set_paintable(Some(&texture));
                self.stack.set_visible_child_name("picture");
            }
            None => {
                eprintln!("Could not load cover {}", path.display());
                self.picture.set_paintable(None::<&gdk::Texture>);
                self.stack.set_visible_child_name("placeholder");
            }
        }
    }
}

fn square_cover_pixbuf(path: &Path, size: i32) -> Option<gdk_pixbuf::Pixbuf> {
    let pixbuf = gdk_pixbuf::Pixbuf::from_file(path).ok()?;
    let width = pixbuf.width();
    let height = pixbuf.height();
    if width <= 0 || height <= 0 {
        return None;
    }

    let side = width.min(height);
    let x = (width - side) / 2;
    let y = (height - side) / 2;
    let cropped = pixbuf.new_subpixbuf(x, y, side, side);
    cropped.scale_simple(size, size, gdk_pixbuf::InterpType::Bilinear)
}

pub(crate) fn build_cover(size: i32) -> CoverView {
    let icon = gtk::Image::from_icon_name("audio-x-generic-symbolic");
    icon.set_pixel_size((size as f64 * 0.30) as i32);
    icon.add_css_class("cover-icon");
    icon.set_halign(gtk::Align::Center);
    icon.set_valign(gtk::Align::Center);
    icon.set_hexpand(true);
    icon.set_vexpand(true);

    let placeholder = gtk::Box::new(gtk::Orientation::Vertical, 0);
    placeholder.set_width_request(size);
    placeholder.set_height_request(size);
    placeholder.set_halign(gtk::Align::Center);
    placeholder.set_valign(gtk::Align::Center);
    placeholder.set_hexpand(false);
    placeholder.set_vexpand(false);
    placeholder.append(&icon);

    let picture = gtk::Picture::new();
    picture.set_content_fit(gtk::ContentFit::Cover);
    picture.set_can_shrink(true);
    picture.set_width_request(size);
    picture.set_height_request(size);
    picture.set_halign(gtk::Align::Center);
    picture.set_valign(gtk::Align::Center);
    picture.add_css_class("cover-picture");

    let stack = gtk::Stack::new();
    stack.set_width_request(size);
    stack.set_height_request(size);
    stack.set_halign(gtk::Align::Center);
    stack.set_valign(gtk::Align::Center);
    stack.set_hexpand(false);
    stack.set_vexpand(false);
    stack.set_overflow(gtk::Overflow::Hidden);
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    stack.set_transition_duration(180);
    stack.add_named(&placeholder, Some("placeholder"));
    stack.add_named(&picture, Some("picture"));
    stack.set_visible_child_name("placeholder");
    stack.add_css_class("album-cover");
    if size <= 64 {
        stack.add_css_class("mini-cover");
    }

    CoverView {
        stack,
        picture,
        placeholder,
        icon,
        display_size: Rc::new(Cell::new(size)),
        current_path: Rc::new(RefCell::new(None)),
        transition: TransitionClock::new(),
    }
}

fn mpris_track_id(path: &Path) -> String {
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    format!("/io/github/maylton/Nocky/track_{:016x}", hasher.finish())
}

fn mpris_youtube_track_id(video_id: &str) -> String {
    let mut hasher = DefaultHasher::new();
    video_id.hash(&mut hasher);
    format!("/io/github/maylton/Nocky/youtube_{:016x}", hasher.finish())
}

fn format_time(microseconds: i64) -> String {
    let total_seconds = (microseconds / 1_000_000).max(0);
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{minutes}:{seconds:02}")
}

mod browser;
mod config;
mod i18n;
mod library;
mod listening_history;
mod lyrics;
mod lyrics_provider;
mod lyrics_view;
mod model;
mod mpris;
mod playback;
mod theme;
mod visualizer;
mod wave_progress;
mod youtube;

use crate::youtube::YouTubeArtistOverview;

use adw::prelude::*;
use browser::{BrowserEvent, BrowserRoute, LibraryBrowser};
use config::{AppLanguage, BlurMode, StartupSource};
use gtk::prelude::FileExt;
use gtk::{gdk, gio, glib};
use i18n::Message;
use listening_history::{ListeningHistory, ListeningSource};
use lyrics::LyricLine;
use lyrics_view::LyricsPresenter;
use model::{Track, TrackData};
use playback::{PlaybackEngine, PlaybackEvent};
use std::{
    cell::{Cell, RefCell},
    collections::{hash_map::DefaultHasher, HashMap, HashSet, VecDeque},
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use visualizer::SpectrumVisualizer;
use wave_progress::WaveProgress;
use youtube::{
    cache_items_for_browser, cacheable_youtube_playlist, clear_library_cache, download_cover,
    load_library_cache, save_library_cache, youtube_collection_cache_key, youtube_collection_key,
    YouTubeBridge, YouTubeItem, YouTubeLibraryCache, YouTubeLibrarySnapshot, YouTubePage,
    YouTubePageEvent, YouTubeSearchResults, YouTubeStatus, YouTubeStream,
};

const APP_ID: &str = "io.github.maylton.Nocky";

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
    stream: YouTubeStream,
    cover_path: Option<PathBuf>,
    lyrics: Vec<LyricLine>,
}

enum BackgroundMessage {
    LibraryScanned {
        root: PathBuf,
        result: Result<Vec<TrackData>, String>,
    },
    LyricsDownloaded {
        path: PathBuf,
        result: Result<(), String>,
        notify: bool,
    },
    YouTubeLyricsDownloaded {
        video_id: String,
        notify: bool,
        result: Result<Vec<LyricLine>, String>,
    },
    YouTubeStatus(Result<YouTubeStatus, String>),
    YouTubeConnected(Result<YouTubeStatus, String>),
    YouTubeDisconnected(Result<YouTubeStatus, String>),
    YouTubeLibrarySynced {
        notify: bool,
        result: Result<YouTubeLibrarySnapshot, String>,
    },
    YouTubeBrowserPlaylist {
        request_id: u64,
        playlist: YouTubeItem,
        result: Result<Vec<YouTubeItem>, String>,
    },
    YouTubeBrowserCollection {
        item: YouTubeItem,
        key: String,
        result: Result<Vec<YouTubeItem>, String>,
    },
    YouTubeArtistOverview {
        key: String,
        result: Result<YouTubeArtistOverview, String>,
    },
    YouTubePlaylistsCached(Result<HashMap<String, Vec<YouTubeItem>>, String>),
    YouTubeCollectionsCached(Result<HashMap<String, Vec<YouTubeItem>>, String>),
    YouTubeItems {
        title: String,
        result: Result<Vec<YouTubeItem>, String>,
    },
    YouTubeGlobalSearch {
        request_id: u64,
        query: String,
        result: Result<YouTubeSearchResults, String>,
    },
    YouTubeResolved {
        request_id: u64,
        queue: Vec<YouTubeItem>,
        index: usize,
        item: YouTubeItem,
        result: Result<(YouTubeStream, Option<PathBuf>), String>,
    },
}

struct SidebarParts {
    revealer: gtk::Revealer,
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
    config: RefCell<config::AppConfig>,
    listening_history: RefCell<ListeningHistory>,
    listening_session_id: RefCell<Option<String>>,
    listening_session_recorded: Cell<bool>,
    updating_progress: Cell<bool>,
    scanning: Cell<bool>,
    shuffle_enabled: Cell<bool>,
    rng_state: Cell<u64>,
    search_query: RefCell<String>,
    lyrics_pending: RefCell<HashSet<PathBuf>>,
    background_tx: Sender<BackgroundMessage>,
    background_rx: Receiver<BackgroundMessage>,
    mpris: mpris::MprisBridge,
    last_mpris_position: Cell<i64>,
    playback_source: Cell<PlaybackSource>,
    youtube_state: RefCell<Option<YouTubePlaybackState>>,
    youtube_request_id: Cell<u64>,
    youtube_search_request_id: Cell<u64>,
    youtube_recovery_in_progress: Cell<bool>,
    youtube_recovery_attempted: Cell<bool>,
    youtube_recovery_resume_us: Cell<i64>,
    youtube_playlist_request_id: Cell<u64>,
    youtube_collection_prefetching: Cell<bool>,
    youtube_playlist_loading: Cell<bool>,
    youtube_playlist_prefetching: Cell<bool>,
    youtube_pending_playlist: RefCell<Option<YouTubeItem>>,
    youtube_bridge: Option<Arc<YouTubeBridge>>,
    youtube_library: RefCell<YouTubeLibraryCache>,

    sidebar: gtk::Revealer,
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
    menu_button: gtk::MenuButton,
    search_entry: gtk::SearchEntry,
    views: adw::ViewStack,
    music_page: adw::ViewStackPage,
    lyrics_page: adw::ViewStackPage,
    browser: LibraryBrowser,
    lyrics: LyricsPresenter,
    youtube_page: Rc<YouTubePage>,

    title: gtk::Label,
    artist: gtk::Label,
    album: gtk::Label,
    now_heading: gtk::Label,
    favorite_button: gtk::Button,
    previous_button: gtk::Button,
    hero_play_button: gtk::Button,
    next_button: gtk::Button,
    mini_title: gtk::Label,
    mini_artist: gtk::Label,
    footer_source: gtk::Label,
    footer_now_playing: gtk::Button,
    footer_center: gtk::Box,
    footer_right_controls: gtk::Box,
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
    elapsed: gtk::Label,
    duration: gtk::Label,
    footer_progress: WaveProgress,
    footer_elapsed: gtk::Label,
    footer_duration: gtk::Label,
    volume: gtk::Scale,
    mute_icon: gtk::Image,
    mute_button: gtk::Button,
    volume_before_mute: Cell<f64>,
    lyrics_button: gtk::ToggleButton,
    footer_previous: gtk::Button,
    footer_play_button: gtk::Button,
    footer_next: gtk::Button,
    footer_repeat_button: gtk::ToggleButton,
    footer_shuffle_button: gtk::ToggleButton,
    repeat_button: gtk::ToggleButton,
    shuffle_button: gtk::ToggleButton,
    visualizer: SpectrumVisualizer,

    _theme: Rc<theme::ThemeBridge>,
}

fn main() -> glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_application);
    app.run()
}

fn build_application(app: &adw::Application) {
    let controller = AppController::new(app);
    controller.setup_callbacks();
    controller.install_actions(app);
    controller.load_saved_library();
    controller.window.present();

    let startup_controller = controller.clone();
    glib::idle_add_local_once(move || startup_controller.apply_startup_source());

    // Keep the controller alive for as long as the application is running.
    let keep_alive = controller.clone();
    app.connect_shutdown(move |_| {
        keep_alive.player.shutdown();
        keep_alive.mpris.send(mpris::MprisUpdate::Shutdown);
    });
}

impl AppController {
    fn new(app: &adw::Application) -> Rc<Self> {
        let theme = theme::ThemeBridge::install();
        let config = config::AppConfig::load();
        let tr = |message: Message| i18n::text(config.language, message);
        theme.set_noctalia_enabled(config.noctalia_theme_sync);
        theme.set_blur_preferences(config.blur_mode, config.blur_opacity);
        let player = PlaybackEngine::new(config.volume.clamp(0.0, 1.0))
            .unwrap_or_else(|error| panic!("Nocky playback initialization failed: {error}"));
        let (background_tx, background_rx) = mpsc::channel();

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

        let sidebar_button = gtk::ToggleButton::builder()
            .icon_name("sidebar-show-symbolic")
            .active(true)
            .tooltip_text(tr(Message::SidebarToggle))
            .build();
        header.pack_start(&sidebar_button);

        let brand = gtk::Label::new(Some("NOCKY"));
        brand.add_css_class("brand-title");
        header.pack_start(&brand);

        let switcher = adw::ViewSwitcher::builder()
            .stack(&views)
            .policy(adw::ViewSwitcherPolicy::Wide)
            .build();
        header.set_title_widget(Some(&switcher));

        let search_button = gtk::ToggleButton::builder()
            .icon_name("system-search-symbolic")
            .tooltip_text(tr(Message::SearchLibrary))
            .build();
        header.pack_end(&search_button);

        let folder_button = gtk::Button::builder()
            .icon_name("folder-open-symbolic")
            .tooltip_text(tr(Message::ChooseMusicFolderTooltip))
            .build();
        header.pack_end(&folder_button);

        let menu = build_main_menu(config.language);
        let menu_button = gtk::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .menu_model(&menu)
            .build();
        header.pack_end(&menu_button);
        shell.append(&header);

        let search_bar = gtk::SearchBar::new();
        let search_entry = gtk::SearchEntry::builder()
            .placeholder_text(tr(Message::SearchPlaceholder))
            .hexpand(true)
            .build();
        search_bar.set_child(Some(&search_entry));
        search_bar.connect_entry(&search_entry);
        search_bar.set_key_capture_widget(Some(&window));
        search_bar.set_show_close_button(true);
        shell.append(&search_bar);

        let body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        body.set_vexpand(true);
        body.set_hexpand(true);
        shell.append(&body);

        let sidebar_parts = build_sidebar(config.language);
        body.append(&sidebar_parts.revealer);

        let title = gtk::Label::new(Some(tr(Message::IntegratedMusic)));
        title.set_xalign(0.0);
        title.set_wrap(false);
        title.set_single_line_mode(true);
        title.set_width_chars(28);
        title.set_max_width_chars(28);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title.add_css_class("hero-title");

        let artist = gtk::Label::new(Some(tr(Message::NoTrackSelected)));
        artist.set_xalign(0.0);
        artist.set_single_line_mode(true);
        artist.set_width_chars(28);
        artist.set_max_width_chars(28);
        artist.set_ellipsize(gtk::pango::EllipsizeMode::End);
        artist.add_css_class("hero-artist");

        let album = gtk::Label::new(Some(tr(Message::ChooseFolderToStart)));
        album.set_xalign(0.0);
        album.set_single_line_mode(true);
        album.set_width_chars(28);
        album.set_max_width_chars(28);
        album.set_ellipsize(gtk::pango::EllipsizeMode::End);
        album.add_css_class("dim-label");

        let favorite_icon = gtk::Image::from_icon_name("emblem-favorite-symbolic");
        favorite_icon.set_opacity(0.28);
        let favorite = gtk::Button::new();
        favorite.set_child(Some(&favorite_icon));
        favorite.add_css_class("flat");
        favorite.add_css_class("card-icon-button");
        favorite.set_tooltip_text(Some(tr(Message::FavoriteTooltip)));
        favorite.add_css_class("like-button");

        let now_heading = gtk::Label::new(Some(tr(Message::NowPlaying)));
        now_heading.set_xalign(0.0);
        now_heading.set_hexpand(true);
        now_heading.add_css_class("now-heading");
        let headphones = gtk::Image::from_icon_name("audio-headphones-symbolic");
        headphones.add_css_class("now-heading-icon");
        let now_header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        now_header.append(&now_heading);
        now_header.append(&headphones);

        let title_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        title_row.set_width_request(320);
        title_row.append(&title);
        title_row.append(&favorite);

        let hero_cover = build_cover(280);
        hero_cover.stack.set_halign(gtk::Align::Center);

        // Keep placeholder and real artwork in the same balanced vertical slot.
        let hero_cover_slot = gtk::CenterBox::new();
        hero_cover_slot.set_orientation(gtk::Orientation::Vertical);
        hero_cover_slot.set_vexpand(false);
        hero_cover_slot.set_hexpand(true);
        hero_cover_slot.set_margin_top(28);
        hero_cover_slot.set_margin_bottom(20);
        hero_cover_slot.set_center_widget(Some(&hero_cover.stack));
        hero_cover_slot.add_css_class("hero-cover-slot");

        let elapsed = gtk::Label::new(Some("0:00"));
        elapsed.add_css_class("time-label");
        let duration = gtk::Label::new(Some("0:00"));
        duration.add_css_class("time-label");
        let progress = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 0.001);
        progress.set_draw_value(false);
        progress.set_hexpand(true);
        progress.add_css_class("progress-scale");

        let time_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        elapsed.set_hexpand(true);
        elapsed.set_xalign(0.0);
        duration.set_xalign(1.0);
        time_row.append(&elapsed);
        time_row.append(&duration);

        let repeat = gtk::ToggleButton::builder()
            .icon_name("media-playlist-repeat-symbolic")
            .tooltip_text(tr(Message::RepeatTrack))
            .build();
        repeat.add_css_class("media-control");
        let previous = gtk::Button::from_icon_name("media-skip-backward-symbolic");
        previous.set_tooltip_text(Some(tr(Message::PreviousTrack)));
        previous.add_css_class("media-control");

        let hero_play_icon = gtk::Image::from_icon_name("media-playback-start-symbolic");
        hero_play_icon.set_pixel_size(24);
        let hero_play_button = gtk::Button::new();
        hero_play_button.set_child(Some(&hero_play_icon));
        hero_play_button.add_css_class("shell-play-button");
        hero_play_button.set_tooltip_text(Some(tr(Message::PlayPause)));

        let next = gtk::Button::from_icon_name("media-skip-forward-symbolic");
        next.set_tooltip_text(Some(tr(Message::NextTrack)));
        next.add_css_class("media-control");
        let shuffle = gtk::ToggleButton::builder()
            .icon_name("media-playlist-shuffle-symbolic")
            .tooltip_text(tr(Message::Shuffle))
            .build();
        shuffle.add_css_class("media-control");

        let controls = gtk::Box::new(gtk::Orientation::Horizontal, 18);
        controls.set_halign(gtk::Align::Center);
        controls.append(&repeat);
        controls.append(&previous);
        controls.append(&hero_play_button);
        controls.append(&next);
        controls.append(&shuffle);

        let visualizer = SpectrumVisualizer::new();
        let lyrics = LyricsPresenter::new();

        let now_card = gtk::Box::new(gtk::Orientation::Vertical, 12);
        now_card.set_size_request(380, -1);
        now_card.set_hexpand(false);
        now_card.set_vexpand(true);
        now_card.set_valign(gtk::Align::Fill);
        now_card.add_css_class("now-playing-card");
        now_card.append(&now_header);
        now_card.append(&hero_cover_slot);
        now_card.append(&title_row);
        now_card.append(&artist);
        now_card.append(&album);
        now_card.append(&progress);
        now_card.append(&time_row);
        now_card.append(&controls);
        now_card.append(visualizer.widget());
        now_card.append(lyrics.inline_widget());

        let browser = LibraryBrowser::new();

        let dashboard = gtk::Box::new(gtk::Orientation::Horizontal, 22);
        dashboard.set_margin_top(22);
        dashboard.set_margin_bottom(22);
        dashboard.set_margin_start(24);
        dashboard.set_margin_end(24);
        dashboard.set_vexpand(true);
        dashboard.append(&now_card);
        dashboard.append(browser.root());

        let empty_state = gtk::Box::new(gtk::Orientation::Vertical, 12);
        empty_state.set_halign(gtk::Align::Center);
        empty_state.set_valign(gtk::Align::Center);
        empty_state.set_vexpand(true);
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
        body.append(&views);

        let mini_cover = build_cover(54);
        let mini_title = gtk::Label::new(Some(tr(Message::NothingPlaying)));
        mini_title.set_xalign(0.0);
        mini_title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        mini_title.add_css_class("now-title");
        mini_title.set_hexpand(true);

        let footer_favorite_icon = gtk::Image::from_icon_name("emblem-favorite-symbolic");
        footer_favorite_icon.set_opacity(0.28);
        let footer_favorite = gtk::Button::new();
        footer_favorite.set_child(Some(&footer_favorite_icon));
        footer_favorite.add_css_class("flat");
        footer_favorite.add_css_class("footer-control");
        footer_favorite.add_css_class("footer-favorite-button");
        footer_favorite.set_tooltip_text(Some(tr(Message::FavoriteTooltip)));

        let mini_title_row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        mini_title_row.append(&mini_title);
        mini_title_row.append(&footer_favorite);

        let mini_artist = gtk::Label::new(Some("Nocky"));
        mini_artist.set_xalign(0.0);
        mini_artist.set_ellipsize(gtk::pango::EllipsizeMode::End);
        mini_artist.add_css_class("dim-label");
        mini_artist.set_hexpand(true);

        let footer_source = gtk::Label::new(Some(tr(Message::SourceNone)));
        footer_source.add_css_class("source-badge");
        footer_source.add_css_class("footer-source-badge");

        let mini_subtitle_row = gtk::Box::new(gtk::Orientation::Horizontal, 7);
        mini_subtitle_row.append(&mini_artist);
        mini_subtitle_row.append(&footer_source);

        let mini_text = gtk::Box::new(gtk::Orientation::Vertical, 1);
        mini_text.set_hexpand(true);
        mini_text.add_css_class("footer-meta");
        mini_text.append(&mini_title_row);
        mini_text.append(&mini_subtitle_row);

        let now_playing_content = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        now_playing_content.append(&mini_cover.stack);
        now_playing_content.append(&mini_text);

        let footer_now_playing = gtk::Button::new();
        footer_now_playing.set_child(Some(&now_playing_content));
        footer_now_playing.set_size_request(350, 56);
        footer_now_playing.add_css_class("flat");
        footer_now_playing.add_css_class("footer-now-playing-button");

        let footer_shuffle = gtk::ToggleButton::builder()
            .icon_name("media-playlist-shuffle-symbolic")
            .tooltip_text(tr(Message::Shuffle))
            .build();
        footer_shuffle.add_css_class("flat");
        footer_shuffle.add_css_class("footer-control");

        let footer_previous = gtk::Button::from_icon_name("media-skip-backward-symbolic");
        footer_previous.set_tooltip_text(Some(tr(Message::PreviousTrack)));
        footer_previous.add_css_class("flat");
        footer_previous.add_css_class("footer-control");

        let play_icon = gtk::Image::from_icon_name("media-playback-start-symbolic");
        play_icon.set_pixel_size(20);
        let play = gtk::Button::new();
        play.set_child(Some(&play_icon));
        play.add_css_class("flat");
        play.add_css_class("mini-play-button");
        play.set_tooltip_text(Some(tr(Message::PlayPause)));

        let footer_next = gtk::Button::from_icon_name("media-skip-forward-symbolic");
        footer_next.set_tooltip_text(Some(tr(Message::NextTrack)));
        footer_next.add_css_class("flat");
        footer_next.add_css_class("footer-control");

        let footer_repeat = gtk::ToggleButton::builder()
            .icon_name("media-playlist-repeat-symbolic")
            .tooltip_text(tr(Message::RepeatTrack))
            .build();
        footer_repeat.add_css_class("flat");
        footer_repeat.add_css_class("footer-control");

        let footer_transport = gtk::Box::new(gtk::Orientation::Horizontal, 7);
        footer_transport.set_halign(gtk::Align::Center);
        footer_transport.append(&footer_shuffle);
        footer_transport.append(&footer_previous);
        footer_transport.append(&play);
        footer_transport.append(&footer_next);
        footer_transport.append(&footer_repeat);

        let footer_progress = WaveProgress::new();
        let footer_elapsed = gtk::Label::new(Some("0:00"));
        footer_elapsed.add_css_class("time-label");
        let footer_duration = gtk::Label::new(Some("0:00"));
        footer_duration.add_css_class("time-label");

        let footer_progress_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        footer_progress_row.set_hexpand(true);
        footer_progress_row.append(&footer_elapsed);
        footer_progress_row.append(footer_progress.widget());
        footer_progress_row.append(&footer_duration);

        let footer_center = gtk::Box::new(gtk::Orientation::Vertical, 2);
        footer_center.set_size_request(500, 60);
        footer_center.set_halign(gtk::Align::Center);
        footer_center.append(&footer_transport);
        footer_center.append(&footer_progress_row);

        let lyrics_button = gtk::ToggleButton::builder()
            .icon_name("audio-input-microphone-symbolic")
            .tooltip_text(tr(Message::LyricsTooltip))
            .build();
        lyrics_button.add_css_class("flat");
        lyrics_button.add_css_class("footer-control");
        lyrics_button.add_css_class("footer-lyrics-button");

        let mute_icon = gtk::Image::from_icon_name("audio-volume-high-symbolic");
        let mute_button = gtk::Button::new();
        mute_button.set_child(Some(&mute_icon));
        mute_button.add_css_class("flat");
        mute_button.add_css_class("footer-control");
        mute_button.set_tooltip_text(Some(tr(Message::Mute)));

        let volume = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 0.01);
        volume.set_draw_value(false);
        volume.set_value(config.volume.clamp(0.0, 1.0));
        volume.set_size_request(112, -1);
        volume.add_css_class("footer-volume");

        let right_controls = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        right_controls.set_halign(gtk::Align::End);
        right_controls.set_size_request(220, 56);
        right_controls.append(&lyrics_button);
        right_controls.append(&mute_button);
        right_controls.append(&volume);

        let player_bar = gtk::CenterBox::new();
        player_bar.set_height_request(88);
        player_bar.add_css_class("player-bar");
        player_bar.add_css_class("player-bar-v2");
        player_bar.set_start_widget(Some(&footer_now_playing));
        player_bar.set_center_widget(Some(&footer_center));
        player_bar.set_end_widget(Some(&right_controls));
        shell.append(&player_bar);

        let mpris = mpris::MprisBridge::start(config.volume);
        let youtube_bridge = YouTubeBridge::discover().ok().map(Arc::new);

        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos() as u64)
            .unwrap_or(0x9e37_79b9_7f4a_7c15);

        let initial_volume = config.volume.clamp(0.15, 1.0);
        let controller = Rc::new(Self {
            window,
            toast_overlay,
            player,
            state: RefCell::new(AppState::default()),
            config: RefCell::new(config),
            listening_history: RefCell::new(ListeningHistory::load()),
            listening_session_id: RefCell::new(None),
            listening_session_recorded: Cell::new(false),
            updating_progress: Cell::new(false),
            scanning: Cell::new(false),
            shuffle_enabled: Cell::new(false),
            rng_state: Cell::new(seed),
            search_query: RefCell::new(String::new()),
            lyrics_pending: RefCell::new(HashSet::new()),
            background_tx,
            background_rx,
            mpris,
            last_mpris_position: Cell::new(-1),
            playback_source: Cell::new(PlaybackSource::None),
            youtube_state: RefCell::new(None),
            youtube_request_id: Cell::new(0),
            youtube_search_request_id: Cell::new(0),
            youtube_recovery_in_progress: Cell::new(false),
            youtube_recovery_attempted: Cell::new(false),
            youtube_recovery_resume_us: Cell::new(0),
            youtube_playlist_request_id: Cell::new(0),
            youtube_collection_prefetching: Cell::new(false),
            youtube_playlist_loading: Cell::new(false),
            youtube_playlist_prefetching: Cell::new(false),
            youtube_pending_playlist: RefCell::new(None),
            youtube_bridge,
            youtube_library: RefCell::new(load_library_cache()),
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
            menu_button: menu_button.clone(),
            search_entry: search_entry.clone(),
            views,
            music_page,
            lyrics_page,
            browser,
            lyrics,
            youtube_page,
            title,
            artist,
            album,
            now_heading,
            favorite_button: favorite.clone(),
            previous_button: previous.clone(),
            hero_play_button: hero_play_button.clone(),
            next_button: next.clone(),
            mini_title,
            mini_artist,
            footer_source,
            footer_now_playing: footer_now_playing.clone(),
            footer_center,
            footer_right_controls: right_controls,
            music_stack,
            empty_title,
            empty_text,
            empty_add,
            hero_cover,
            mini_cover,
            player_bar: player_bar.clone(),
            play_icon,
            hero_play_icon,
            favorite_icon,
            footer_favorite_icon,
            footer_favorite_button: footer_favorite.clone(),
            progress,
            elapsed,
            duration,
            footer_progress,
            footer_elapsed,
            footer_duration,
            volume,
            mute_icon,
            mute_button: mute_button.clone(),
            volume_before_mute: Cell::new(initial_volume),
            lyrics_button,
            footer_previous: footer_previous.clone(),
            footer_play_button: play.clone(),
            footer_next: footer_next.clone(),
            footer_repeat_button: footer_repeat.clone(),
            footer_shuffle_button: footer_shuffle.clone(),
            repeat_button: repeat.clone(),
            shuffle_button: shuffle.clone(),
            visualizer,
            _theme: theme,
        });
        controller.apply_translations();
        controller.apply_home_preferences();
        controller.apply_volume_icon();
        controller.install_footer_adaptive();

        {
            let weak = Rc::downgrade(&controller);
            sidebar_button.connect_toggled(move |button| {
                if let Some(controller) = weak.upgrade() {
                    controller.sidebar.set_reveal_child(button.is_active());
                }
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
            let click = gtk::GestureClick::new();
            click.connect_released(move |_, _, _, _| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                if controller.views.visible_child_name().as_deref() == Some("music") {
                    controller.navigate_browser(BrowserRoute::All);
                }
            });
            switcher.add_controller(click);
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
                    controller.views.set_visible_child_name("music");
                    if controller.lyrics_button.is_active() {
                        controller.lyrics_button.set_active(false);
                    }
                }
            });
        }

        {
            let weak = Rc::downgrade(&controller);
            mute_button.connect_clicked(move |_| {
                if let Some(controller) = weak.upgrade() {
                    let current = controller.volume.value();
                    if current > 0.001 {
                        controller.volume_before_mute.set(current);
                        controller.volume.set_value(0.0);
                    } else {
                        controller
                            .volume
                            .set_value(controller.volume_before_mute.get().clamp(0.15, 1.0));
                    }
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
            let weak = Rc::downgrade(self);
            self.window.connect_close_request(move |_| {
                if let Some(controller) = weak.upgrade() {
                    controller.player.shutdown();
                    controller.mpris.send(mpris::MprisUpdate::Shutdown);
                }
                glib::Propagation::Proceed
            });
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
            let mut progress_ticks = 0_u8;
            glib::timeout_add_local(Duration::from_millis(50), move || {
                let Some(controller) = weak.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                controller.handle_background_messages();
                controller.handle_browser_events();
                controller.handle_youtube_events();
                controller.handle_mpris_commands();
                controller.handle_playback_events();

                progress_ticks = progress_ticks.wrapping_add(1);
                let cadence = if controller.player.is_playing() {
                    2
                } else {
                    10
                };
                if progress_ticks % cadence == 0 {
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

    fn refresh_youtube_status(&self) {
        let Some(bridge) = self.youtube_bridge.clone() else {
            self.youtube_page.set_status(&YouTubeStatus::default());
            self.youtube_page.show_error(
                "YouTube Music runtime is missing. Run ./scripts/setup-youtube-runtime.sh for cargo run, or reinstall with ./install.sh --install-youtube.",
            );
            return;
        };
        let sender = self.background_tx.clone();
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
        self.refresh_browser();
        let sender = self.background_tx.clone();
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
        let sender = self.background_tx.clone();
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
        let sender = self.background_tx.clone();
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
        let title = item.title.clone();
        let route = if item.result_type == "artist" {
            BrowserRoute::YouTubeArtist(title)
        } else {
            BrowserRoute::YouTubeAlbum(title)
        };
        let key = youtube_collection_cache_key(&item);

        if item.result_type == "artist" {
            let cached = self
                .youtube_library
                .borrow()
                .artist_albums
                .contains_key(&key);
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
                .artist_loading
                .insert(key.clone());
            self.navigate_browser(route);
            let sender = self.background_tx.clone();
            thread::spawn(move || {
                let result = bridge.artist_overview(&item).map(|mut overview| {
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

        let sender = self.background_tx.clone();
        thread::spawn(move || {
            let result = bridge.collection(&item).map(|mut items| {
                cache_items_for_browser(&mut items);
                items
            });
            let _ = sender.send(BackgroundMessage::YouTubeBrowserCollection { item, key, result });
        });
    }

    fn is_open_youtube_collection(&self, key: &str) -> bool {
        match self.browser.route() {
            BrowserRoute::YouTubeAlbum(title) => youtube_collection_key("album", &title) == key,
            BrowserRoute::YouTubeArtist(title) => youtube_collection_key("artist", &title) == key,
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
        let sender = self.background_tx.clone();
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

    fn prefetch_home_artist_profiles(&self) {
        let Some(bridge) = self.youtube_bridge.clone() else {
            return;
        };

        let artists = {
            let mut library = self.youtube_library.borrow_mut();
            let candidates = library
                .artists
                .iter()
                .take(12)
                .filter_map(|entry| {
                    let key = youtube_collection_key("artist", &entry.title);
                    let missing = !library.artist_profiles.contains_key(&key);
                    let idle = !library.artist_loading.contains(&key);

                    (missing && idle).then(|| (key, entry.source.clone()))
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

        let sender = self.background_tx.clone();
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

                    let result = bridge.artist_overview(&item).map(|mut overview| {
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

        let sender = self.background_tx.clone();
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
                    let sender = self.background_tx.clone();
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
                    let sender = self.background_tx.clone();
                    thread::spawn(move || {
                        let _ =
                            sender.send(BackgroundMessage::YouTubeConnected(bridge.connect(&raw)));
                    });
                }
                YouTubePageEvent::Disconnect => {
                    self.youtube_page
                        .set_loading(true, "Desconectando conta...");
                    let sender = self.background_tx.clone();
                    thread::spawn(move || {
                        let _ = sender
                            .send(BackgroundMessage::YouTubeDisconnected(bridge.disconnect()));
                    });
                }
                YouTubePageEvent::LoadLibrary => {
                    self.youtube_page
                        .set_loading(true, "Carregando sua biblioteca...");
                    let sender = self.background_tx.clone();
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
                    let sender = self.background_tx.clone();
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
                    let sender = self.background_tx.clone();
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
                    let sender = self.background_tx.clone();
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

    fn resolve_youtube_track(
        &self,
        item: YouTubeItem,
        queue: Vec<YouTubeItem>,
        index: usize,
        force: bool,
    ) {
        let Some(bridge) = self.youtube_bridge.clone() else {
            self.show_toast("As dependências do YouTube Music não estão instaladas");
            return;
        };
        if item.video_id.is_empty() {
            return;
        }
        let request_id = self.youtube_request_id.get().wrapping_add(1);
        self.youtube_request_id.set(request_id);
        let sender = self.background_tx.clone();
        thread::spawn(move || {
            let result = bridge
                .resolve(&item.video_id, force)
                .map(|stream| {
                    let cover = download_cover(&item, &stream.thumbnail_url);
                    (stream, cover)
                })
                .map_err(|error| {
                    if force {
                        format!("__NOCKY_STREAM_RECOVERY_FAILED__{error}")
                    } else {
                        error
                    }
                });
            let _ = sender.send(BackgroundMessage::YouTubeResolved {
                request_id,
                queue,
                index,
                item,
                result,
            });
        });
    }

    fn try_recover_youtube_stream(&self, error: &str) -> bool {
        if self.playback_source.get() != PlaybackSource::YouTube
            || self.youtube_recovery_in_progress.get()
            || self.youtube_recovery_attempted.get()
            || !is_refreshable_stream_error(error)
        {
            return false;
        }

        let snapshot = {
            let state = self.youtube_state.borrow();
            state
                .as_ref()
                .map(|state| (state.queue.clone(), state.current, state.item.clone()))
        };
        let Some((queue, index, item)) = snapshot else {
            return false;
        };

        self.youtube_recovery_attempted.set(true);
        self.youtube_recovery_in_progress.set(true);
        self.youtube_recovery_resume_us
            .set(self.player.position_us().max(0));
        let _ = self.player.stop();

        eprintln!(
            "Nocky YouTube stream rejected; refreshing signed URL: {}",
            redact_stream_url(error)
        );
        self.resolve_youtube_track(item, queue, index, true);
        true
    }

    fn reset_youtube_recovery(&self) {
        self.youtube_recovery_in_progress.set(false);
        self.youtube_recovery_attempted.set(false);
        self.youtube_recovery_resume_us.set(0);
    }

    fn resume_youtube_after_recovery(&self) {
        let resume_us = self.youtube_recovery_resume_us.replace(0);
        if resume_us <= 0 || self.playback_source.get() != PlaybackSource::YouTube {
            return;
        }

        if self.player.duration_us() <= 0 {
            self.youtube_recovery_resume_us.set(resume_us);
            return;
        }

        if let Err(error) = self.player.seek(resume_us) {
            eprintln!("Could not restore YouTube playback position: {error}");
            return;
        }

        self.last_mpris_position.set(resume_us);
        self.mpris.send(mpris::MprisUpdate::Position(resume_us));
    }

    fn apply_youtube_track(
        &self,
        queue: Vec<YouTubeItem>,
        index: usize,
        mut item: YouTubeItem,
        stream: YouTubeStream,
        cover_path: Option<PathBuf>,
    ) {
        let recovering = self.youtube_recovery_in_progress.replace(false);
        let (preserved_lyrics, preserved_cover) = if recovering {
            self.youtube_state
                .borrow()
                .as_ref()
                .filter(|state| state.item.video_id == item.video_id)
                .map(|state| (state.lyrics.clone(), state.cover_path.clone()))
                .unwrap_or_default()
        } else {
            self.youtube_recovery_attempted.set(false);
            self.youtube_recovery_resume_us.set(0);
            (Vec::new(), None)
        };
        let cover_path = cover_path.or(preserved_cover);

        if item.title.is_empty() {
            item.title = stream.title.clone();
        }
        if item.artist.is_empty() {
            item.artist = stream.artist.clone();
        }
        if item.album.is_empty() {
            item.album = stream.album.clone();
        }
        if item.duration_seconds == 0 {
            item.duration_seconds = stream.duration_seconds;
        }

        if let Err(error) =
            self.player
                .load_with_headers(&stream.stream_url, true, stream.http_headers.clone())
        {
            self.youtube_recovery_in_progress.set(false);
            self.youtube_recovery_resume_us.set(0);
            self.show_error(&error);
            return;
        }

        self.state.borrow_mut().current = None;
        self.playback_source.set(PlaybackSource::YouTube);
        self.update_footer_source();
        self.begin_listening_session(format!("youtube:{}", item.video_id));
        self.youtube_state.replace(Some(YouTubePlaybackState {
            queue,
            current: index,
            item: item.clone(),
            stream: stream.clone(),
            cover_path: cover_path.clone(),
            lyrics: preserved_lyrics.clone(),
        }));

        self.title.set_text(&item.title);
        self.artist.set_text(if item.artist.is_empty() {
            "YouTube Music"
        } else {
            &item.artist
        });
        self.album.set_text(if item.album.is_empty() {
            "YouTube Music"
        } else {
            &item.album
        });
        self.mini_title.set_text(&item.title);
        self.mini_artist.set_text(if item.artist.is_empty() {
            "YouTube Music"
        } else {
            &item.artist
        });
        self.hero_cover.set_path(cover_path.as_deref());
        self.mini_cover.set_path(cover_path.as_deref());
        self.favorite_icon
            .set_icon_name(Some("emblem-favorite-symbolic"));
        self.favorite_icon.set_opacity(0.28);
        self.footer_favorite_icon
            .set_icon_name(Some("emblem-favorite-symbolic"));
        self.footer_favorite_icon.set_opacity(0.28);

        if recovering && !preserved_lyrics.is_empty() {
            self.rebuild_youtube_lyrics(&preserved_lyrics);
        } else if self.config.borrow().auto_download_lyrics {
            self.set_lyrics_message("Searching synchronized lyrics for this YouTube track…");
            self.request_youtube_lyrics(&item, false);
        } else {
            self.set_lyrics_message(
                "No synchronized lyrics loaded yet. Use the menu to search for this YouTube track.",
            );
        }

        self.update_play_icons(true);
        if !recovering {
            self.last_mpris_position.set(0);
            self.mpris.send(mpris::MprisUpdate::Position(0));
        }
        self.publish_mpris_youtube(&item, &stream, cover_path.as_deref());
        self.mpris
            .send(mpris::MprisUpdate::Playback(mpris::MprisPlayback::Playing));
        self.prefetch_youtube_queue();
    }

    fn prefetch_youtube_queue(&self) {
        let Some(bridge) = self.youtube_bridge.clone() else {
            return;
        };
        let (queue, current) = {
            let state = self.youtube_state.borrow();
            let Some(state) = state.as_ref() else {
                return;
            };
            (state.queue.clone(), state.current)
        };
        thread::spawn(move || bridge.preload_streams(&queue, current, 4));
    }

    fn set_lyrics_message(&self, message: &str) {
        self.lyrics.show_message(message, None);
    }

    fn youtube_next_track(&self) {
        let state = self.youtube_state.borrow();
        let Some(current) = state.as_ref() else {
            return;
        };
        if current.queue.is_empty() {
            return;
        }
        let next = if self.shuffle_enabled.get() && current.queue.len() > 1 {
            let mut value = self.rng_state.get();
            value ^= value << 13;
            value ^= value >> 7;
            value ^= value << 17;
            self.rng_state.set(value);
            let mut position = value as usize % current.queue.len();
            if position == current.current {
                position = (position + 1) % current.queue.len();
            }
            Some(position)
        } else {
            current
                .current
                .checked_add(1)
                .filter(|position| *position < current.queue.len())
        };
        let Some(next) = next else {
            return;
        };
        let queue = current.queue.clone();
        let item = queue[next].clone();
        drop(state);
        self.resolve_youtube_track(item, queue, next, false);
    }

    fn youtube_previous_track(&self) {
        if self.player.position_us() > 5_000_000 {
            self.seek_to(0, true);
            return;
        }
        let state = self.youtube_state.borrow();
        let Some(current) = state.as_ref() else {
            return;
        };
        let Some(previous) = current.current.checked_sub(1) else {
            drop(state);
            self.seek_to(0, true);
            return;
        };
        let queue = current.queue.clone();
        let item = queue[previous].clone();
        drop(state);
        self.resolve_youtube_track(item, queue, previous, false);
    }

    fn publish_mpris_youtube(
        &self,
        item: &YouTubeItem,
        stream: &YouTubeStream,
        cover_path: Option<&Path>,
    ) {
        let length_us = item
            .duration_seconds
            .max(stream.duration_seconds)
            .saturating_mul(1_000_000)
            .min(i64::MAX as u64) as i64;
        let art_url = cover_path.map(|path| gio::File::for_path(path).uri().to_string());
        self.mpris
            .send(mpris::MprisUpdate::Metadata(mpris::MprisTrack {
                track_id: mpris_youtube_track_id(&item.video_id),
                title: item.title.clone(),
                artist: if item.artist.is_empty() {
                    stream.artist.clone()
                } else {
                    item.artist.clone()
                },
                album: if item.album.is_empty() {
                    stream.album.clone()
                } else {
                    item.album.clone()
                },
                length_us,
                art_url,
                url: Some(stream.webpage_url.clone()),
            }));
        self.publish_mpris_capabilities();
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

        let settings = gio::SimpleAction::new("settings", None);
        {
            let weak = Rc::downgrade(self);
            settings.connect_activate(move |_, _| {
                if let Some(controller) = weak.upgrade() {
                    controller.show_settings_dialog();
                }
            });
        }
        app.add_action(&settings);

        let about = gio::SimpleAction::new("about", None);
        {
            let weak = Rc::downgrade(self);
            about.connect_activate(move |_, _| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                let dialog = gtk::AboutDialog::builder()
                    .transient_for(&controller.window)
                    .modal(true)
                    .program_name("Nocky")
                    .version(env!("CARGO_PKG_VERSION"))
                    .comments(controller.tr(Message::AboutDescription))
                    .license_type(gtk::License::Gpl30)
                    .build();
                dialog.present();
            });
        }
        app.add_action(&about);

        let quit = gio::SimpleAction::new("quit", None);
        {
            let app = app.clone();
            quit.connect_activate(move |_, _| app.quit());
        }
        app.add_action(&quit);

        app.set_accels_for_action("app.choose-library", &["<Primary>O"]);
        app.set_accels_for_action("app.rescan", &["F5"]);
        app.set_accels_for_action("app.download-lyrics", &["<Primary>L"]);
        app.set_accels_for_action("app.quit", &["<Primary>Q"]);
    }

    fn apply_startup_source(self: &Rc<Self>) {
        self.views.set_visible_child_name("music");
        if self.lyrics_button.is_active() {
            self.lyrics_button.set_active(false);
        }
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
        self.config.borrow_mut().startup_source = Some(source);
        self.save_config();
        self.views.set_visible_child_name("music");
        if self.lyrics_button.is_active() {
            self.lyrics_button.set_active(false);
        }
        match source {
            StartupSource::Local => self.refresh_browser(),
            StartupSource::YouTube => {
                self.refresh_browser();
                self.refresh_youtube_status();
            }
        }
    }

    fn tr(&self, message: Message) -> &'static str {
        i18n::text(self.config.borrow().language, message)
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
        self.mute_button.set_tooltip_text(Some(if value <= 0.001 {
            self.tr(Message::Unmute)
        } else {
            self.tr(Message::Mute)
        }));
    }

    fn apply_translations(&self) {
        let language = self.config.borrow().language;
        let tr = |message| i18n::text(language, message);

        self.sidebar_button
            .set_tooltip_text(Some(tr(Message::SidebarToggle)));
        self.search_button
            .set_tooltip_text(Some(tr(Message::SearchLibrary)));
        self.folder_button
            .set_tooltip_text(Some(tr(Message::ChooseMusicFolderTooltip)));
        self.search_entry
            .set_placeholder_text(Some(tr(Message::SearchPlaceholder)));
        self.menu_button
            .set_menu_model(Some(&build_main_menu(language)));

        self.sidebar_all_label.set_text(tr(Message::Library));
        self.sidebar_albums_label.set_text(tr(Message::Albums));
        self.sidebar_artists_label.set_text(tr(Message::Artists));
        self.sidebar_playlists_label
            .set_text(tr(Message::Playlists));
        self.sidebar_liked_label.set_text(tr(Message::LikedSongs));
        self.sidebar_section_label
            .set_text(tr(Message::LocalCollection));

        self.now_heading.set_text(tr(Message::NowPlaying));
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

        self.music_page.set_title(tr(Message::MusicTab));
        self.lyrics_page.set_title(tr(Message::LyricsTab));
        self.empty_title.set_text(tr(Message::EmptyLibraryTitle));
        self.empty_text
            .set_text(tr(Message::EmptyLibraryDescription));
        self.empty_add.set_label(tr(Message::ChooseFolderAction));

        if self.playback_source.get() == PlaybackSource::None {
            self.title.set_text(tr(Message::IntegratedMusic));
            self.artist.set_text(tr(Message::NoTrackSelected));
            self.album.set_text(tr(Message::ChooseFolderToStart));
            self.mini_title.set_text(tr(Message::NothingPlaying));
        }

        self.update_footer_source();
        self.apply_volume_icon();
    }

    fn install_footer_adaptive(&self) {
        let mode = Rc::new(Cell::new(u8::MAX));
        let mode_state = mode.clone();
        let now_playing = self.footer_now_playing.clone();
        let center = self.footer_center.clone();
        let right = self.footer_right_controls.clone();
        let source = self.footer_source.clone();
        let artist = self.mini_artist.clone();
        let elapsed = self.footer_elapsed.clone();
        let duration = self.footer_duration.clone();
        let shuffle = self.footer_shuffle_button.clone();
        let repeat = self.footer_repeat_button.clone();
        let volume = self.volume.clone();

        self.player_bar.add_tick_callback(move |bar, _| {
            let width = bar.width();
            let next_mode = if width >= 1040 {
                0
            } else if width >= 790 {
                1
            } else {
                2
            };

            if mode_state.get() == next_mode {
                return glib::ControlFlow::Continue;
            }
            mode_state.set(next_mode);

            match next_mode {
                0 => {
                    now_playing.set_size_request(350, 56);
                    center.set_size_request(500, 60);
                    right.set_size_request(220, 56);
                    source.set_visible(true);
                    artist.set_visible(true);
                    elapsed.set_visible(true);
                    duration.set_visible(true);
                    shuffle.set_visible(true);
                    repeat.set_visible(true);
                    volume.set_visible(true);
                }
                1 => {
                    now_playing.set_size_request(280, 56);
                    center.set_size_request(390, 60);
                    right.set_size_request(98, 56);
                    source.set_visible(false);
                    artist.set_visible(true);
                    elapsed.set_visible(false);
                    duration.set_visible(false);
                    shuffle.set_visible(true);
                    repeat.set_visible(true);
                    volume.set_visible(false);
                }
                _ => {
                    now_playing.set_size_request(190, 56);
                    center.set_size_request(190, 60);
                    right.set_size_request(92, 56);
                    source.set_visible(false);
                    artist.set_visible(false);
                    elapsed.set_visible(false);
                    duration.set_visible(false);
                    shuffle.set_visible(false);
                    repeat.set_visible(false);
                    volume.set_visible(false);
                }
            }

            glib::ControlFlow::Continue
        });
    }

    fn apply_home_preferences(&self) {
        let config = self.config.borrow();
        self.visualizer
            .widget()
            .set_visible(config.show_home_visualizer);
        self.visualizer
            .set_active(config.show_home_visualizer && self.player.is_playing());
        self.lyrics
            .inline_widget()
            .set_visible(config.show_home_lyrics);
        self._theme.set_noctalia_enabled(config.noctalia_theme_sync);
        self._theme
            .set_blur_preferences(config.blur_mode, config.blur_opacity);
    }

    fn show_settings_dialog(self: &Rc<Self>) {
        let dialog = gtk::Dialog::builder()
            .transient_for(&self.window)
            .modal(true)
            .title(self.tr(Message::SettingsTitle))
            .default_width(560)
            .build();
        dialog.add_button(self.tr(Message::Close), gtk::ResponseType::Close);
        dialog.connect_response(|dialog, _| dialog.close());

        let content = dialog.content_area();
        content.set_spacing(14);
        content.set_margin_top(22);
        content.set_margin_bottom(22);
        content.set_margin_start(22);
        content.set_margin_end(22);

        let title = gtk::Label::new(Some(self.tr(Message::SettingsTitle)));
        title.set_xalign(0.0);
        title.add_css_class("title-2");
        let description = gtk::Label::new(Some(self.tr(Message::SettingsDescription)));
        description.set_xalign(0.0);
        description.set_wrap(true);
        description.add_css_class("dim-label");
        content.append(&title);
        content.append(&description);

        let config = self.config.borrow().clone();
        let language = gtk::DropDown::from_strings(&[
            AppLanguage::Portuguese.label(),
            AppLanguage::English.label(),
            AppLanguage::Spanish.label(),
        ]);
        language.set_selected(match config.language {
            AppLanguage::Portuguese => 0,
            AppLanguage::English => 1,
            AppLanguage::Spanish => 2,
        });
        content.append(&settings_dropdown_row(
            self.tr(Message::Language),
            self.tr(Message::LanguageDescription),
            &language,
        ));

        let source =
            gtk::DropDown::from_strings(&[self.tr(Message::LocalLibrary), "YouTube Music"]);
        source.set_selected(
            match config.startup_source.unwrap_or(StartupSource::YouTube) {
                StartupSource::Local => 0,
                StartupSource::YouTube => 1,
            },
        );
        content.append(&settings_dropdown_row(
            self.tr(Message::HomeSource),
            self.tr(Message::HomeSourceDescription),
            &source,
        ));

        let blur_mode = gtk::DropDown::from_strings(&[
            self.tr(Message::BlurCustom),
            self.tr(Message::BlurNoctalia),
            self.tr(Message::BlurOff),
        ]);
        blur_mode.set_selected(match config.blur_mode {
            BlurMode::Custom => 0,
            BlurMode::Noctalia => 1,
            BlurMode::Off => 2,
        });
        content.append(&settings_dropdown_row(
            self.tr(Message::WindowBlur),
            self.tr(Message::WindowBlurDescription),
            &blur_mode,
        ));

        let blur_opacity = gtk::Scale::with_range(gtk::Orientation::Horizontal, 45.0, 95.0, 1.0);
        blur_opacity.set_draw_value(true);
        blur_opacity.set_value(config.blur_opacity.clamp(0.45, 0.95) * 100.0);
        blur_opacity.set_value_pos(gtk::PositionType::Right);
        let blur_opacity_row = settings_scale_row(
            self.tr(Message::BlurOpacity),
            self.tr(Message::BlurOpacityDescription),
            &blur_opacity,
        );
        blur_opacity_row.set_visible(config.blur_mode == BlurMode::Custom);
        content.append(&blur_opacity_row);

        let visualizer = settings_switch(config.show_home_visualizer);
        content.append(&settings_switch_row(
            self.tr(Message::HomeVisualizer),
            self.tr(Message::HomeVisualizerDescription),
            &visualizer,
        ));

        let lyrics = settings_switch(config.show_home_lyrics);
        content.append(&settings_switch_row(
            self.tr(Message::HomeLyrics),
            self.tr(Message::HomeLyricsDescription),
            &lyrics,
        ));

        let auto_lyrics = settings_switch(config.auto_download_lyrics);
        content.append(&settings_switch_row(
            self.tr(Message::AutoLyrics),
            self.tr(Message::AutoLyricsDescription),
            &auto_lyrics,
        ));

        let youtube_sync = settings_switch(config.youtube_auto_sync);
        content.append(&settings_switch_row(
            self.tr(Message::YoutubeSync),
            self.tr(Message::YoutubeSyncDescription),
            &youtube_sync,
        ));

        let youtube_button = gtk::Button::with_label(self.tr(Message::YoutubeManageAction));
        youtube_button.add_css_class("suggested-action");
        content.append(&settings_button_row(
            self.tr(Message::YoutubeManage),
            self.tr(Message::YoutubeManageDescription),
            &youtube_button,
        ));

        let noctalia = settings_switch(config.noctalia_theme_sync);
        content.append(&settings_switch_row(
            self.tr(Message::NoctaliaSync),
            self.tr(Message::NoctaliaSyncDescription),
            &noctalia,
        ));

        {
            let weak = Rc::downgrade(self);
            let dialog = dialog.clone();
            language.connect_selected_notify(move |dropdown| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                controller.config.borrow_mut().language = match dropdown.selected() {
                    1 => AppLanguage::English,
                    2 => AppLanguage::Spanish,
                    _ => AppLanguage::Portuguese,
                };
                controller.save_config();
                controller.apply_translations();
                dialog.close();
                let controller = controller.clone();
                glib::idle_add_local_once(move || controller.show_settings_dialog());
            });
        }
        {
            let weak = Rc::downgrade(self);
            source.connect_selected_notify(move |dropdown| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                controller.set_startup_source(if dropdown.selected() == 0 {
                    StartupSource::Local
                } else {
                    StartupSource::YouTube
                });
            });
        }
        {
            let weak = Rc::downgrade(self);
            let opacity_row = blur_opacity_row.clone();
            blur_mode.connect_selected_notify(move |dropdown| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                let mode = match dropdown.selected() {
                    0 => BlurMode::Custom,
                    2 => BlurMode::Off,
                    _ => BlurMode::Noctalia,
                };
                opacity_row.set_visible(mode == BlurMode::Custom);
                controller.config.borrow_mut().blur_mode = mode;
                controller.save_config();
                controller.apply_home_preferences();
            });
        }
        {
            let weak = Rc::downgrade(self);
            let pending_save = Rc::new(RefCell::new(None::<glib::SourceId>));
            blur_opacity.connect_value_changed(move |scale| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                controller.config.borrow_mut().blur_opacity =
                    (scale.value() / 100.0).clamp(0.45, 0.95);
                if controller.config.borrow().blur_mode == BlurMode::Custom {
                    controller.apply_home_preferences();
                }

                if let Some(source) = pending_save.borrow_mut().take() {
                    source.remove();
                }
                let weak = weak.clone();
                let pending = pending_save.clone();
                let source = glib::timeout_add_local_once(Duration::from_millis(350), move || {
                    pending.borrow_mut().take();
                    if let Some(controller) = weak.upgrade() {
                        controller.save_config();
                    }
                });
                pending_save.borrow_mut().replace(source);
            });
        }
        {
            let weak = Rc::downgrade(self);
            youtube_button.connect_clicked(move |_| {
                if let Some(controller) = weak.upgrade() {
                    controller.show_youtube_settings_dialog();
                }
            });
        }

        for (switch, setting) in [
            (&visualizer, 0_u8),
            (&lyrics, 1),
            (&auto_lyrics, 2),
            (&youtube_sync, 3),
            (&noctalia, 4),
        ] {
            let weak = Rc::downgrade(self);
            switch.connect_active_notify(move |switch| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                let active = switch.is_active();
                {
                    let mut config = controller.config.borrow_mut();
                    match setting {
                        0 => config.show_home_visualizer = active,
                        1 => config.show_home_lyrics = active,
                        2 => config.auto_download_lyrics = active,
                        3 => config.youtube_auto_sync = active,
                        _ => config.noctalia_theme_sync = active,
                    }
                }
                controller.save_config();
                controller.apply_home_preferences();
            });
        }

        dialog.present();
    }

    fn show_youtube_settings_dialog(self: &Rc<Self>) {
        let dialog = gtk::Dialog::builder()
            .transient_for(&self.window)
            .modal(true)
            .title("YouTube Music")
            .default_width(760)
            .default_height(620)
            .build();
        dialog.add_button(self.tr(Message::Close), gtk::ResponseType::Close);

        let content = dialog.content_area();
        content.set_spacing(0);
        content.set_margin_top(0);
        content.set_margin_bottom(0);
        content.set_margin_start(0);
        content.set_margin_end(0);
        content.append(self.youtube_page.root());

        let youtube_root = self.youtube_page.root().clone();
        dialog.connect_response(move |dialog, _| {
            dialog.content_area().remove(&youtube_root);
            dialog.close();
        });
        dialog.present();
    }

    fn tr(&self, key: &str) -> &'static str {
        translate(self.config.borrow().language, key)
    }

    fn show_startup_source_dialog(self: &Rc<Self>, first_run: bool) {
        let dialog = gtk::Dialog::builder()
            .transient_for(&self.window)
            .modal(true)
            .title(if first_run {
                self.tr(Message::StartupWelcome)
            } else {
                self.tr(Message::StartupSourceTitle)
            })
            .default_width(480)
            .build();

        let content = dialog.content_area();
        content.set_spacing(14);
        content.set_margin_top(22);
        content.set_margin_bottom(22);
        content.set_margin_start(22);
        content.set_margin_end(22);

        let title = gtk::Label::new(Some(if first_run {
            self.tr(Message::StartupQuestion)
        } else {
            self.tr(Message::StartupChoose)
        }));
        title.set_wrap(true);
        title.set_xalign(0.0);
        title.add_css_class("title-2");

        let description = gtk::Label::new(Some(self.tr(Message::StartupDescription)));
        description.set_wrap(true);
        description.set_xalign(0.0);
        description.add_css_class("dim-label");

        let local_button = gtk::Button::with_label(self.tr(Message::UseLocalLibrary));
        local_button.set_tooltip_text(Some(self.tr(Message::UseLocalLibraryTooltip)));
        local_button.add_css_class("source-choice-button");

        let youtube_button = gtk::Button::with_label(self.tr(Message::UseYoutubeMusic));
        youtube_button.set_tooltip_text(Some(self.tr(Message::UseYoutubeMusicTooltip)));
        youtube_button.add_css_class("source-choice-button");
        youtube_button.add_css_class("suggested-action");

        let choices = gtk::Box::new(gtk::Orientation::Vertical, 10);
        choices.append(&local_button);
        choices.append(&youtube_button);

        content.append(&title);
        content.append(&description);
        content.append(&choices);

        if !first_run {
            dialog.add_button(self.tr(Message::Cancel), gtk::ResponseType::Cancel);
            dialog.connect_response(|dialog, _| dialog.close());
        }

        {
            let weak = Rc::downgrade(self);
            let dialog = dialog.clone();
            local_button.connect_clicked(move |_| {
                if let Some(controller) = weak.upgrade() {
                    controller.set_startup_source(StartupSource::Local);
                }
                dialog.close();
            });
        }
        {
            let weak = Rc::downgrade(self);
            let dialog = dialog.clone();
            youtube_button.connect_clicked(move |_| {
                if let Some(controller) = weak.upgrade() {
                    controller.set_startup_source(StartupSource::YouTube);
                }
                dialog.close();
            });
        }

        dialog.present();
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

        let sender = self.background_tx.clone();
        thread::spawn(move || {
            let result = library::scan_music_directory(&root);
            let _ = sender.send(BackgroundMessage::LibraryScanned { root, result });
        });
    }

    fn handle_background_messages(&self) {
        while let Ok(message) = self.background_rx.try_recv() {
            match message {
                BackgroundMessage::LibraryScanned { root, result } => {
                    self.scanning.set(false);
                    if self.config.borrow().music_directory.as_ref() != Some(&root) {
                        continue;
                    }
                    match result {
                        Ok(paths) => self.apply_scanned_library(paths),
                        Err(error) => self.show_error(&error),
                    }
                }
                BackgroundMessage::LyricsDownloaded {
                    path,
                    result,
                    notify,
                } => {
                    self.lyrics_pending.borrow_mut().remove(&path);
                    match result {
                        Ok(()) => {
                            let current_track = {
                                let mut state = self.state.borrow_mut();
                                let current = state.current;
                                let mut changed = None;
                                if let Some((index, track)) = state
                                    .tracks
                                    .iter_mut()
                                    .enumerate()
                                    .find(|(_, track)| track.path == path)
                                {
                                    track.reload_lyrics();
                                    changed = Some((index, track.clone()));
                                }
                                changed.filter(|(index, _)| Some(*index) == current)
                            };

                            if let Some((_, track)) = current_track {
                                self.rebuild_lyrics(&track);
                            }
                            self.refresh_browser();
                            if notify {
                                self.show_toast("Letras sincronizadas baixadas");
                            }
                        }
                        Err(error) => {
                            if notify {
                                self.show_toast(&error);
                            }
                        }
                    }
                }
                BackgroundMessage::YouTubeLyricsDownloaded {
                    video_id,
                    notify,
                    result,
                } => {
                    let current = self.youtube_state.borrow().as_ref().map(|state| {
                        (
                            state.item.video_id.clone(),
                            state.item.title.clone(),
                            state.item.artist.clone(),
                        )
                    });
                    if current
                        .as_ref()
                        .map(|(current_id, _, _)| current_id.as_str())
                        != Some(video_id.as_str())
                    {
                        continue;
                    }

                    match result {
                        Ok(lyrics) => {
                            if let Some(state) = self.youtube_state.borrow_mut().as_mut() {
                                state.lyrics = lyrics.clone();
                            }
                            self.rebuild_youtube_lyrics(&lyrics);
                            if notify {
                                self.show_toast("Letras sincronizadas do YouTube carregadas");
                            }
                        }
                        Err(error) => {
                            let title = current
                                .as_ref()
                                .map(|(_, title, _)| title.as_str())
                                .unwrap_or("esta música");
                            self.set_lyrics_message(&format!(
                                "No synchronized lyrics were found for {title}. {error}"
                            ));
                        }
                    }
                }
                BackgroundMessage::YouTubeStatus(result) => match result {
                    Ok(status) => {
                        self.youtube_page.set_status(&status);
                        if status.connected {
                            self.youtube_library.borrow_mut().connected = true;
                            let syncing = self.config.borrow().youtube_auto_sync
                                && self.sync_youtube_library(true, false);
                            if syncing {
                                self.youtube_page.set_loading(
                                    true,
                                    "Sincronizando biblioteca do YouTube Music…",
                                );
                            } else {
                                self.prefetch_youtube_playlist_cache();
                                self.prefetch_home_artist_profiles();
                            }
                        } else {
                            self.youtube_library.borrow_mut().clear();
                            clear_library_cache();
                            self.refresh_browser();
                        }
                    }
                    Err(error) => self.youtube_page.show_error(&error),
                },
                BackgroundMessage::YouTubeConnected(result) => match result {
                    Ok(status) => {
                        self.youtube_page.set_status(&status);
                        self.youtube_page
                            .set_loading(false, "YouTube Music connected");
                        {
                            let mut library = self.youtube_library.borrow_mut();
                            library.connected = true;
                            library.synced = false;
                        }
                        let _ = self.sync_youtube_library(true, false);
                        self.show_toast("Conta do YouTube Music conectada");
                    }
                    Err(error) => {
                        self.youtube_page.show_error(&error);
                        self.show_toast("Não foi possível conectar o YouTube Music");
                    }
                },
                BackgroundMessage::YouTubeDisconnected(result) => match result {
                    Ok(status) => {
                        self.youtube_page.set_status(&status);
                        self.youtube_page.set_loading(false, "YouTube Music");
                        self.youtube_page
                            .show_empty("Search for music or connect your account.");
                        self.youtube_library.borrow_mut().clear();
                        clear_library_cache();
                        self.refresh_browser();
                        self.show_toast("Conta do YouTube Music desconectada");
                    }
                    Err(error) => self.youtube_page.show_error(&error),
                },
                BackgroundMessage::YouTubeLibrarySynced { notify, result } => match result {
                    Ok(snapshot) => {
                        let counts = (
                            snapshot.library.len(),
                            snapshot.liked.len(),
                            snapshot.playlists.len(),
                        );
                        self.youtube_library.borrow_mut().apply(snapshot);
                        if let Err(error) = save_library_cache(&self.youtube_library.borrow()) {
                            eprintln!("Could not save the YouTube library cache: {error}");
                        }
                        self.youtube_page
                            .set_loading(false, "Library synchronized with Nocky");
                        self.refresh_browser();
                        self.prefetch_youtube_playlist_cache();
                        self.prefetch_youtube_collection_cache();
                        if notify {
                            self.show_toast(&format!(
                                "YouTube Music sincronizado: {} faixas, {} curtidas e {} playlists",
                                counts.0, counts.1, counts.2
                            ));
                        }
                    }
                    Err(error) => {
                        self.youtube_library.borrow_mut().syncing = false;
                        self.youtube_page.set_loading(false, "YouTube Music");
                        self.refresh_browser();
                        self.show_toast(&format!(
                            "Não foi possível sincronizar a biblioteca: {error}"
                        ));
                    }
                },
                BackgroundMessage::YouTubeBrowserPlaylist {
                    request_id,
                    playlist,
                    result,
                } => match result {
                    Ok(items) => {
                        if request_id != self.youtube_playlist_request_id.get() {
                            continue;
                        }
                        self.youtube_playlist_loading.set(false);
                        let browse_id = playlist.browse_id.clone();
                        self.youtube_library
                            .borrow_mut()
                            .playlist_loading
                            .remove(&browse_id);

                        if items.is_empty() {
                            self.youtube_library
                                .borrow_mut()
                                .playlist_tracks
                                .remove(&browse_id);
                            if self.is_open_youtube_playlist(&browse_id) {
                                self.refresh_browser();
                            }
                            self.show_toast(
                                "Esta playlist não retornou faixas reproduzíveis agora",
                            );
                        } else {
                            self.youtube_library
                                .borrow_mut()
                                .playlist_tracks
                                .insert(browse_id.clone(), items);
                            if cacheable_youtube_playlist(&playlist) {
                                if let Err(error) =
                                    save_library_cache(&self.youtube_library.borrow())
                                {
                                    eprintln!("Could not save the YouTube playlist cache: {error}");
                                }
                            }
                            if self.is_open_youtube_playlist(&browse_id) {
                                self.refresh_browser();
                            }
                        }

                        let pending = self.youtube_pending_playlist.borrow_mut().take();
                        if let Some(pending) = pending {
                            self.load_youtube_playlist_for_browser(pending);
                        }
                    }
                    Err(error) => {
                        if request_id != self.youtube_playlist_request_id.get() {
                            continue;
                        }
                        self.youtube_playlist_loading.set(false);
                        let browse_id = playlist.browse_id.clone();
                        self.youtube_library
                            .borrow_mut()
                            .playlist_loading
                            .remove(&browse_id);
                        if self.is_open_youtube_playlist(&browse_id) {
                            self.refresh_browser();
                        }
                        self.show_toast(&format!("Não foi possível carregar a playlist: {error}"));
                        let pending = self.youtube_pending_playlist.borrow_mut().take();
                        if let Some(pending) = pending {
                            self.load_youtube_playlist_for_browser(pending);
                        }
                    }
                },
                BackgroundMessage::YouTubeArtistOverview { key, result } => {
                    self.youtube_library
                        .borrow_mut()
                        .artist_loading
                        .remove(&key);
                    match result {
                        Ok(overview) => {
                            let mut library = self.youtube_library.borrow_mut();
                            library
                                .artist_profiles
                                .insert(key.clone(), overview.profile);
                            library.artist_albums.insert(key.clone(), overview.albums);
                            drop(library);
                            if let Err(error) = save_library_cache(&self.youtube_library.borrow()) {
                                eprintln!("Could not save YouTube artist details: {error}");
                            }
                        }
                        Err(error) => {
                            eprintln!("Could not load YouTube artist details: {error}");
                            if self.is_open_youtube_collection(&key) {
                                self.show_toast(&format!(
                                    "Não foi possível carregar os álbuns do artista: {error}"
                                ));
                            }
                        }
                    }
                    self.refresh_browser();
                }
                BackgroundMessage::YouTubeBrowserCollection { item, key, result } => {
                    self.youtube_library
                        .borrow_mut()
                        .collection_loading
                        .remove(&key);
                    match result {
                        Ok(items) if !items.is_empty() => {
                            self.youtube_library
                                .borrow_mut()
                                .collection_tracks
                                .insert(key.clone(), items);
                            if let Err(error) = save_library_cache(&self.youtube_library.borrow()) {
                                eprintln!("Could not save the YouTube collection cache: {error}");
                            }
                        }
                        Ok(_) => {
                            self.youtube_library
                                .borrow_mut()
                                .collection_tracks
                                .remove(&key);
                            self.show_toast(if item.result_type == "artist" {
                                "Este artista não retornou faixas reproduzíveis agora"
                            } else {
                                "Este álbum não retornou faixas reproduzíveis agora"
                            });
                        }
                        Err(error) => {
                            self.youtube_library
                                .borrow_mut()
                                .collection_tracks
                                .remove(&key);
                            self.show_toast(&format!(
                                "Não foi possível carregar {}: {error}",
                                if item.result_type == "artist" {
                                    "o artista"
                                } else {
                                    "o álbum"
                                }
                            ));
                        }
                    }
                    if self.is_open_youtube_collection(&key) {
                        self.refresh_browser();
                    }
                }
                BackgroundMessage::YouTubeCollectionsCached(result) => match result {
                    Ok(cached) => {
                        self.youtube_collection_prefetching.set(false);
                        if cached.is_empty() {
                            continue;
                        }
                        self.youtube_library
                            .borrow_mut()
                            .collection_tracks
                            .extend(cached);
                        if let Err(error) = save_library_cache(&self.youtube_library.borrow()) {
                            eprintln!("Could not save the YouTube collection cache: {error}");
                        }
                        self.refresh_browser();
                    }
                    Err(error) => {
                        self.youtube_collection_prefetching.set(false);
                        eprintln!("Could not pre-cache YouTube collections: {error}");
                    }
                },
                BackgroundMessage::YouTubePlaylistsCached(result) => match result {
                    Ok(cached) => {
                        self.youtube_playlist_prefetching.set(false);
                        if cached.is_empty() {
                            continue;
                        }
                        self.youtube_library
                            .borrow_mut()
                            .playlist_tracks
                            .extend(cached);
                        if let Err(error) = save_library_cache(&self.youtube_library.borrow()) {
                            eprintln!("Could not save the YouTube playlist cache: {error}");
                        }
                        self.refresh_browser();
                    }
                    Err(error) => {
                        self.youtube_playlist_prefetching.set(false);
                        eprintln!("Could not pre-cache YouTube playlists: {error}");
                    }
                },
                BackgroundMessage::YouTubeGlobalSearch {
                    request_id,
                    query,
                    result,
                } => {
                    if request_id != self.youtube_search_request_id.get()
                        || self.search_query.borrow().trim() != query.as_str()
                        || self.config.borrow().startup_source != Some(StartupSource::YouTube)
                    {
                        continue;
                    }

                    let mut library = self.youtube_library.borrow_mut();
                    match result {
                        Ok(mut categorized) => {
                            categorized.loading = false;
                            library.search = categorized;
                        }
                        Err(error) => {
                            library.search = YouTubeSearchResults {
                                query,
                                error,
                                ..YouTubeSearchResults::default()
                            };
                        }
                    }
                    drop(library);
                    self.refresh_browser();
                }
                BackgroundMessage::YouTubeItems { title, result } => match result {
                    Ok(items) => self.youtube_page.show_items(&title, items),
                    Err(error) => self.youtube_page.show_error(&error),
                },
                BackgroundMessage::YouTubeResolved {
                    request_id,
                    queue,
                    index,
                    item,
                    result,
                } => {
                    if request_id != self.youtube_request_id.get() {
                        continue;
                    }
                    match result {
                        Ok((stream, cover)) => {
                            self.apply_youtube_track(queue, index, item, stream, cover)
                        }
                        Err(error) => {
                            self.show_error(&error);
                            self.youtube_page.show_error(&error);
                        }
                    }
                }
            }
        }
    }

    fn apply_scanned_library(&self, data: Vec<TrackData>) {
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

    fn refresh_browser(&self) {
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
            &self.listening_history.borrow(),
            &query,
        );
        if !youtube_only {
            if let Some(current) = state.current {
                self.browser.select_track(current);
            }
        }
    }

    fn navigate_browser(&self, route: BrowserRoute) {
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
            &self.listening_history.borrow(),
            &query,
        );
        drop(query);
        drop(youtube);
        drop(config);
        drop(state);
        self.update_sidebar_active(&route);
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

    fn handle_browser_events(&self) {
        while let Some(event) = self.browser.try_recv() {
            match event {
                BrowserEvent::RefreshSearch => self.refresh_browser(),
                BrowserEvent::TrackActivated(index) => {
                    self.prepare_playback_queue(index);
                    self.select_track(index, true);
                }
                BrowserEvent::YouTubeTrackActivated { item, queue, index } => {
                    self.resolve_youtube_track(item, queue, index, false);
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
        self.update_footer_source();
        if let Some(index) = self.state.borrow().current {
            if let Some(track) = self.state.borrow().tracks.get(index) {
                self.begin_listening_session(format!("local:{}", track.path.display()));
            }
        }
        self.youtube_state.replace(None);
        self.reset_youtube_recovery();
        self.state.borrow_mut().current = Some(index);
        self.title.set_text(&track.title);
        self.artist.set_text(&track.artist);
        self.album.set_text(&track.album);
        self.mini_title.set_text(&track.title);
        self.mini_artist.set_text(&track.artist);
        self.hero_cover.set_path(track.cover_path.as_deref());
        self.mini_cover.set_path(track.cover_path.as_deref());
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
        let sender = self.background_tx.clone();
        thread::spawn(move || {
            let result = lyrics_provider::download_to_sidecar(&path, &lookup);
            let _ = sender.send(BackgroundMessage::LyricsDownloaded {
                path,
                result,
                notify,
            });
        });
    }

    fn request_youtube_lyrics(&self, item: &YouTubeItem, notify: bool) {
        if item.video_id.is_empty() {
            return;
        }
        let lookup = lyrics_provider::LyricsLookup {
            title: item.title.clone(),
            artist: item.artist.clone(),
            album: item.album.clone(),
        };
        let video_id = item.video_id.clone();
        let sender = self.background_tx.clone();
        thread::spawn(move || {
            let result = lyrics_provider::fetch_synced_lyrics(&lookup)
                .map(|contents| lyrics::parse_lrc(&contents));
            let _ = sender.send(BackgroundMessage::YouTubeLyricsDownloaded {
                video_id,
                notify,
                result,
            });
        });
    }

    fn toggle_favorite(&self) {
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

    fn prepare_playback_queue(&self, selected: usize) {
        let mut sequence = self.browser.visible_indices();
        if sequence.is_empty() || !sequence.contains(&selected) {
            sequence = (0..self.state.borrow().tracks.len()).collect();
        }
        self.state.borrow_mut().playback_queue = sequence;
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

    fn next_track(&self) {
        if self.playback_source.get() == PlaybackSource::YouTube {
            self.youtube_next_track();
            return;
        }
        let sequence = self.playback_sequence();
        if sequence.is_empty() {
            return;
        }
        let current = self.state.borrow().current;
        let next = if self.shuffle_enabled.get() {
            self.random_visible_index(&sequence, current)
        } else {
            current
                .and_then(|current| sequence.iter().position(|index| *index == current))
                .and_then(|position| sequence.get(position + 1).copied())
                .or_else(|| current.is_none().then_some(sequence[0]))
        };
        if let Some(next) = next {
            self.select_track(next, true);
        }
    }

    fn previous_track(&self) {
        if self.playback_source.get() == PlaybackSource::YouTube {
            self.youtube_previous_track();
            return;
        }
        if self.player.position_us() > 5_000_000 {
            self.seek_to(0, true);
            return;
        }

        let sequence = self.playback_sequence();
        let current = self.state.borrow().current;
        let previous = current
            .and_then(|current| sequence.iter().position(|index| *index == current))
            .and_then(|position| position.checked_sub(1))
            .and_then(|position| sequence.get(position).copied());

        if let Some(previous) = previous {
            self.select_track(previous, true);
        } else if current.is_some() {
            self.seek_to(0, true);
        }
    }

    fn random_visible_index(&self, sequence: &[usize], current: Option<usize>) -> Option<usize> {
        if sequence.is_empty() {
            return None;
        }
        if sequence.len() == 1 {
            return sequence.first().copied();
        }

        let mut value = self.rng_state.get();
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        self.rng_state.set(value);
        let mut candidate = sequence[value as usize % sequence.len()];
        if Some(candidate) == current {
            let position = sequence
                .iter()
                .position(|index| *index == candidate)
                .unwrap_or(0);
            candidate = sequence[(position + 1) % sequence.len()];
        }
        Some(candidate)
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
        if self.repeat_button.is_active() {
            self.seek_to(0, true);
            self.play_current();
            return;
        }

        if self.playback_source.get() == PlaybackSource::YouTube {
            let has_next = self.youtube_state.borrow().as_ref().is_some_and(|state| {
                self.shuffle_enabled.get() && state.queue.len() > 1
                    || state.current + 1 < state.queue.len()
            });
            if has_next {
                self.youtube_next_track();
            } else {
                let _ = self.player.pause();
                self.update_play_icons(false);
                self.mpris
                    .send(mpris::MprisUpdate::Playback(mpris::MprisPlayback::Stopped));
            }
            return;
        }

        let sequence = self.playback_sequence();
        let current = self.state.borrow().current;
        let has_next = if self.shuffle_enabled.get() {
            sequence.len() > 1
        } else {
            current
                .and_then(|current| sequence.iter().position(|index| *index == current))
                .is_some_and(|position| position + 1 < sequence.len())
        };

        if has_next {
            self.next_track();
        } else {
            let _ = self.player.pause();
            self.update_play_icons(false);
            self.mpris
                .send(mpris::MprisUpdate::Playback(mpris::MprisPlayback::Stopped));
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
                mpris::MprisCommand::Next => self.next_track(),
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
        let icon = if playing {
            "media-playback-pause-symbolic"
        } else {
            "media-playback-start-symbolic"
        };
        self.play_icon.set_icon_name(Some(icon));
        self.hero_play_icon.set_icon_name(Some(icon));
        self.visualizer
            .set_active(playing && self.visualizer.widget().is_visible());
        self.footer_progress.set_playing(playing);
    }

    fn begin_listening_session(&self, id: String) {
        self.listening_session_id.replace(Some(id));
        self.listening_session_recorded.set(false);
    }

    fn maybe_record_listening(&self) {
        if self.listening_session_recorded.get() || !self.player.is_playing() {
            return;
        }
        let listened_seconds = (self.player.position_us().max(0) / 1_000_000) as u64;
        let duration_seconds = (self.player.duration_us().max(0) / 1_000_000) as u64;
        let completed =
            duration_seconds > 0 && listened_seconds.saturating_mul(2) >= duration_seconds;
        if listened_seconds < 30 && !completed {
            return;
        }

        let recorded = match self.playback_source.get() {
            PlaybackSource::Local => {
                let state = self.state.borrow();
                let Some(index) = state.current else {
                    return;
                };
                let Some(track) = state.tracks.get(index) else {
                    return;
                };
                self.listening_history.borrow_mut().record(
                    track.path.to_string_lossy().into_owned(),
                    track.artist.clone(),
                    track.album.clone(),
                    ListeningSource::Local,
                    listened_seconds,
                    completed,
                )
            }
            PlaybackSource::YouTube => {
                let state = self.youtube_state.borrow();
                let Some(state) = state.as_ref() else {
                    return;
                };
                self.listening_history.borrow_mut().record(
                    state.item.video_id.clone(),
                    state.item.artist.clone(),
                    state.item.album.clone(),
                    ListeningSource::YouTube,
                    listened_seconds,
                    completed,
                )
            }
            PlaybackSource::None => false,
        };
        if recorded {
            self.listening_session_recorded.set(true);
            self.refresh_browser();
        }
    }

    fn refresh_progress(&self) {
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
        self.reset_youtube_recovery();
        self.title.set_text(self.tr(Message::IntegratedMusic));
        self.artist.set_text(self.tr(Message::NoTrackSelected));
        self.album.set_text(message);
        self.mini_title.set_text(self.tr(Message::NothingPlaying));
        self.mini_artist.set_text("Nocky");
        self.update_footer_source();
        self.lyrics.show_state(
            "As letras aparecerão aqui",
            Some("Reproduza uma música com letras sincronizadas para acompanhar cada verso."),
            "As letras aparecerão aqui",
            Some("Reproduza uma música com letras sincronizadas para ver o contexto."),
        );
        self.hero_cover.set_path(None);
        self.mini_cover.set_path(None);
        self.elapsed.set_text("0:00");
        self.duration.set_text("0:00");
        self.footer_elapsed.set_text("0:00");
        self.footer_duration.set_text("0:00");
        self.progress.set_value(0.0);
        self.footer_progress.set_fraction(0.0);
        self.update_play_icons(false);
        self.last_mpris_position.set(0);
        self.mpris.send(mpris::MprisUpdate::ClearMetadata);
        self.mpris
            .send(mpris::MprisUpdate::Playback(mpris::MprisPlayback::Stopped));
        self.mpris.send(mpris::MprisUpdate::Position(0));
        self.publish_mpris_capabilities();
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
    network_source && rejected
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

fn playback_error_message(message: &str) -> &str {
    let lower = message.to_ascii_lowercase();
    if lower.contains("forbidden") || lower.contains("(403)") || lower.contains("http 403") {
        "O YouTube recusou o stream desta faixa mesmo após a renovação."
    } else if lower.contains("souphttpsrc")
        || lower.contains("internal data stream error")
        || lower.contains("can't typefind stream")
    {
        "A reprodução online foi interrompida. Verifique a conexão e tente novamente."
    } else {
        "Não foi possível reproduzir esta faixa."
    }
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
    content.set_size_request(252, -1);
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
    revealer.set_child(Some(&content));
    revealer.add_css_class("sidebar");

    SidebarParts {
        revealer,
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

fn settings_switch(active: bool) -> gtk::Switch {
    gtk::Switch::builder()
        .active(active)
        .valign(gtk::Align::Center)
        .build()
}

fn settings_switch_row(title: &str, subtitle: &str, switch: &gtk::Switch) -> gtk::Box {
    let title_label = gtk::Label::new(Some(title));
    title_label.set_xalign(0.0);
    title_label.add_css_class("track-title");
    let subtitle_label = gtk::Label::new(Some(subtitle));
    subtitle_label.set_xalign(0.0);
    subtitle_label.set_wrap(true);
    subtitle_label.add_css_class("dim-label");

    let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
    text.set_hexpand(true);
    text.append(&title_label);
    text.append(&subtitle_label);

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.add_css_class("settings-row");
    row.append(&text);
    row.append(switch);
    row
}

fn settings_dropdown_row(title: &str, subtitle: &str, dropdown: &gtk::DropDown) -> gtk::Box {
    let title_label = gtk::Label::new(Some(title));
    title_label.set_xalign(0.0);
    title_label.add_css_class("track-title");
    let subtitle_label = gtk::Label::new(Some(subtitle));
    subtitle_label.set_xalign(0.0);
    subtitle_label.set_wrap(true);
    subtitle_label.add_css_class("dim-label");

    let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
    text.set_hexpand(true);
    text.append(&title_label);
    text.append(&subtitle_label);

    dropdown.set_valign(gtk::Align::Center);
    dropdown.set_width_request(170);

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.add_css_class("settings-row");
    row.append(&text);
    row.append(dropdown);
    row
}

fn settings_scale_row(title: &str, subtitle: &str, scale: &gtk::Scale) -> gtk::Box {
    let title_label = gtk::Label::new(Some(title));
    title_label.set_xalign(0.0);
    title_label.add_css_class("track-title");
    let subtitle_label = gtk::Label::new(Some(subtitle));
    subtitle_label.set_xalign(0.0);
    subtitle_label.set_wrap(true);
    subtitle_label.add_css_class("dim-label");

    let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
    text.set_hexpand(true);
    text.append(&title_label);
    text.append(&subtitle_label);

    scale.set_valign(gtk::Align::Center);
    scale.set_width_request(190);

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.add_css_class("settings-row");
    row.append(&text);
    row.append(scale);
    row
}

fn settings_button_row(title: &str, subtitle: &str, button: &gtk::Button) -> gtk::Box {
    let title_label = gtk::Label::new(Some(title));
    title_label.set_xalign(0.0);
    title_label.add_css_class("track-title");
    let subtitle_label = gtk::Label::new(Some(subtitle));
    subtitle_label.set_xalign(0.0);
    subtitle_label.set_wrap(true);
    subtitle_label.add_css_class("dim-label");

    let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
    text.set_hexpand(true);
    text.append(&title_label);
    text.append(&subtitle_label);

    button.set_valign(gtk::Align::Center);

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.add_css_class("settings-row");
    row.append(&text);
    row.append(button);
    row
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

fn build_main_menu(language: AppLanguage) -> gio::Menu {
    let tr = |message| i18n::text(language, message);
    let menu = gio::Menu::new();

    let library_section = gio::Menu::new();
    library_section.append(
        Some(tr(Message::MenuChooseMusicFolder)),
        Some("app.choose-library"),
    );
    library_section.append(Some(tr(Message::MenuRescanLibrary)), Some("app.rescan"));
    library_section.append(
        Some(tr(Message::MenuDownloadLyrics)),
        Some("app.download-lyrics"),
    );
    library_section.append(
        Some(tr(Message::MenuToggleAutomaticLyrics)),
        Some("app.toggle-auto-lyrics"),
    );
    menu.append_section(None, &library_section);

    let app_section = gio::Menu::new();
    app_section.append(Some(tr(Message::MenuSettings)), Some("app.settings"));
    app_section.append(Some(tr(Message::MenuAbout)), Some("app.about"));
    app_section.append(Some(tr(Message::MenuQuit)), Some("app.quit"));
    menu.append_section(None, &app_section);

    menu
}

#[derive(Clone)]
struct CoverView {
    stack: gtk::Stack,
    picture: gtk::Picture,
    size: i32,
}

impl CoverView {
    fn set_path(&self, path: Option<&Path>) {
        let Some(path) = path.filter(|path| path.is_file()) else {
            self.picture.set_paintable(None::<&gdk::Texture>);
            self.stack.set_visible_child_name("placeholder");
            return;
        };

        match square_cover_pixbuf(path, self.size) {
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

fn build_cover(size: i32) -> CoverView {
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
    picture.set_can_shrink(false);
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
        size,
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

fn format_duration_seconds(total_seconds: u64) -> String {
    if total_seconds == 0 {
        return "—".to_string();
    }
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{minutes}:{seconds:02}")
}

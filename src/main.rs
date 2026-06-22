mod browser;
mod config;
mod library;
mod lyrics;
mod lyrics_provider;
mod lyrics_view;
mod model;
mod mpris;
mod playback;
mod theme;
mod visualizer;
mod youtube;

use adw::prelude::*;
use browser::{BrowserEvent, BrowserRoute, LibraryBrowser};
use config::{AppLanguage, BlurMode, StartupSource};
use gtk::prelude::FileExt;
use gtk::{gdk, gio, glib};
use lyrics::LyricLine;
use lyrics_view::LyricsPresenter;
use model::{Track, TrackData};
use playback::{PlaybackEngine, PlaybackEvent};
use std::{
    cell::{Cell, RefCell},
    collections::{hash_map::DefaultHasher, HashMap, HashSet},
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use visualizer::SpectrumVisualizer;
use youtube::{
    cache_items_for_browser, clear_library_cache, download_cover, load_library_cache,
    save_library_cache, YouTubeBridge, YouTubeItem, YouTubeLibraryCache, YouTubeLibrarySnapshot,
    YouTubePage, YouTubePageEvent, YouTubeStatus, YouTubeStream,
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
    YouTubePlaylistsCached(Result<HashMap<String, Vec<YouTubeItem>>, String>),
    YouTubeItems {
        title: String,
        result: Result<Vec<YouTubeItem>, String>,
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
    albums_button: gtk::Button,
    artists_button: gtk::Button,
    playlists_button: gtk::Button,
    liked_button: gtk::Button,
}

struct AppController {
    window: adw::ApplicationWindow,
    toast_overlay: adw::ToastOverlay,
    player: PlaybackEngine,
    state: RefCell<AppState>,
    config: RefCell<config::AppConfig>,
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
    youtube_recovery_in_progress: Cell<bool>,
    youtube_recovery_attempted: Cell<bool>,
    youtube_recovery_resume_us: Cell<i64>,
    youtube_playlist_request_id: Cell<u64>,
    youtube_bridge: Option<Arc<YouTubeBridge>>,
    youtube_library: RefCell<YouTubeLibraryCache>,

    sidebar: gtk::Revealer,
    sidebar_all: gtk::Button,
    sidebar_albums: gtk::Button,
    sidebar_artists: gtk::Button,
    sidebar_playlists: gtk::Button,
    sidebar_liked: gtk::Button,
    views: adw::ViewStack,
    browser: LibraryBrowser,
    lyrics: LyricsPresenter,
    youtube_page: Rc<YouTubePage>,

    title: gtk::Label,
    artist: gtk::Label,
    album: gtk::Label,
    mini_title: gtk::Label,
    mini_artist: gtk::Label,
    music_stack: gtk::Stack,
    hero_cover: CoverView,
    mini_cover: CoverView,

    play_icon: gtk::Image,
    hero_play_icon: gtk::Image,
    favorite_icon: gtk::Image,
    progress: gtk::Scale,
    elapsed: gtk::Label,
    duration: gtk::Label,
    volume: gtk::Scale,
    lyrics_button: gtk::ToggleButton,
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
            .tooltip_text("Show or hide the sidebar")
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
            .tooltip_text("Search the library")
            .build();
        header.pack_end(&search_button);

        let folder_button = gtk::Button::builder()
            .icon_name("folder-open-symbolic")
            .tooltip_text("Choose the music folder")
            .build();
        header.pack_end(&folder_button);

        let menu = gio::Menu::new();
        let library_section = gio::Menu::new();
        library_section.append(Some("Choose Music Folder…"), Some("app.choose-library"));
        library_section.append(Some("Rescan Library"), Some("app.rescan"));
        library_section.append(
            Some("Download Lyrics for Current Track"),
            Some("app.download-lyrics"),
        );
        library_section.append(
            Some("Toggle Automatic Lyrics"),
            Some("app.toggle-auto-lyrics"),
        );
        menu.append_section(None, &library_section);

        let app_section = gio::Menu::new();
        app_section.append(Some("Settings"), Some("app.settings"));
        app_section.append(Some("About Nocky"), Some("app.about"));
        app_section.append(Some("Quit"), Some("app.quit"));
        menu.append_section(None, &app_section);

        let menu_button = gtk::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .menu_model(&menu)
            .build();
        header.pack_end(&menu_button);
        shell.append(&header);

        let search_bar = gtk::SearchBar::new();
        let search_entry = gtk::SearchEntry::builder()
            .placeholder_text("Buscar por título, artista ou álbum")
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

        let sidebar_parts = build_sidebar();
        body.append(&sidebar_parts.revealer);

        let title = gtk::Label::new(Some("Sua música, naturalmente integrada"));
        title.set_xalign(0.0);
        title.set_wrap(false);
        title.set_single_line_mode(true);
        title.set_width_chars(28);
        title.set_max_width_chars(28);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title.add_css_class("hero-title");

        let artist = gtk::Label::new(Some("Nenhuma faixa selecionada"));
        artist.set_xalign(0.0);
        artist.set_single_line_mode(true);
        artist.set_width_chars(28);
        artist.set_max_width_chars(28);
        artist.set_ellipsize(gtk::pango::EllipsizeMode::End);
        artist.add_css_class("hero-artist");

        let album = gtk::Label::new(Some("Escolha uma pasta de músicas para começar"));
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
        favorite.set_tooltip_text(Some("Add or remove from liked songs"));
        favorite.add_css_class("like-button");

        let now_heading = gtk::Label::new(Some("Reproduzindo agora"));
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

        let hero_cover = build_cover(230);
        hero_cover.stack.set_halign(gtk::Align::Center);

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
            .tooltip_text("Repeat current track")
            .build();
        repeat.add_css_class("media-control");
        let previous = gtk::Button::from_icon_name("media-skip-backward-symbolic");
        previous.set_tooltip_text(Some("Previous track"));
        previous.add_css_class("media-control");

        let hero_play_icon = gtk::Image::from_icon_name("media-playback-start-symbolic");
        hero_play_icon.set_pixel_size(24);
        let hero_play_button = gtk::Button::new();
        hero_play_button.set_child(Some(&hero_play_icon));
        hero_play_button.add_css_class("shell-play-button");
        hero_play_button.set_tooltip_text(Some("Play or pause"));

        let next = gtk::Button::from_icon_name("media-skip-forward-symbolic");
        next.set_tooltip_text(Some("Next track"));
        next.add_css_class("media-control");
        let shuffle = gtk::ToggleButton::builder()
            .icon_name("media-playlist-shuffle-symbolic")
            .tooltip_text("Shuffle")
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
        now_card.add_css_class("now-playing-card");
        now_card.append(&now_header);
        now_card.append(&hero_cover.stack);
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
        let empty_title = gtk::Label::new(Some("Escolha sua biblioteca musical"));
        empty_title.add_css_class("title-2");
        let empty_text = gtk::Label::new(Some(
            "Nocky scans the selected folder recursively and remembers it for the next launch.",
        ));
        empty_text.set_wrap(true);
        empty_text.set_justify(gtk::Justification::Center);
        empty_text.add_css_class("dim-label");
        let empty_add = gtk::Button::with_label("Escolher pasta de músicas");
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

        views.add_titled_with_icon(
            &music_stack,
            Some("music"),
            "Music",
            "folder-music-symbolic",
        );
        views.add_titled_with_icon(
            lyrics.full_widget(),
            Some("lyrics"),
            "Lyrics",
            "audio-input-microphone-symbolic",
        );

        let youtube_page = YouTubePage::new();
        body.append(&views);

        let mini_cover = build_cover(46);
        let mini_title = gtk::Label::new(Some("Nada reproduzindo"));
        mini_title.set_xalign(0.0);
        mini_title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        mini_title.add_css_class("now-title");
        let mini_artist = gtk::Label::new(Some("Nocky"));
        mini_artist.set_xalign(0.0);
        mini_artist.set_ellipsize(gtk::pango::EllipsizeMode::End);
        mini_artist.add_css_class("dim-label");
        mini_title.set_width_chars(30);
        mini_title.set_max_width_chars(30);
        mini_artist.set_width_chars(30);
        mini_artist.set_max_width_chars(30);
        let mini_text = gtk::Box::new(gtk::Orientation::Vertical, 2);
        mini_text.set_width_request(270);
        mini_text.set_hexpand(false);
        mini_text.add_css_class("footer-meta");
        mini_text.append(&mini_title);
        mini_text.append(&mini_artist);
        let now_playing = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        now_playing.set_size_request(370, 46);
        now_playing.set_hexpand(false);
        now_playing.append(&mini_cover.stack);
        now_playing.append(&mini_text);

        let footer_previous = gtk::Button::from_icon_name("media-skip-backward-symbolic");
        footer_previous.set_tooltip_text(Some("Faixa anterior"));
        footer_previous.add_css_class("flat");
        footer_previous.add_css_class("footer-control");

        let play_icon = gtk::Image::from_icon_name("media-playback-start-symbolic");
        let play = gtk::Button::new();
        play.set_child(Some(&play_icon));
        play.add_css_class("flat");
        play.add_css_class("mini-play-button");
        play.set_tooltip_text(Some("Reproduzir ou pausar"));

        let footer_next = gtk::Button::from_icon_name("media-skip-forward-symbolic");
        footer_next.set_tooltip_text(Some("Próxima faixa"));
        footer_next.add_css_class("flat");
        footer_next.add_css_class("footer-control");

        let footer_transport = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        footer_transport.set_halign(gtk::Align::Center);
        footer_transport.set_valign(gtk::Align::Center);
        footer_transport.append(&footer_previous);
        footer_transport.append(&play);
        footer_transport.append(&footer_next);

        let lyrics_button = gtk::ToggleButton::builder()
            .icon_name("audio-input-microphone-symbolic")
            .tooltip_text("Lyrics")
            .build();
        lyrics_button.add_css_class("flat");
        lyrics_button.add_css_class("footer-control");
        lyrics_button.add_css_class("footer-lyrics-button");
        let volume_icon = gtk::Image::from_icon_name("audio-volume-high-symbolic");
        let volume = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 0.01);
        volume.set_draw_value(false);
        volume.set_value(config.volume.clamp(0.0, 1.0));
        volume.set_size_request(110, -1);
        let right_controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        right_controls.set_halign(gtk::Align::End);
        right_controls.set_size_request(176, 46);
        right_controls.append(&lyrics_button);
        right_controls.append(&volume_icon);
        right_controls.append(&volume);

        let player_bar = gtk::CenterBox::new();
        player_bar.set_height_request(66);
        player_bar.add_css_class("player-bar");
        player_bar.set_start_widget(Some(&now_playing));
        player_bar.set_center_widget(Some(&footer_transport));
        player_bar.set_end_widget(Some(&right_controls));
        shell.append(&player_bar);

        let mpris = mpris::MprisBridge::start(config.volume);
        let youtube_bridge = YouTubeBridge::discover().ok().map(Arc::new);

        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos() as u64)
            .unwrap_or(0x9e37_79b9_7f4a_7c15);

        let controller = Rc::new(Self {
            window,
            toast_overlay,
            player,
            state: RefCell::new(AppState::default()),
            config: RefCell::new(config),
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
            youtube_recovery_in_progress: Cell::new(false),
            youtube_recovery_attempted: Cell::new(false),
            youtube_recovery_resume_us: Cell::new(0),
            youtube_playlist_request_id: Cell::new(0),
            youtube_bridge,
            youtube_library: RefCell::new(load_library_cache()),
            sidebar: sidebar_parts.revealer,
            sidebar_all: sidebar_parts.all_button,
            sidebar_albums: sidebar_parts.albums_button,
            sidebar_artists: sidebar_parts.artists_button,
            sidebar_playlists: sidebar_parts.playlists_button,
            sidebar_liked: sidebar_parts.liked_button,
            views,
            browser,
            lyrics,
            youtube_page,
            title,
            artist,
            album,
            mini_title,
            mini_artist,
            music_stack,
            hero_cover,
            mini_cover,
            play_icon,
            hero_play_icon,
            favorite_icon,
            progress,
            elapsed,
            duration,
            volume,
            lyrics_button,
            repeat_button: repeat.clone(),
            shuffle_button: shuffle.clone(),
            visualizer,
            _theme: theme,
        });
        controller.apply_home_preferences();

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
            search_entry.connect_search_changed(move |entry| {
                if let Some(controller) = weak.upgrade() {
                    controller.search_query.replace(entry.text().to_string());
                    controller.refresh_browser();
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
                    controller.mpris.send(mpris::MprisUpdate::Loop(enabled));
                }
            });
        }

        {
            let weak = Rc::downgrade(&controller);
            shuffle.connect_toggled(move |button| {
                if let Some(controller) = weak.upgrade() {
                    let enabled = button.is_active();
                    controller.shuffle_enabled.set(enabled);
                    controller.mpris.send(mpris::MprisUpdate::Shuffle(enabled));
                }
            });
        }

        {
            let weak = Rc::downgrade(&controller);
            favorite.connect_clicked(move |_| {
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
        let playlists = {
            let library = self.youtube_library.borrow();
            library
                .playlists
                .iter()
                .filter(|playlist| !playlist.browse_id.is_empty())
                .filter(|playlist| {
                    playlist.playlist_kind.is_empty() || playlist.playlist_kind == "library"
                })
                .filter(|playlist| !library.playlist_tracks.contains_key(&playlist.browse_id))
                .take(4)
                .cloned()
                .collect::<Vec<_>>()
        };
        if playlists.is_empty() {
            return;
        }

        let sender = self.background_tx.clone();
        thread::spawn(move || {
            let mut cached = HashMap::new();
            for playlist in playlists {
                let browse_id = playlist.browse_id.clone();
                match bridge.playlist(&playlist) {
                    Ok(mut items) => {
                        cache_items_for_browser(&mut items);
                        cached.insert(browse_id, items);
                    }
                    Err(error) => {
                        eprintln!(
                            "Could not pre-cache YouTube playlist '{}': {error}",
                            playlist.title
                        );
                    }
                }
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
        let request_id = self.youtube_playlist_request_id.get().wrapping_add(1);
        self.youtube_playlist_request_id.set(request_id);
        if self
            .youtube_library
            .borrow()
            .playlist_tracks
            .contains_key(&browse_id)
        {
            self.navigate_browser(BrowserRoute::YouTubePlaylist {
                title: playlist.title,
                browse_id,
            });
            return;
        }

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
                        "Automatic lyrics enabled"
                    } else {
                        "Automatic lyrics disabled"
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
            let window = self.window.clone();
            about.connect_activate(move |_, _| {
                let dialog = gtk::AboutDialog::builder()
                    .transient_for(&window)
                    .modal(true)
                    .program_name("Nocky")
                    .version(env!("CARGO_PKG_VERSION"))
                    .comments("A native GTK4/libadwaita music player designed for Noctalia Shell.")
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
            .title(self.tr("settings_title"))
            .default_width(560)
            .build();
        dialog.add_button(self.tr("close"), gtk::ResponseType::Close);
        dialog.connect_response(|dialog, _| dialog.close());

        let content = dialog.content_area();
        content.set_spacing(14);
        content.set_margin_top(22);
        content.set_margin_bottom(22);
        content.set_margin_start(22);
        content.set_margin_end(22);

        let title = gtk::Label::new(Some(self.tr("settings_title")));
        title.set_xalign(0.0);
        title.add_css_class("title-2");
        let description = gtk::Label::new(Some(self.tr("settings_description")));
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
            self.tr("language"),
            self.tr("language_description"),
            &language,
        ));

        let source = gtk::DropDown::from_strings(&["Local library", "YouTube Music"]);
        source.set_selected(
            match config.startup_source.unwrap_or(StartupSource::YouTube) {
                StartupSource::Local => 0,
                StartupSource::YouTube => 1,
            },
        );
        content.append(&settings_dropdown_row(
            self.tr("home_source"),
            self.tr("home_source_description"),
            &source,
        ));

        let blur_mode = gtk::DropDown::from_strings(&[
            self.tr("blur_custom"),
            self.tr("blur_noctalia"),
            self.tr("blur_off"),
        ]);
        blur_mode.set_selected(match config.blur_mode {
            BlurMode::Custom => 0,
            BlurMode::Noctalia => 1,
            BlurMode::Off => 2,
        });
        content.append(&settings_dropdown_row(
            self.tr("window_blur"),
            self.tr("window_blur_description"),
            &blur_mode,
        ));

        let blur_opacity = gtk::Scale::with_range(gtk::Orientation::Horizontal, 45.0, 95.0, 1.0);
        blur_opacity.set_draw_value(true);
        blur_opacity.set_value(config.blur_opacity.clamp(0.45, 0.95) * 100.0);
        blur_opacity.set_value_pos(gtk::PositionType::Right);
        let blur_opacity_row = settings_scale_row(
            self.tr("blur_opacity"),
            self.tr("blur_opacity_description"),
            &blur_opacity,
        );
        blur_opacity_row.set_visible(config.blur_mode == BlurMode::Custom);
        content.append(&blur_opacity_row);

        let visualizer = settings_switch(config.show_home_visualizer);
        content.append(&settings_switch_row(
            self.tr("home_visualizer"),
            self.tr("home_visualizer_description"),
            &visualizer,
        ));

        let lyrics = settings_switch(config.show_home_lyrics);
        content.append(&settings_switch_row(
            self.tr("home_lyrics"),
            self.tr("home_lyrics_description"),
            &lyrics,
        ));

        let auto_lyrics = settings_switch(config.auto_download_lyrics);
        content.append(&settings_switch_row(
            self.tr("auto_lyrics"),
            self.tr("auto_lyrics_description"),
            &auto_lyrics,
        ));

        let youtube_sync = settings_switch(config.youtube_auto_sync);
        content.append(&settings_switch_row(
            self.tr("youtube_sync"),
            self.tr("youtube_sync_description"),
            &youtube_sync,
        ));

        let youtube_button = gtk::Button::with_label(self.tr("youtube_manage_action"));
        youtube_button.add_css_class("suggested-action");
        content.append(&settings_button_row(
            self.tr("youtube_manage"),
            self.tr("youtube_manage_description"),
            &youtube_button,
        ));

        let noctalia = settings_switch(config.noctalia_theme_sync);
        content.append(&settings_switch_row(
            self.tr("noctalia_sync"),
            self.tr("noctalia_sync_description"),
            &noctalia,
        ));

        {
            let weak = Rc::downgrade(self);
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
        dialog.add_button(self.tr("close"), gtk::ResponseType::Close);

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
                "Boas-vindas ao Nocky"
            } else {
                "Fonte da Home"
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
            "Qual fonte a Home deve mostrar?"
        } else {
            "Escolha o que aparece na Home"
        }));
        title.set_wrap(true);
        title.set_xalign(0.0);
        title.add_css_class("title-2");

        let description = gtk::Label::new(Some(
            "O Nocky sempre abre na Home. Esta escolha controla se a Home mostra músicas locais ou sua biblioteca conectada do YouTube Music.",
        ));
        description.set_wrap(true);
        description.set_xalign(0.0);
        description.add_css_class("dim-label");

        let local_button = gtk::Button::with_label("Usar biblioteca local");
        local_button.set_tooltip_text(Some(
            "Mostra álbuns, artistas, playlists e curtidas salvas neste computador",
        ));
        local_button.add_css_class("source-choice-button");

        let youtube_button = gtk::Button::with_label("Usar YouTube Music");
        youtube_button.set_tooltip_text(Some(
            "Mostra sua biblioteca online conectada na Home e mantém a busca do YouTube disponível",
        ));
        youtube_button.add_css_class("source-choice-button");
        youtube_button.add_css_class("suggested-action");

        let choices = gtk::Box::new(gtk::Orientation::Vertical, 10);
        choices.append(&local_button);
        choices.append(&youtube_button);

        content.append(&title);
        content.append(&description);
        content.append(&choices);

        if !first_run {
            dialog.add_button("Cancelar", gtk::ResponseType::Cancel);
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
            .title("Escolha sua pasta de músicas")
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
                            self.prefetch_youtube_playlist_cache();
                            if self.config.borrow().youtube_auto_sync
                                && self.sync_youtube_library(true, false)
                            {
                                self.youtube_page.set_loading(
                                    true,
                                    "Sincronizando biblioteca do YouTube Music…",
                                );
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
                        let browse_id = playlist.browse_id.clone();
                        self.youtube_library
                            .borrow_mut()
                            .playlist_tracks
                            .insert(browse_id.clone(), items);
                        if let Err(error) = save_library_cache(&self.youtube_library.borrow()) {
                            eprintln!("Could not save the YouTube playlist cache: {error}");
                        }
                        self.navigate_browser(BrowserRoute::YouTubePlaylist {
                            title: playlist.title,
                            browse_id,
                        });
                    }
                    Err(error) => {
                        if request_id != self.youtube_playlist_request_id.get() {
                            continue;
                        }
                        self.show_toast(&format!("Não foi possível carregar a playlist: {error}"))
                    }
                },
                BackgroundMessage::YouTubePlaylistsCached(result) => match result {
                    Ok(cached) => {
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
                    Err(error) => eprintln!("Could not pre-cache YouTube playlists: {error}"),
                },
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
        let has_library = !effective_tracks.is_empty() || youtube.has_content() || youtube.syncing;
        self.music_stack
            .set_visible_child_name(if has_library { "library" } else { "empty" });
        self.browser
            .refresh(effective_tracks, &effective_config, &youtube, &query);
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
            "Added to liked songs"
        } else {
            "Removed from liked songs"
        });
    }

    fn update_favorite_icon(&self, path: &std::path::Path) {
        let liked = self.config.borrow().is_liked(path);
        self.favorite_icon
            .set_icon_name(Some("emblem-favorite-symbolic"));
        self.favorite_icon
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
    }

    fn refresh_progress(&self) {
        let timestamp = self.player.position_us().max(0);
        let duration = self.player.duration_us().max(0);
        let fraction = if duration > 0 {
            timestamp as f64 / duration as f64
        } else {
            0.0
        };

        self.updating_progress.set(true);
        self.progress.set_value(fraction.clamp(0.0, 1.0));
        self.updating_progress.set(false);
        self.elapsed.set_text(&format_time(timestamp));
        self.duration.set_text(&format_time(duration));
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
        self.title.set_text("Sua música, naturalmente integrada");
        self.artist.set_text("Nenhuma faixa selecionada");
        self.album.set_text(message);
        self.mini_title.set_text("Nada reproduzindo");
        self.mini_artist.set_text("Nocky");
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
        self.progress.set_value(0.0);
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

fn build_sidebar() -> SidebarParts {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
    content.set_size_request(252, -1);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(10);
    content.set_margin_end(10);
    content.add_css_class("sidebar-content");

    let all_button = sidebar_row("view-list-symbolic", "Biblioteca", true);
    let albums_button = sidebar_row("folder-music-symbolic", "Álbuns", false);
    let artists_button = sidebar_row("avatar-default-symbolic", "Artistas", false);
    let playlists_button = sidebar_row("view-list-symbolic", "Playlists", false);
    content.append(&all_button);
    content.append(&albums_button);
    content.append(&artists_button);
    content.append(&playlists_button);

    let section = gtk::Label::new(Some("COLEÇÃO LOCAL"));
    section.set_xalign(0.0);
    section.set_margin_top(18);
    section.set_margin_start(10);
    section.add_css_class("section-title");
    content.append(&section);
    let liked_button = sidebar_row("emblem-favorite-symbolic", "Músicas curtidas", false);
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
        albums_button,
        artists_button,
        playlists_button,
        liked_button,
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

fn translate(language: AppLanguage, key: &str) -> &'static str {
    match language {
        AppLanguage::Portuguese => match key {
            "settings_title" => "Configurações",
            "settings_description" => "Ajuste a Home, integrações e idioma do Nocky.",
            "language" => "Idioma",
            "language_description" => "Escolha o idioma preferido para as configurações.",
            "home_source" => "Fonte da Home",
            "home_source_description" => {
                "Escolha se a Home mostra a biblioteca local ou o YouTube Music."
            }
            "home_visualizer" => "Visualizador na Home",
            "home_visualizer_description" => {
                "Mostra ou oculta o espectro de áudio no painel reproduzindo agora."
            }
            "home_lyrics" => "Letras na Home",
            "home_lyrics_description" => {
                "Mostra ou oculta a prévia sincronizada de letras na Home."
            }
            "auto_lyrics" => "Buscar letras automaticamente",
            "auto_lyrics_description" => {
                "Procura letras sincronizadas para músicas locais e do YouTube."
            }
            "youtube_sync" => "Sincronização automática do YouTube",
            "youtube_sync_description" => {
                "Atualiza biblioteca, curtidas e playlists ao abrir o app."
            }
            "youtube_manage" => "YouTube Music",
            "youtube_manage_description" => {
                "Conecte a conta, sincronize manualmente e use a busca do YouTube Music."
            }
            "youtube_manage_action" => "Gerenciar",
            "noctalia_sync" => "Sincronizar com Noctalia Shell",
            "noctalia_sync_description" => {
                "Aplica o CSS gerado pelo Noctalia em ~/.config/nocky/theme.css."
            }
            "window_blur" => "Desfoque da janela",
            "window_blur_description" => {
                "Escolha o vidro do Nocky, a aparência sincronizada do Noctalia ou uma janela opaca."
            }
            "blur_custom" => "Desfoque",
            "blur_noctalia" => "Desfoque do Noctalia",
            "blur_off" => "Desativado",
            "blur_opacity" => "Transparência do vidro",
            "blur_opacity_description" => {
                "Controla a transparência usada no modo Desfoque."
            }
            "settings_saved" => "Configurações salvas",
            "close" => "Fechar",
            _ => "Nocky",
        },
        AppLanguage::English => match key {
            "settings_title" => "Settings",
            "settings_description" => "Tune Nocky's Home, integrations, and language.",
            "language" => "Language",
            "language_description" => "Choose the preferred language for settings.",
            "home_source" => "Home source",
            "home_source_description" => {
                "Choose whether Home shows your local library or YouTube Music."
            }
            "home_visualizer" => "Home visualizer",
            "home_visualizer_description" => {
                "Show or hide the audio spectrum in the now-playing panel."
            }
            "home_lyrics" => "Home lyrics",
            "home_lyrics_description" => "Show or hide the synchronized lyrics preview on Home.",
            "auto_lyrics" => "Fetch lyrics automatically",
            "auto_lyrics_description" => "Search synchronized lyrics for local and YouTube tracks.",
            "youtube_sync" => "Automatic YouTube sync",
            "youtube_sync_description" => {
                "Refresh library, liked songs, and playlists when the app opens."
            }
            "youtube_manage" => "YouTube Music",
            "youtube_manage_description" => {
                "Connect your account, sync manually, and use YouTube Music search."
            }
            "youtube_manage_action" => "Manage",
            "noctalia_sync" => "Sync with Noctalia Shell",
            "noctalia_sync_description" => {
                "Apply the CSS generated by Noctalia at ~/.config/nocky/theme.css."
            }
            "window_blur" => "Window blur",
            "window_blur_description" => {
                "Use Nocky's glass, follow Noctalia's appearance, or keep the window opaque."
            }
            "blur_custom" => "Blur",
            "blur_noctalia" => "Noctalia blur",
            "blur_off" => "Off",
            "blur_opacity" => "Glass transparency",
            "blur_opacity_description" => {
                "Controls the transparency used by the Blur mode."
            }
            "settings_saved" => "Settings saved",
            "close" => "Close",
            _ => "Nocky",
        },
        AppLanguage::Spanish => match key {
            "settings_title" => "Configuración",
            "settings_description" => "Ajusta la Home, las integraciones y el idioma de Nocky.",
            "language" => "Idioma",
            "language_description" => "Elige el idioma preferido para la configuración.",
            "home_source" => "Fuente de Home",
            "home_source_description" => {
                "Elige si Home muestra la biblioteca local o YouTube Music."
            }
            "home_visualizer" => "Visualizador en Home",
            "home_visualizer_description" => {
                "Muestra u oculta el espectro de audio en el panel de reproducción."
            }
            "home_lyrics" => "Letras en Home",
            "home_lyrics_description" => {
                "Muestra u oculta la vista previa de letras sincronizadas en Home."
            }
            "auto_lyrics" => "Buscar letras automáticamente",
            "auto_lyrics_description" => {
                "Busca letras sincronizadas para canciones locales y de YouTube."
            }
            "youtube_sync" => "Sincronización automática de YouTube",
            "youtube_sync_description" => {
                "Actualiza biblioteca, favoritos y playlists al abrir la app."
            }
            "youtube_manage" => "YouTube Music",
            "youtube_manage_description" => {
                "Conecta la cuenta, sincroniza manualmente y usa la búsqueda de YouTube Music."
            }
            "youtube_manage_action" => "Gestionar",
            "noctalia_sync" => "Sincronizar con Noctalia Shell",
            "noctalia_sync_description" => {
                "Aplica el CSS generado por Noctalia en ~/.config/nocky/theme.css."
            }
            "window_blur" => "Desenfoque de la ventana",
            "window_blur_description" => {
                "Usa el cristal de Nocky, sigue la apariencia de Noctalia o deja la ventana opaca."
            }
            "blur_custom" => "Desenfoque",
            "blur_noctalia" => "Desenfoque de Noctalia",
            "blur_off" => "Desactivado",
            "blur_opacity" => "Transparencia del cristal",
            "blur_opacity_description" => {
                "Controla la transparencia usada por el modo Desenfoque."
            }
            "settings_saved" => "Configuración guardada",
            "close" => "Cerrar",
            _ => "Nocky",
        },
    }
}

fn sidebar_row(icon_name: &str, text: &str, active: bool) -> gtk::Button {
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
    button
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
    placeholder.set_hexpand(true);
    placeholder.set_vexpand(true);
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

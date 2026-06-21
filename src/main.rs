mod browser;
mod config;
mod library;
mod lyrics;
mod lyrics_provider;
mod model;
mod mpris;
mod playback;
mod theme;
mod visualizer;

use adw::prelude::*;
use gtk::prelude::FileExt;
use gtk::{gdk, gio, glib};
use browser::{BrowserEvent, BrowserRoute, LibraryBrowser};
use model::{Track, TrackData};
use playback::{PlaybackEngine, PlaybackEvent};
use visualizer::SpectrumVisualizer;
use std::{
    cell::{Cell, RefCell},
    collections::{hash_map::DefaultHasher, HashSet},
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    rc::Rc,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const APP_ID: &str = "io.github.maylton.Nocky";

#[derive(Default)]
struct AppState {
    tracks: Vec<Track>,
    current: Option<usize>,
    playback_queue: Vec<usize>,
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

    sidebar: gtk::Revealer,
    sidebar_all: gtk::Button,
    sidebar_albums: gtk::Button,
    sidebar_artists: gtk::Button,
    sidebar_playlists: gtk::Button,
    sidebar_liked: gtk::Button,
    views: adw::ViewStack,
    browser: LibraryBrowser,
    lyrics_box: gtk::Box,

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
    inline_lyric_lines: Vec<gtk::Label>,

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
        library_section.append(Some("Download Lyrics for Current Track"), Some("app.download-lyrics"));
        library_section.append(Some("Toggle Automatic Lyrics"), Some("app.toggle-auto-lyrics"));
        menu.append_section(None, &library_section);

        let app_section = gio::Menu::new();
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
            .placeholder_text("Search by title, artist, or album")
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

        let title = gtk::Label::new(Some("Your music, naturally integrated"));
        title.set_xalign(0.0);
        title.set_wrap(false);
        title.set_single_line_mode(true);
        title.set_width_chars(28);
        title.set_max_width_chars(28);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title.add_css_class("hero-title");

        let artist = gtk::Label::new(Some("No track selected"));
        artist.set_xalign(0.0);
        artist.set_single_line_mode(true);
        artist.set_width_chars(28);
        artist.set_max_width_chars(28);
        artist.set_ellipsize(gtk::pango::EllipsizeMode::End);
        artist.add_css_class("hero-artist");

        let album = gtk::Label::new(Some("Choose a local music folder to begin"));
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

        let inline_lyrics = gtk::Box::new(gtk::Orientation::Vertical, 4);
        inline_lyrics.set_margin_top(4);
        inline_lyrics.set_margin_bottom(2);
        inline_lyrics.set_vexpand(true);
        inline_lyrics.set_valign(gtk::Align::Center);
        inline_lyrics.add_css_class("inline-lyrics-panel");

        let mut inline_lyric_lines = Vec::with_capacity(5);
        for index in 0..5 {
            let label = gtk::Label::new(None);
            label.set_wrap(true);
            label.set_justify(gtk::Justification::Center);
            label.set_halign(gtk::Align::Center);
            label.set_hexpand(true);
            label.add_css_class("inline-lyric-line");
            match index {
                2 => label.add_css_class("inline-lyric-current"),
                1 | 3 => label.add_css_class("inline-lyric-near"),
                _ => label.add_css_class("inline-lyric-far"),
            }
            inline_lyrics.append(&label);
            inline_lyric_lines.push(label);
        }
        inline_lyric_lines[2].set_text("Lyrics will appear here");
        inline_lyric_lines[3]
            .set_text("Play a song with synchronized lyrics to see the surrounding lines");

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
        now_card.append(&inline_lyrics);

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
        let empty_title = gtk::Label::new(Some("Choose your music library"));
        empty_title.add_css_class("title-2");
        let empty_text = gtk::Label::new(Some(
            "Nocky scans the selected folder recursively and remembers it for the next launch.",
        ));
        empty_text.set_wrap(true);
        empty_text.set_justify(gtk::Justification::Center);
        empty_text.add_css_class("dim-label");
        let empty_add = gtk::Button::with_label("Choose music folder");
        empty_add.add_css_class("suggested-action");
        empty_add.add_css_class("pill");
        empty_state.append(&empty_icon);
        empty_state.append(&empty_title);
        empty_state.append(&empty_text);
        empty_state.append(&empty_add);

        let music_stack = gtk::Stack::new();
        music_stack.set_transition_type(gtk::StackTransitionType::Crossfade);
        music_stack.add_named(&empty_state, Some("empty"));
        music_stack.add_named(&dashboard, Some("library"));
        music_stack.set_visible_child_name("empty");

        let lyrics_box = gtk::Box::new(gtk::Orientation::Vertical, 22);
        lyrics_box.set_margin_top(56);
        lyrics_box.set_margin_bottom(56);
        lyrics_box.set_margin_start(36);
        lyrics_box.set_margin_end(36);
        lyrics_box.set_halign(gtk::Align::Center);
        let lyrics_scroll = gtk::ScrolledWindow::new();
        lyrics_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        lyrics_scroll.set_child(Some(&lyrics_box));

        views.add_titled_with_icon(
            &music_stack,
            Some("music"),
            "Music",
            "folder-music-symbolic",
        );
        views.add_titled_with_icon(
            &lyrics_scroll,
            Some("lyrics"),
            "Lyrics",
            "audio-input-microphone-symbolic",
        );
        body.append(&views);

        let mini_cover = build_cover(46);
        let mini_title = gtk::Label::new(Some("Nothing playing"));
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
            sidebar: sidebar_parts.revealer,
            sidebar_all: sidebar_parts.all_button,
            sidebar_albums: sidebar_parts.albums_button,
            sidebar_artists: sidebar_parts.artists_button,
            sidebar_playlists: sidebar_parts.playlists_button,
            sidebar_liked: sidebar_parts.liked_button,
            views,
            browser,
            lyrics_box,
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
            inline_lyric_lines,
            _theme: theme,
        });

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
                        .set_visible_child_name(if button.is_active() { "lyrics" } else { "music" });
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

        controller
    }

    fn setup_callbacks(self: &Rc<Self>) {
        self.mpris.send(mpris::MprisUpdate::Volume(self.volume.value()));
        self.mpris.send(mpris::MprisUpdate::Loop(self.repeat_button.is_active()));
        self.mpris.send(mpris::MprisUpdate::Shuffle(self.shuffle_button.is_active()));
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
            self.volume.connect_value_changed(move |scale| {
                if let Some(controller) = weak.upgrade() {
                    let value = scale.value().clamp(0.0, 1.0);
                    controller.player.set_volume(value);
                    controller.config.borrow_mut().volume = value;
                    controller.save_config();
                    controller.mpris.send(mpris::MprisUpdate::Volume(value));
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
            glib::timeout_add_local(Duration::from_millis(50), move || {
                let Some(controller) = weak.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                controller.handle_background_messages();
                controller.handle_browser_events();
                controller.handle_mpris_commands();
                controller.handle_playback_events();
                controller.refresh_progress();
                glib::ControlFlow::Continue
            });
        }
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
                    let current = controller.state.borrow().current;
                    if let Some(index) = current {
                        controller.request_lyrics(index, true, true);
                    } else {
                        controller.show_toast("Select a track first");
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
                        if let Some(index) = controller.state.borrow().current {
                            controller.request_lyrics(index, false, false);
                        }
                    }
                }
            });
        }
        app.add_action(&toggle_auto);

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

    fn load_saved_library(self: &Rc<Self>) {
        if self.config.borrow().music_directory.is_some() {
            self.scan_library();
        }
    }

    fn choose_library_folder(self: &Rc<Self>) {
        let dialog = gtk::FileDialog::builder()
            .title("Choose your music folder")
            .accept_label("Select")
            .modal(true)
            .build();

        if let Some(path) = self.config.borrow().music_directory.as_ref() {
            let folder = gio::File::for_path(path);
            dialog.set_initial_folder(Some(&folder));
        }

        let weak = Rc::downgrade(self);
        dialog.select_folder(
            Some(&self.window),
            gio::Cancellable::NONE,
            move |result| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                let Ok(folder) = result else {
                    return;
                };
                let Some(path) = folder.path() else {
                    controller.show_toast("Only local folders are supported right now");
                    return;
                };

                controller.config.borrow_mut().music_directory = Some(path);
                controller.save_config();
                controller.scan_library();
            },
        );
    }

    fn scan_library(&self) {
        if self.scanning.replace(true) {
            self.show_toast("The library is already being scanned");
            return;
        }

        let Some(root) = self.config.borrow().music_directory.clone() else {
            self.scanning.set(false);
            self.show_toast("Choose a music folder first");
            return;
        };

        self.show_toast("Scanning the music library…");
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
                                self.show_toast("Synchronized lyrics downloaded");
                            }
                        }
                        Err(error) => {
                            if notify {
                                self.show_toast(&error);
                            }
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
            self.select_track(selected.unwrap_or(0), false);
            self.show_toast(&format!("{count} tracks found"));
        } else {
            self.reset_now_playing("No supported audio files were found");
            self.show_toast("No supported audio files were found in this folder");
        }
    }

    fn refresh_browser(&self) {
        let state = self.state.borrow();
        let config = self.config.borrow();
        let query = self.search_query.borrow();
        self.music_stack.set_visible_child_name(if state.tracks.is_empty() {
            "empty"
        } else {
            "library"
        });
        self.browser.refresh(&state.tracks, &config, &query);
        if let Some(current) = state.current {
            self.browser.select_track(current);
        }
    }

    fn navigate_browser(&self, route: BrowserRoute) {
        let state = self.state.borrow();
        let config = self.config.borrow();
        let query = self.search_query.borrow();
        self.browser.navigate(route.clone(), &state.tracks, &config, &query);
        drop(query);
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
            BrowserRoute::Albums | BrowserRoute::Album(_) => {
                self.sidebar_albums.add_css_class("active")
            }
            BrowserRoute::Artists | BrowserRoute::Artist(_) => {
                self.sidebar_artists.add_css_class("active")
            }
            BrowserRoute::Playlists | BrowserRoute::Playlist(_) => {
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
                },
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
                    let removed = self
                        .config
                        .borrow_mut()
                        .remove_from_playlist(&name, &path);
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
                self.show_toast("Lyrics are already being searched");
            }
            return;
        }

        if notify {
            self.show_toast("Searching for synchronized lyrics…");
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

    fn toggle_favorite(&self) {
        let path = {
            let state = self.state.borrow();
            let Some(track) = state.current.and_then(|index| state.tracks.get(index)) else {
                self.show_toast("Select a track first");
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
        self.favorite_icon.set_icon_name(Some("emblem-favorite-symbolic"));
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
            BrowserRoute::Albums | BrowserRoute::Artists | BrowserRoute::Playlists => {
                (0..self.state.borrow().tracks.len()).collect()
            }
            _ => visible,
        }
    }

    fn toggle_playback(&self) {
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
            let position = sequence.iter().position(|index| *index == candidate).unwrap_or(0);
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
                PlaybackEvent::DurationChanged => self.publish_mpris_capabilities(),
                PlaybackEvent::Spectrum(values) => self.visualizer.set_values(&values),
                PlaybackEvent::Error(error) => {
                    self.update_play_icons(false);
                    self.mpris
                        .send(mpris::MprisUpdate::Playback(mpris::MprisPlayback::Stopped));
                    self.show_error(&error);
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
                    if self.state.borrow().current.is_none() {
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

        self.mpris.send(mpris::MprisUpdate::Metadata(mpris::MprisTrack {
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
        let has_tracks = !state.tracks.is_empty();
        let can_seek = state
            .current
            .and_then(|index| state.tracks.get(index))
            .is_some_and(|track| track.duration_seconds > 0)
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
        self.visualizer.set_active(playing);
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
        while let Some(child) = self.lyrics_box.first_child() {
            self.lyrics_box.remove(&child);
        }

        if track.lyrics.is_empty() {
            let title = gtk::Label::new(Some("No synchronized lyrics available yet"));
            title.add_css_class("title-2");
            let hint = gtk::Label::new(Some(if self.config.borrow().auto_download_lyrics {
                "Automatic LRCLIB lookup is enabled. Use the menu to retry whenever needed."
            } else {
                "Use the menu to download lyrics, or place a matching .lrc file beside the song."
            }));
            hint.set_wrap(true);
            hint.set_justify(gtk::Justification::Center);
            hint.add_css_class("dim-label");
            self.lyrics_box.append(&title);
            self.lyrics_box.append(&hint);
            self.clear_inline_lyrics();
            self.inline_lyric_lines[2]
                .set_text("No synchronized lyrics available yet");
            self.inline_lyric_lines[3].set_text(if self.config.borrow().auto_download_lyrics {
                "Automatic LRCLIB lookup is enabled. You can also open the Lyrics page for the full view."
            } else {
                "Use the menu to download lyrics, or open the Lyrics page for the full view."
            });
            return;
        }

        for line in &track.lyrics {
            let label = gtk::Label::new(Some(&line.text));
            label.set_wrap(true);
            label.set_justify(gtk::Justification::Center);
            label.set_halign(gtk::Align::Center);
            label.add_css_class("lyric-line");
            self.lyrics_box.append(&label);
        }
        self.update_inline_lyric_preview(track, None);
    }

    fn highlight_lyric(&self, timestamp: i64) {
        let state = self.state.borrow();
        let Some(track) = state.current.and_then(|index| state.tracks.get(index)) else {
            return;
        };
        if track.lyrics.is_empty() {
            return;
        }

        let current_index = track
            .lyrics
            .iter()
            .enumerate()
            .rev()
            .find(|(_, line)| timestamp >= line.timestamp_us)
            .map(|(index, _)| index);

        self.update_inline_lyric_preview(track, current_index);

        let mut child = self.lyrics_box.first_child();
        let mut index = 0;
        while let Some(widget) = child {
            widget.remove_css_class("current-lyric");
            widget.remove_css_class("past-lyric");
            if Some(index) == current_index {
                widget.add_css_class("current-lyric");
            } else if current_index.is_some_and(|current| index < current) {
                widget.add_css_class("past-lyric");
            }
            child = widget.next_sibling();
            index += 1;
        }
    }


    fn clear_inline_lyrics(&self) {
        for label in &self.inline_lyric_lines {
            label.set_text("");
        }
    }

    fn update_inline_lyric_preview(&self, track: &Track, current_index: Option<usize>) {
        self.clear_inline_lyrics();

        if track.lyrics.is_empty() {
            self.inline_lyric_lines[2]
                .set_text("No synchronized lyrics available yet");
            self.inline_lyric_lines[3].set_text(if self.config.borrow().auto_download_lyrics {
                "Automatic LRCLIB lookup is enabled. You can also open the Lyrics page for the full view."
            } else {
                "Use the menu to download lyrics, or open the Lyrics page for the full view."
            });
            return;
        }

        let visible = track
            .lyrics
            .iter()
            .enumerate()
            .filter(|(_, line)| !line.text.trim().is_empty())
            .collect::<Vec<_>>();

        if visible.is_empty() {
            self.inline_lyric_lines[2].set_text("♪");
            return;
        }

        let active_visible = current_index
            .and_then(|current| visible.iter().position(|(index, _)| *index == current))
            .unwrap_or(0);

        for (slot, offset) in (-2_isize..=2).enumerate() {
            let position = active_visible as isize + offset;
            if position < 0 || position >= visible.len() as isize {
                continue;
            }
            self.inline_lyric_lines[slot].set_text(visible[position as usize].1.text.trim());
        }
    }

    fn reset_now_playing(&self, message: &str) {
        let _ = self.player.stop();
        self.title.set_text("Your music, naturally integrated");
        self.artist.set_text("No track selected");
        self.album.set_text(message);
        self.mini_title.set_text("Nothing playing");
        self.mini_artist.set_text("Nocky");
        self.clear_inline_lyrics();
        self.inline_lyric_lines[2].set_text("Lyrics will appear here");
        self.inline_lyric_lines[3]
            .set_text("Play a song with synchronized lyrics to see the surrounding lines");
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
        self.toast_overlay.add_toast(adw::Toast::new(message));
    }

    fn show_error(&self, message: &str) {
        eprintln!("Nocky error: {message}");
        self.album.set_text(&format!("Error: {message}"));
        self.show_toast(message);
    }
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

        match gdk_pixbuf::Pixbuf::from_file_at_scale(path, self.size, self.size, false) {
            Ok(pixbuf) => {
                let texture = gdk::Texture::for_pixbuf(&pixbuf);
                self.picture.set_paintable(Some(&texture));
                self.stack.set_visible_child_name("picture");
            }
            Err(error) => {
                eprintln!("Could not load cover {}: {error}", path.display());
                self.picture.set_paintable(None::<&gdk::Texture>);
                self.stack.set_visible_child_name("placeholder");
            }
        }
    }
}

fn build_cover(size: i32) -> CoverView {
    let icon = gtk::Image::from_icon_name("audio-x-generic-symbolic");
    icon.set_pixel_size((size as f64 * 0.30) as i32);
    icon.add_css_class("cover-icon");
    icon.set_halign(gtk::Align::Center);
    icon.set_valign(gtk::Align::Center);

    let placeholder = gtk::Box::new(gtk::Orientation::Vertical, 0);
    placeholder.set_width_request(size);
    placeholder.set_height_request(size);
    placeholder.set_halign(gtk::Align::Center);
    placeholder.set_valign(gtk::Align::Center);
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
    format!(
        "/io/github/maylton/Nocky/track_{:016x}",
        hasher.finish()
    )
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

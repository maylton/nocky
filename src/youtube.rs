use gtk::glib;
use gtk::prelude::*;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    rc::Rc,
    sync::{
        mpsc::{self, Receiver, Sender},
        OnceLock,
    },
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct YouTubeItem {
    pub result_type: String,
    pub title: String,
    pub subtitle: String,
    pub video_id: String,
    pub browse_id: String,
    pub album: String,
    pub artist: String,
    pub playlist_kind: String,
    pub params: String,
    pub duration_seconds: u64,
    pub thumbnail_url: String,
    pub cover_path: String,
}

impl YouTubeItem {
    pub fn playable(&self) -> bool {
        !self.video_id.is_empty()
    }

    pub fn cached_cover(&self) -> Option<&Path> {
        let path = Path::new(&self.cover_path);
        (!self.cover_path.is_empty() && path.is_file()).then_some(path)
    }
}

pub fn cacheable_youtube_playlist(item: &YouTubeItem) -> bool {
    item.result_type == "playlist"
        && !item.browse_id.is_empty()
        && (item.playlist_kind.is_empty() || item.playlist_kind == "library")
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct YouTubeStatus {
    pub connected: bool,
    pub account: String,
    pub storage: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct YouTubeLibrarySnapshot {
    pub library: Vec<YouTubeItem>,
    pub liked: Vec<YouTubeItem>,
    pub playlists: Vec<YouTubeItem>,
    pub suggested_albums: Vec<YouTubeItem>,
    pub suggested_artists: Vec<YouTubeItem>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct YouTubeCollectionEntry {
    pub title: String,
    pub subtitle: String,
    pub detail: String,
    pub cover_path: String,
    pub item_count: usize,
}

impl YouTubeCollectionEntry {
    pub fn cached_cover(&self) -> Option<&Path> {
        let path = Path::new(&self.cover_path);
        (!self.cover_path.is_empty() && path.is_file()).then_some(path)
    }
}

#[derive(Clone, Debug, Default)]
pub struct YouTubeLibraryCache {
    pub connected: bool,
    pub syncing: bool,
    pub synced: bool,
    pub library: Vec<YouTubeItem>,
    pub liked: Vec<YouTubeItem>,
    pub playlists: Vec<YouTubeItem>,
    pub suggested_albums: Vec<YouTubeItem>,
    pub suggested_artists: Vec<YouTubeItem>,
    pub playlist_tracks: HashMap<String, Vec<YouTubeItem>>,
    pub albums: Vec<YouTubeCollectionEntry>,
    pub artists: Vec<YouTubeCollectionEntry>,
}

impl YouTubeLibraryCache {
    pub fn has_content(&self) -> bool {
        !self.library.is_empty() || !self.liked.is_empty() || !self.playlists.is_empty()
    }

    pub fn clear(&mut self) {
        self.connected = false;
        self.syncing = false;
        self.synced = false;
        self.library.clear();
        self.liked.clear();
        self.playlists.clear();
        self.suggested_albums.clear();
        self.suggested_artists.clear();
        self.playlist_tracks.clear();
        self.albums.clear();
        self.artists.clear();
    }

    pub fn apply(&mut self, snapshot: YouTubeLibrarySnapshot) {
        self.syncing = false;
        self.synced = true;
        self.library = snapshot.library;
        self.liked = snapshot.liked;
        self.playlists = snapshot.playlists;
        self.suggested_albums = snapshot.suggested_albums;
        self.suggested_artists = snapshot.suggested_artists;

        let valid_playlists = self
            .playlists
            .iter()
            .filter(|item| cacheable_youtube_playlist(item))
            .map(|item| item.browse_id.clone())
            .collect::<HashSet<_>>();
        self.playlist_tracks
            .retain(|browse_id, _| valid_playlists.contains(browse_id));
        self.rebuild_collections();
    }

    pub fn rebuild_collections(&mut self) {
        let catalog = youtube_catalog(&self.library, &self.liked);
        self.albums = build_album_cache(&catalog);
        self.artists = build_artist_cache(&catalog);
        merge_suggested_collections(&mut self.albums, &self.suggested_albums, "album");
        merge_suggested_collections(&mut self.artists, &self.suggested_artists, "artist");
    }
}

const LIBRARY_CACHE_VERSION: u32 = 4;
const BROWSER_COVER_SIZE: u32 = 512;
const PLAYER_COVER_SIZE: u32 = 1200;

static COVER_CLIENT: OnceLock<Option<reqwest::blocking::Client>> = OnceLock::new();

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct PersistedYouTubeLibraryCache {
    version: u32,
    saved_at: u64,
    library: Vec<YouTubeItem>,
    liked: Vec<YouTubeItem>,
    playlists: Vec<YouTubeItem>,
    suggested_albums: Vec<YouTubeItem>,
    suggested_artists: Vec<YouTubeItem>,
    playlist_tracks: HashMap<String, Vec<YouTubeItem>>,
    albums: Vec<YouTubeCollectionEntry>,
    artists: Vec<YouTubeCollectionEntry>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct YouTubeStream {
    pub video_id: String,
    pub stream_url: String,
    pub webpage_url: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_seconds: u64,
    pub thumbnail_url: String,
    pub http_headers: HashMap<String, String>,
    pub expires_at: f64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct YouTubeHomeSuggestions {
    pub playlists: Vec<YouTubeItem>,
    pub albums: Vec<YouTubeItem>,
    pub artists: Vec<YouTubeItem>,
}

#[derive(Debug, Deserialize)]
struct HelperResponse<T> {
    ok: bool,
    result: Option<T>,
    error: Option<String>,
}

#[derive(Clone, Debug)]
pub enum YouTubePageEvent {
    Search {
        query: String,
        filter: String,
    },
    Connect(String),
    Disconnect,
    SyncLibrary,
    LoadLibrary,
    LoadLiked,
    LoadPlaylists,
    Activate {
        item: YouTubeItem,
        queue: Vec<YouTubeItem>,
        index: usize,
    },
    OpenPlaylist(YouTubeItem),
}

pub struct YouTubeBridge {
    python: PathBuf,
    helper: PathBuf,
}

impl YouTubeBridge {
    pub fn discover() -> Result<Self, String> {
        let helper = helper_path().ok_or_else(|| {
            "The Nocky YouTube helper was not found. Reinstall Nocky 0.2.4.".to_string()
        })?;
        let python = python_path().ok_or_else(|| {
            "The YouTube Music Python runtime is missing or incomplete. Run ./scripts/setup-youtube-runtime.sh for development, or reinstall with ./install.sh --install-youtube.".to_string()
        })?;
        Ok(Self { python, helper })
    }

    pub fn status(&self) -> Result<YouTubeStatus, String> {
        self.run("status", json!({}))
    }

    pub fn connect(&self, raw: &str) -> Result<YouTubeStatus, String> {
        self.run("connect", json!({ "raw": raw }))
    }

    pub fn disconnect(&self) -> Result<YouTubeStatus, String> {
        self.run("disconnect", json!({}))
    }

    pub fn search(&self, query: &str, filter: &str) -> Result<Vec<YouTubeItem>, String> {
        self.run(
            "search",
            json!({ "query": query, "filter": filter, "limit": 30 }),
        )
    }

    pub fn library(&self) -> Result<Vec<YouTubeItem>, String> {
        self.run("library", json!({ "limit": 200 }))
    }

    pub fn liked(&self) -> Result<Vec<YouTubeItem>, String> {
        self.run("liked", json!({ "limit": 200 }))
    }

    pub fn playlists(&self) -> Result<Vec<YouTubeItem>, String> {
        self.run("playlists", json!({ "limit": 150, "home_limit": 8 }))
    }

    fn library_playlists(&self) -> Result<Vec<YouTubeItem>, String> {
        self.run("playlists", json!({ "limit": 150, "home_limit": 0 }))
    }

    pub fn home(&self) -> Result<YouTubeHomeSuggestions, String> {
        self.run("home", json!({ "limit": 8 }))
    }

    pub fn sync_library(&self) -> Result<YouTubeLibrarySnapshot, String> {
        let home = self.home().unwrap_or_else(|error| {
            eprintln!("Could not load YouTube Music home suggestions: {error}");
            YouTubeHomeSuggestions::default()
        });
        let mut playlists = self.library_playlists()?;
        extend_unique_youtube_items(&mut playlists, home.playlists);
        let mut snapshot = YouTubeLibrarySnapshot {
            library: self.library()?,
            liked: self.liked()?,
            playlists,
            suggested_albums: home.albums,
            suggested_artists: home.artists,
        };
        cache_library_covers(&mut snapshot);
        Ok(snapshot)
    }

    pub fn playlist(&self, playlist: &YouTubeItem) -> Result<Vec<YouTubeItem>, String> {
        self.run(
            "playlist",
            json!({
                "browse_id": playlist.browse_id,
                "video_id": playlist.video_id,
                "playlist_kind": playlist.playlist_kind,
                "params": playlist.params,
                "limit": 300,
            }),
        )
    }

    pub fn resolve(&self, video_id: &str, force: bool) -> Result<YouTubeStream, String> {
        self.run("resolve", json!({ "video_id": video_id, "force": force }))
    }

    pub fn preload_streams(&self, queue: &[YouTubeItem], current_index: usize, limit: usize) {
        if queue.is_empty() || limit == 0 {
            return;
        }

        let mut seen = HashSet::new();
        for item in queue
            .iter()
            .skip(current_index.saturating_add(1))
            .take(limit)
        {
            if item.video_id.is_empty() || !seen.insert(item.video_id.clone()) {
                continue;
            }
            let _ = self.resolve(&item.video_id, false);
        }
    }

    fn run<T: DeserializeOwned>(
        &self,
        command: &str,
        payload: serde_json::Value,
    ) -> Result<T, String> {
        let mut child = Command::new(&self.python)
            .arg(&self.helper)
            .arg(command)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("Could not start the YouTube helper: {error}"))?;

        if let Some(mut stdin) = child.stdin.take() {
            serde_json::to_writer(&mut stdin, &payload)
                .map_err(|error| format!("Could not send data to the YouTube helper: {error}"))?;
        }

        let output = child
            .wait_with_output()
            .map_err(|error| format!("The YouTube helper did not finish: {error}"))?;
        let response: HelperResponse<T> =
            serde_json::from_slice(&output.stdout).map_err(|error| {
                let stderr = String::from_utf8_lossy(&output.stderr);
                format!("Invalid response from the YouTube helper: {error}. {stderr}")
            })?;

        if !response.ok {
            return Err(response
                .error
                .unwrap_or_else(|| "The YouTube helper reported an unknown error".to_string()));
        }
        response
            .result
            .ok_or_else(|| "The YouTube helper returned no result".to_string())
    }
}

pub struct YouTubePage {
    root: gtk::Box,
    status: gtk::Label,
    connect_button: gtk::Button,
    disconnect_button: gtk::Button,
    private_actions: gtk::Box,
    auth_revealer: gtk::Revealer,
    auth_buffer: gtk::TextBuffer,
    search_entry: gtk::SearchEntry,
    filter: gtk::DropDown,
    heading: gtk::Label,
    spinner: gtk::Spinner,
    results: gtk::ListBox,
    items: RefCell<Vec<YouTubeItem>>,
    event_tx: Sender<YouTubePageEvent>,
    event_rx: Receiver<YouTubePageEvent>,
}

impl YouTubePage {
    pub fn new() -> Rc<Self> {
        let (event_tx, event_rx) = mpsc::channel();

        let root = gtk::Box::new(gtk::Orientation::Vertical, 14);
        root.set_margin_top(20);
        root.set_margin_bottom(20);
        root.set_margin_start(24);
        root.set_margin_end(24);
        root.set_vexpand(true);
        root.add_css_class("youtube-page");

        let title = gtk::Label::new(Some("YouTube Music"));
        title.set_xalign(0.0);
        title.add_css_class("title-1");
        let subtitle = gtk::Label::new(Some(
            "Busque no catálogo ou conecte a sessão do navegador para acessar sua biblioteca.",
        ));
        subtitle.set_xalign(0.0);
        subtitle.set_wrap(true);
        subtitle.add_css_class("dim-label");

        let status = gtk::Label::new(Some("Verificando conta..."));
        status.set_xalign(0.0);
        status.set_hexpand(true);
        status.add_css_class("youtube-status");
        let connect_button = gtk::Button::with_label("Conectar conta");
        connect_button.add_css_class("suggested-action");
        let disconnect_button = gtk::Button::with_label("Desconectar");
        disconnect_button.add_css_class("flat");
        disconnect_button.set_visible(false);
        let account_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        account_row.append(&status);
        account_row.append(&connect_button);
        account_row.append(&disconnect_button);

        let auth_text = gtk::Label::new(Some(
            "Com a conta aberta em music.youtube.com, copie uma requisição bem-sucedida como cURL ou copie o cabeçalho Cookie e cole abaixo. A sessão é salva no Secret Service quando disponível.",
        ));
        auth_text.set_wrap(true);
        auth_text.set_xalign(0.0);
        auth_text.add_css_class("dim-label");
        let auth_buffer = gtk::TextBuffer::new(None::<&gtk::TextTagTable>);
        let auth_view = gtk::TextView::with_buffer(&auth_buffer);
        auth_view.set_wrap_mode(gtk::WrapMode::WordChar);
        auth_view.set_monospace(true);
        auth_view.set_top_margin(8);
        auth_view.set_bottom_margin(8);
        auth_view.set_left_margin(8);
        auth_view.set_right_margin(8);
        let auth_scroll = gtk::ScrolledWindow::new();
        auth_scroll.set_min_content_height(110);
        auth_scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
        auth_scroll.set_child(Some(&auth_view));
        auth_scroll.add_css_class("youtube-auth-input");
        let import_button = gtk::Button::with_label("Importar sessão");
        import_button.add_css_class("suggested-action");
        let cancel_auth = gtk::Button::with_label("Cancelar");
        cancel_auth.add_css_class("flat");
        let auth_buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        auth_buttons.set_halign(gtk::Align::End);
        auth_buttons.append(&cancel_auth);
        auth_buttons.append(&import_button);
        let auth_box = gtk::Box::new(gtk::Orientation::Vertical, 10);
        auth_box.add_css_class("youtube-auth-card");
        auth_box.append(&auth_text);
        auth_box.append(&auth_scroll);
        auth_box.append(&auth_buttons);
        let auth_revealer = gtk::Revealer::new();
        auth_revealer.set_transition_type(gtk::RevealerTransitionType::SlideDown);
        auth_revealer.set_child(Some(&auth_box));

        let search_entry = gtk::SearchEntry::builder()
            .placeholder_text("Buscar músicas, artistas, álbuns ou playlists")
            .hexpand(true)
            .build();
        let filter = gtk::DropDown::from_strings(&[
            "Músicas",
            "Tudo",
            "Vídeos",
            "Álbuns",
            "Artistas",
            "Playlists",
        ]);
        filter.set_selected(0);
        let search_button = gtk::Button::with_label("Buscar");
        search_button.add_css_class("suggested-action");
        let search_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        search_row.append(&search_entry);
        search_row.append(&filter);
        search_row.append(&search_button);

        let sync_button = gtk::Button::with_label("Sincronizar com Nocky");
        sync_button.add_css_class("suggested-action");
        let library_button = gtk::Button::with_label("Biblioteca");
        let liked_button = gtk::Button::with_label("Curtidas");
        let playlists_button = gtk::Button::with_label("Playlists");
        for button in [&library_button, &liked_button, &playlists_button] {
            button.add_css_class("pill");
        }
        let private_actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        private_actions.append(&sync_button);
        private_actions.append(&library_button);
        private_actions.append(&liked_button);
        private_actions.append(&playlists_button);
        private_actions.set_sensitive(false);

        let heading = gtk::Label::new(Some("Buscar no YouTube Music"));
        heading.set_xalign(0.0);
        heading.set_hexpand(true);
        heading.add_css_class("title-3");
        let spinner = gtk::Spinner::new();
        let results_header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        results_header.append(&heading);
        results_header.append(&spinner);

        let results = gtk::ListBox::new();
        results.set_selection_mode(gtk::SelectionMode::None);
        results.add_css_class("boxed-list");
        results.add_css_class("youtube-results");
        let results_scroll = gtk::ScrolledWindow::new();
        results_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        results_scroll.set_vexpand(true);
        results_scroll.set_child(Some(&results));

        root.append(&title);
        root.append(&subtitle);
        root.append(&account_row);
        root.append(&auth_revealer);
        root.append(&search_row);
        root.append(&private_actions);
        root.append(&results_header);
        root.append(&results_scroll);

        let page = Rc::new(Self {
            root,
            status,
            connect_button,
            disconnect_button,
            private_actions,
            auth_revealer,
            auth_buffer,
            search_entry,
            filter,
            heading,
            spinner,
            results,
            items: RefCell::new(Vec::new()),
            event_tx,
            event_rx,
        });

        {
            let button = page.connect_button.clone();
            let weak = Rc::downgrade(&page);
            button.connect_clicked(move |_| {
                if let Some(page) = weak.upgrade() {
                    page.auth_revealer.set_reveal_child(true);
                }
            });
        }
        {
            let weak = Rc::downgrade(&page);
            cancel_auth.connect_clicked(move |_| {
                if let Some(page) = weak.upgrade() {
                    page.auth_revealer.set_reveal_child(false);
                }
            });
        }
        {
            let weak = Rc::downgrade(&page);
            import_button.connect_clicked(move |_| {
                let Some(page) = weak.upgrade() else {
                    return;
                };
                let raw = page
                    .auth_buffer
                    .text(
                        &page.auth_buffer.start_iter(),
                        &page.auth_buffer.end_iter(),
                        false,
                    )
                    .to_string();
                if !raw.trim().is_empty() {
                    let _ = page.event_tx.send(YouTubePageEvent::Connect(raw));
                }
            });
        }
        {
            let sender = page.event_tx.clone();
            let button = page.disconnect_button.clone();
            button.connect_clicked(move |_| {
                let _ = sender.send(YouTubePageEvent::Disconnect);
            });
        }
        {
            let weak = Rc::downgrade(&page);
            search_button.connect_clicked(move |_| {
                if let Some(page) = weak.upgrade() {
                    page.emit_search();
                }
            });
        }
        {
            let entry = page.search_entry.clone();
            let weak = Rc::downgrade(&page);
            entry.connect_activate(move |_| {
                if let Some(page) = weak.upgrade() {
                    page.emit_search();
                }
            });
        }
        {
            let sender = page.event_tx.clone();
            sync_button.connect_clicked(move |_| {
                let _ = sender.send(YouTubePageEvent::SyncLibrary);
            });
        }
        {
            let sender = page.event_tx.clone();
            library_button.connect_clicked(move |_| {
                let _ = sender.send(YouTubePageEvent::LoadLibrary);
            });
        }
        {
            let sender = page.event_tx.clone();
            liked_button.connect_clicked(move |_| {
                let _ = sender.send(YouTubePageEvent::LoadLiked);
            });
        }
        {
            let sender = page.event_tx.clone();
            playlists_button.connect_clicked(move |_| {
                let _ = sender.send(YouTubePageEvent::LoadPlaylists);
            });
        }
        {
            let results = page.results.clone();
            let weak = Rc::downgrade(&page);
            results.connect_row_activated(move |_, row| {
                let Some(page) = weak.upgrade() else {
                    return;
                };
                let index = row.index().max(0) as usize;
                let Some(item) = page.items.borrow().get(index).cloned() else {
                    return;
                };
                if item.playable() {
                    let queue = page
                        .items
                        .borrow()
                        .iter()
                        .filter(|item| item.playable())
                        .cloned()
                        .collect::<Vec<_>>();
                    let selected = queue
                        .iter()
                        .position(|candidate| candidate.video_id == item.video_id)
                        .unwrap_or(0);
                    let _ = page.event_tx.send(YouTubePageEvent::Activate {
                        item,
                        queue,
                        index: selected,
                    });
                } else if item.result_type == "playlist" && !item.browse_id.is_empty() {
                    let _ = page.event_tx.send(YouTubePageEvent::OpenPlaylist(item));
                }
            });
        }

        page.show_empty("Busque uma música ou conecte sua conta.");
        page
    }

    pub fn root(&self) -> &gtk::Box {
        &self.root
    }

    pub fn try_recv(&self) -> Option<YouTubePageEvent> {
        self.event_rx.try_recv().ok()
    }

    pub fn set_status(&self, status: &YouTubeStatus) {
        if status.connected {
            let account = if status.account.trim().is_empty() {
                "Conta conectada"
            } else {
                status.account.as_str()
            };
            self.status.set_text(&format!("Conectado: {account}"));
            self.connect_button.set_visible(false);
            self.disconnect_button.set_visible(true);
            self.private_actions.set_sensitive(true);
            self.auth_revealer.set_reveal_child(false);
            self.auth_buffer.set_text("");
        } else {
            self.status
                .set_text("Não conectado - a busca pública continua disponível");
            self.connect_button.set_visible(true);
            self.disconnect_button.set_visible(false);
            self.private_actions.set_sensitive(false);
        }
    }

    pub fn set_loading(&self, loading: bool, title: &str) {
        self.heading.set_text(title);
        if loading {
            self.spinner.start();
        } else {
            self.spinner.stop();
        }
    }

    pub fn show_items(&self, title: &str, items: Vec<YouTubeItem>) {
        clear_list_box(&self.results);
        self.heading.set_text(title);
        self.spinner.stop();
        for item in &items {
            self.results.append(&youtube_row(item));
        }
        if items.is_empty() {
            self.results
                .append(&empty_row("Nenhum resultado encontrado"));
        }
        self.items.replace(items);
    }

    pub fn show_error(&self, message: &str) {
        self.spinner.stop();
        self.heading.set_text("Erro no YouTube Music");
        clear_list_box(&self.results);
        self.results.append(&empty_row(message));
        self.items.borrow_mut().clear();
    }

    pub fn show_empty(&self, message: &str) {
        clear_list_box(&self.results);
        self.results.append(&empty_row(message));
        self.items.borrow_mut().clear();
    }

    fn emit_search(&self) {
        let query = self.search_entry.text().trim().to_string();
        if query.is_empty() {
            return;
        }
        let filters = ["songs", "all", "videos", "albums", "artists", "playlists"];
        let filter = filters
            .get(self.filter.selected() as usize)
            .copied()
            .unwrap_or("songs")
            .to_string();
        let _ = self
            .event_tx
            .send(YouTubePageEvent::Search { query, filter });
    }
}

fn youtube_row(item: &YouTubeItem) -> gtk::ListBoxRow {
    let icon_name = match item.result_type.as_str() {
        "playlist" => "view-list-symbolic",
        "album" => "media-optical-symbolic",
        "artist" => "avatar-default-symbolic",
        "video" => "video-x-generic-symbolic",
        _ => "audio-x-generic-symbolic",
    };
    let icon = gtk::Image::from_icon_name(icon_name);
    icon.set_pixel_size(34);
    icon.add_css_class("youtube-result-icon");

    let title = gtk::Label::new(Some(&item.title));
    title.set_xalign(0.0);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title.add_css_class("heading");
    let subtitle = gtk::Label::new(Some(&item.subtitle));
    subtitle.set_xalign(0.0);
    subtitle.set_ellipsize(gtk::pango::EllipsizeMode::End);
    subtitle.add_css_class("dim-label");
    let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
    text.set_hexpand(true);
    text.append(&title);
    text.append(&subtitle);

    let action = gtk::Image::from_icon_name(if item.playable() {
        "media-playback-start-symbolic"
    } else {
        "go-next-symbolic"
    });
    action.set_opacity(0.72);

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    content.set_margin_top(9);
    content.set_margin_bottom(9);
    content.set_margin_start(12);
    content.set_margin_end(12);
    content.append(&icon);
    content.append(&text);
    content.append(&action);

    let row = gtk::ListBoxRow::new();
    row.set_activatable(item.playable() || item.result_type == "playlist");
    row.set_child(Some(&content));
    row
}

fn empty_row(message: &str) -> gtk::ListBoxRow {
    let label = gtk::Label::new(Some(message));
    label.set_wrap(true);
    label.set_justify(gtk::Justification::Center);
    label.set_margin_top(30);
    label.set_margin_bottom(30);
    label.set_margin_start(16);
    label.set_margin_end(16);
    label.add_css_class("dim-label");
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    row.set_child(Some(&label));
    row
}

fn clear_list_box(list: &gtk::ListBox) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
}

pub fn cache_items_for_browser(items: &mut [YouTubeItem]) {
    for item in items {
        item.thumbnail_url = upgrade_thumbnail_url(&item.thumbnail_url, PLAYER_COVER_SIZE);
        if item.cover_path.is_empty() {
            if let Some(path) = download_cover_sized(item, &item.thumbnail_url, BROWSER_COVER_SIZE)
            {
                item.cover_path = path.to_string_lossy().to_string();
            }
        }
    }
}

pub fn cache_library_covers(snapshot: &mut YouTubeLibrarySnapshot) {
    let mut albums = HashSet::new();
    let mut artists = HashSet::new();

    for item in snapshot.library.iter_mut().chain(snapshot.liked.iter_mut()) {
        item.thumbnail_url = upgrade_thumbnail_url(&item.thumbnail_url, PLAYER_COVER_SIZE);
        let album_key = item.album.trim().to_lowercase();
        let artist_key = item.artist.trim().to_lowercase();
        let needs_album = !album_key.is_empty() && albums.insert(album_key);
        let needs_artist = !artist_key.is_empty() && artists.insert(artist_key);
        if (needs_album || needs_artist) && item.cover_path.is_empty() {
            if let Some(path) = download_cover_sized(item, &item.thumbnail_url, BROWSER_COVER_SIZE)
            {
                item.cover_path = path.to_string_lossy().to_string();
            }
        }
    }

    cache_items_for_browser(&mut snapshot.playlists);
    cache_items_for_browser(&mut snapshot.suggested_albums);
    cache_items_for_browser(&mut snapshot.suggested_artists);
}

fn youtube_catalog(library: &[YouTubeItem], liked: &[YouTubeItem]) -> Vec<YouTubeItem> {
    let mut seen = HashSet::new();
    library
        .iter()
        .chain(liked.iter())
        .filter(|item| item.playable())
        .filter(|item| seen.insert(item.video_id.clone()))
        .cloned()
        .collect()
}

fn build_album_cache(catalog: &[YouTubeItem]) -> Vec<YouTubeCollectionEntry> {
    let mut groups: BTreeMap<String, Vec<&YouTubeItem>> = BTreeMap::new();
    for item in catalog {
        let album = item.album.trim();
        if !album.is_empty() {
            groups.entry(album.to_string()).or_default().push(item);
        }
    }

    groups
        .into_iter()
        .map(|(album, items)| {
            let artists = items
                .iter()
                .map(|item| item.artist.as_str())
                .filter(|artist| !artist.is_empty())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
                .join(", ");
            let cover_path = items
                .iter()
                .find_map(|item| item.cached_cover())
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_default();
            YouTubeCollectionEntry {
                title: album,
                subtitle: artists,
                detail: format!("YouTube Music • {} faixas", items.len()),
                cover_path,
                item_count: items.len(),
            }
        })
        .collect()
}

fn merge_suggested_collections(
    collections: &mut Vec<YouTubeCollectionEntry>,
    suggestions: &[YouTubeItem],
    kind: &str,
) {
    let mut seen = collections
        .iter()
        .map(|entry| entry.title.trim().to_lowercase())
        .collect::<HashSet<_>>();
    let mut suggested = Vec::new();
    for item in suggestions {
        let title = item.title.trim();
        if title.is_empty() || !seen.insert(title.to_lowercase()) {
            continue;
        }
        suggested.push(YouTubeCollectionEntry {
            title: title.to_string(),
            subtitle: item.subtitle.clone(),
            detail: match kind {
                "artist" => "YouTube Music • sugerido".to_string(),
                _ => "YouTube Music • álbum sugerido".to_string(),
            },
            cover_path: item
                .cached_cover()
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_default(),
            item_count: 0,
        });
    }
    suggested.extend(collections.iter().cloned());
    *collections = suggested;
}

fn extend_unique_youtube_items(target: &mut Vec<YouTubeItem>, items: Vec<YouTubeItem>) {
    let mut seen = target
        .iter()
        .map(|item| {
            (
                item.result_type.clone(),
                if item.browse_id.is_empty() {
                    item.video_id.clone()
                } else {
                    item.browse_id.clone()
                },
                item.title.clone(),
            )
        })
        .collect::<HashSet<_>>();
    for item in items {
        let key = (
            item.result_type.clone(),
            if item.browse_id.is_empty() {
                item.video_id.clone()
            } else {
                item.browse_id.clone()
            },
            item.title.clone(),
        );
        if seen.insert(key) {
            target.push(item);
        }
    }
}

fn build_artist_cache(catalog: &[YouTubeItem]) -> Vec<YouTubeCollectionEntry> {
    let mut groups: BTreeMap<String, Vec<&YouTubeItem>> = BTreeMap::new();
    for item in catalog {
        let artist = item.artist.trim();
        if !artist.is_empty() {
            groups.entry(artist.to_string()).or_default().push(item);
        }
    }

    groups
        .into_iter()
        .map(|(artist, items)| {
            let albums = items
                .iter()
                .map(|item| item.album.as_str())
                .filter(|album| !album.is_empty())
                .collect::<BTreeSet<_>>()
                .len();
            let cover_path = items
                .iter()
                .find_map(|item| item.cached_cover())
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_default();
            YouTubeCollectionEntry {
                title: artist,
                subtitle: format!("{albums} álbuns"),
                detail: format!("YouTube Music • {} faixas", items.len()),
                cover_path,
                item_count: items.len(),
            }
        })
        .collect()
}

pub fn download_cover(item: &YouTubeItem, url: &str) -> Option<PathBuf> {
    download_cover_sized(item, url, PLAYER_COVER_SIZE)
}

fn download_cover_sized(item: &YouTubeItem, url: &str, size: u32) -> Option<PathBuf> {
    let original = if url.is_empty() {
        item.thumbnail_url.as_str()
    } else {
        url
    };
    if original.is_empty() {
        return None;
    }

    let upgraded = upgrade_thumbnail_url(original, size);
    let cache_root = glib::user_cache_dir()
        .join("nocky")
        .join("youtube")
        .join("covers");
    fs::create_dir_all(&cache_root).ok()?;
    let digest = stable_hash(&upgraded);
    let destination = cache_root.join(format!("{digest:016x}-{size}.cover"));
    if destination.is_file() && fs::metadata(&destination).ok()?.len() > 0 {
        return Some(destination);
    }

    let client = cover_client()?;

    let bytes = fetch_cover_bytes(client, &upgraded).or_else(|| {
        (upgraded != original)
            .then(|| fetch_cover_bytes(client, original))
            .flatten()
    })?;
    let temporary = destination.with_extension("tmp");
    fs::write(&temporary, &bytes).ok()?;
    fs::rename(&temporary, &destination).ok()?;
    Some(destination)
}

fn cover_client() -> Option<&'static reqwest::blocking::Client> {
    COVER_CLIENT
        .get_or_init(|| {
            reqwest::blocking::Client::builder()
                .user_agent("Nocky/0.2.4")
                .timeout(std::time::Duration::from_secs(20))
                .build()
                .ok()
        })
        .as_ref()
}

fn fetch_cover_bytes(client: &reqwest::blocking::Client, url: &str) -> Option<Vec<u8>> {
    let response = client.get(url).send().ok()?;
    if !response.status().is_success() {
        return None;
    }
    let bytes = response.bytes().ok()?;
    (!bytes.is_empty()).then(|| bytes.to_vec())
}

pub fn upgrade_thumbnail_url(url: &str, size: u32) -> String {
    let url = url.trim();
    if url.is_empty() {
        return String::new();
    }

    let mut output = url.to_string();
    let path_end = output
        .find(|character| character == '?' || character == '#')
        .unwrap_or(output.len());
    let slash = output[..path_end].rfind('/').unwrap_or(0);
    if let Some(relative_equal) = output[slash..path_end].rfind('=') {
        let equal = slash + relative_equal;
        let suffix = &output[equal + 1..path_end];
        let replacement = if let Some(rest) = suffix.strip_prefix('s') {
            let digits = rest
                .chars()
                .take_while(|character| character.is_ascii_digit())
                .count();
            (digits > 0).then(|| format!("s{}{}", size, &rest[digits..]))
        } else if let Some(rest) = suffix.strip_prefix('w') {
            let width_digits = rest
                .chars()
                .take_while(|character| character.is_ascii_digit())
                .count();
            let after_width = &rest[width_digits..];
            if width_digits > 0 {
                if let Some(height) = after_width.strip_prefix("-h") {
                    let height_digits = height
                        .chars()
                        .take_while(|character| character.is_ascii_digit())
                        .count();
                    (height_digits > 0)
                        .then(|| format!("w{0}-h{0}{1}", size, &height[height_digits..]))
                } else {
                    Some(format!("w{}{}", size, after_width))
                }
            } else {
                None
            }
        } else {
            None
        };

        if let Some(replacement) = replacement {
            output.replace_range(equal + 1..path_end, &replacement);
            return output;
        }
    }

    if output.contains("googleusercontent.com") {
        output.insert_str(path_end, &format!("=s{size}"));
    }
    output
}

pub fn load_library_cache() -> YouTubeLibraryCache {
    let path = youtube_library_cache_path();
    let Ok(raw) = fs::read_to_string(path) else {
        return YouTubeLibraryCache::default();
    };
    let Ok(cache) = serde_json::from_str::<PersistedYouTubeLibraryCache>(&raw) else {
        return YouTubeLibraryCache::default();
    };
    if cache.version != LIBRARY_CACHE_VERSION {
        return YouTubeLibraryCache::default();
    }

    let cacheable_playlists = cache
        .playlists
        .iter()
        .filter(|item| cacheable_youtube_playlist(item))
        .map(|item| item.browse_id.clone())
        .collect::<HashSet<_>>();
    let playlist_tracks = cache
        .playlist_tracks
        .into_iter()
        .filter(|(browse_id, items)| cacheable_playlists.contains(browse_id) && !items.is_empty())
        .collect();
    let mut library = YouTubeLibraryCache {
        connected: false,
        syncing: false,
        // Keep this false so a connected account refreshes silently in the background.
        synced: false,
        library: cache.library,
        liked: cache.liked,
        playlists: cache.playlists,
        suggested_albums: cache.suggested_albums,
        suggested_artists: cache.suggested_artists,
        playlist_tracks,
        albums: cache.albums,
        artists: cache.artists,
    };
    if library.albums.is_empty() || library.artists.is_empty() {
        library.rebuild_collections();
    }
    library
}

pub fn save_library_cache(cache: &YouTubeLibraryCache) -> Result<(), String> {
    let path = youtube_library_cache_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create the YouTube cache folder: {error}"))?;
    }
    let saved_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    let cacheable_playlists = cache
        .playlists
        .iter()
        .filter(|item| cacheable_youtube_playlist(item))
        .map(|item| item.browse_id.clone())
        .collect::<HashSet<_>>();
    let playlist_tracks = cache
        .playlist_tracks
        .iter()
        .filter(|(browse_id, items)| cacheable_playlists.contains(*browse_id) && !items.is_empty())
        .map(|(browse_id, items)| (browse_id.clone(), items.clone()))
        .collect();
    let payload = PersistedYouTubeLibraryCache {
        version: LIBRARY_CACHE_VERSION,
        saved_at,
        library: cache.library.clone(),
        liked: cache.liked.clone(),
        playlists: cache.playlists.clone(),
        suggested_albums: cache.suggested_albums.clone(),
        suggested_artists: cache.suggested_artists.clone(),
        playlist_tracks,
        albums: cache.albums.clone(),
        artists: cache.artists.clone(),
    };
    let serialized = serde_json::to_vec(&payload)
        .map_err(|error| format!("Could not serialize the YouTube library cache: {error}"))?;
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, serialized)
        .map_err(|error| format!("Could not write the YouTube library cache: {error}"))?;
    fs::rename(&temporary, &path)
        .map_err(|error| format!("Could not replace the YouTube library cache: {error}"))?;
    Ok(())
}

pub fn clear_library_cache() {
    let _ = fs::remove_file(youtube_library_cache_path());
}

fn youtube_library_cache_path() -> PathBuf {
    glib::user_cache_dir()
        .join("nocky")
        .join("youtube")
        .join("library-cache.json")
}

fn stable_hash(value: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn helper_path() -> Option<PathBuf> {
    if let Some(path) = env::var_os("NOCKY_YOUTUBE_HELPER").map(PathBuf::from) {
        if path.is_file() {
            return Some(path);
        }
    }

    let mut candidates = Vec::new();
    if let Ok(executable) = env::current_exe() {
        if let Some(prefix) = executable.parent().and_then(Path::parent) {
            candidates.push(prefix.join("share/nocky/helpers/nocky_youtube.py"));
        }
    }
    candidates.push(
        glib::user_data_dir()
            .join("nocky")
            .join("helpers")
            .join("nocky_youtube.py"),
    );
    candidates.push(PathBuf::from(
        "/usr/local/share/nocky/helpers/nocky_youtube.py",
    ));
    candidates.push(PathBuf::from("/usr/share/nocky/helpers/nocky_youtube.py"));
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("helpers/nocky_youtube.py"));
    candidates.into_iter().find(|path| path.is_file())
}

fn python_path() -> Option<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(path) = env::var_os("NOCKY_PYTHON").map(PathBuf::from) {
        candidates.push(path);
    }
    if let Ok(executable) = env::current_exe() {
        if let Some(prefix) = executable.parent().and_then(Path::parent) {
            candidates.push(prefix.join("share/nocky/runtime/bin/python3"));
            candidates.push(prefix.join("share/nocky/runtime/bin/python"));
        }
    }

    let user_runtime = glib::user_data_dir()
        .join("nocky")
        .join("runtime")
        .join("bin");
    candidates.push(user_runtime.join("python3"));
    candidates.push(user_runtime.join("python"));
    candidates.push(PathBuf::from("/usr/local/share/nocky/runtime/bin/python3"));
    candidates.push(PathBuf::from("/usr/local/share/nocky/runtime/bin/python"));
    candidates.push(PathBuf::from("/usr/share/nocky/runtime/bin/python3"));
    candidates.push(PathBuf::from("/usr/share/nocky/runtime/bin/python"));

    let development_runtime = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".nocky-runtime/bin");
    candidates.push(development_runtime.join("python3"));
    candidates.push(development_runtime.join("python"));

    if let Some(system_python) = find_in_path("python3") {
        candidates.push(system_python);
    }

    candidates
        .into_iter()
        .filter(|path| path.is_file())
        .find(|path| python_supports_youtube(path))
}

fn python_supports_youtube(path: &Path) -> bool {
    Command::new(path)
        .args(["-c", "import requests, ytmusicapi, yt_dlp"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn find_in_path(name: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
}

// rich_youtube_mix_page_header_v1
// rich_youtube_mix_rows_v1
// vertical_collection_edge_scroll_spring_v5_home_timing
// vertical_collection_edge_scroll_spring_v4
// vertical_collection_edge_scroll_spring_v3
// vertical_collection_edge_spring_allocation_safe_v2
// vertical_collection_edge_spring_v1
// collection_card_entry_spring_v1
// remove_collection_now_playing_badge_v1
// richer_collection_cards_phase1_v1
// collection_context_favorites_and_placeholders_v6
// shared_collection_card_descriptor_v1
// hide_youtube_home_sections_in_local_mode_v1
// recently_added_local_music_v1
// restore_circular_artist_covers_v2
// preserve_home_carousel_scroll_v1
// collection_card_inline_loading_fix_v2
// collection_card_loading_spinner_v3\n// collection_overflow_icon_label_fix_v2
// collection_card_overflow_and_play_state_v2
// contextual_collection_controls_v5
// smooth_home_crossfade_v1
// recent_album_cover_resolution_v1
// recent_activity_exact_fix_v1
// merge_recent_activity_sources_v1
// personalized_home_resume_v2
use crate::{
    config::{AppConfig, AppLanguage, StartupSource, VisualTheme},
    listening_history::{HistoryActivity, ListeningHistory, ListeningSource, ListeningStats},
    model::Track,
    youtube::{youtube_collection_key, YouTubeCollectionEntry, YouTubeItem, YouTubeLibraryCache},
};
use gtk::{gdk, gio::prelude::ListModelExt, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque},
    fs,
    path::{Path, PathBuf},
    rc::Rc,
    sync::mpsc::{self, Receiver, Sender},
    time::{Duration, UNIX_EPOCH},
};

const ARTWORK_TEXTURE_CACHE_LIMIT: usize = 160;
const SEARCH_BATCH_SIZE: usize = 5;

#[derive(Default)]
struct ArtworkTextureCache {
    entries: HashMap<(PathBuf, i32, u64), CachedArtworkTexture>,
    clock: u64,
}

struct CachedArtworkTexture {
    texture: gdk::Texture,
    last_used: u64,
}

thread_local! {
    static ARTWORK_TEXTURES: RefCell<ArtworkTextureCache> =
        RefCell::new(ArtworkTextureCache::default());
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum BrowserRoute {
    #[default]
    All,
    Albums,
    Artists,
    Playlists,
    Liked,
    Album(String),
    Artist(String),
    Playlist(String),
    YouTubeAlbum(String),
    YouTubeArtist(String),
    YouTubePlaylist {
        title: String,
        browse_id: String,
    },
}

#[derive(Clone, Debug)]
pub enum BrowserEvent {
    TrackActivated(usize),
    ResumeLocalTrack {
        index: usize,
        position_seconds: u64,
    },
    ResumeYouTubeTrack {
        item: YouTubeItem,
        position_seconds: u64,
    },
    YouTubeTrackActivated {
        item: YouTubeItem,
        queue: Vec<YouTubeItem>,
        index: usize,
    },
    QueueLocalPlayNext(usize),
    QueueLocalAppend(usize),
    QueueYouTubePlayNext(YouTubeItem),
    QueueYouTubeAppend(YouTubeItem),
    ToggleLocalTrackFavorite(usize),
    ToggleYouTubeTrackFavorite(YouTubeItem),
    QueueLocalCollection {
        kind: String,
        title: String,
        play_next: bool,
    },
    QueueYouTubeCollection {
        item: YouTubeItem,
        playlist: bool,
        play_next: bool,
    },
    TogglePlayback,
    PlayLocalAlbum(String),
    PlayLocalPlaylist(String),
    PlayYouTubeAlbum(YouTubeItem),
    PlayYouTubePlaylist(YouTubeItem),
    OpenYouTubePlaylist(YouTubeItem),
    OpenYouTubeCollection(YouTubeItem),
    LoadMoreAlbums,
    LoadMoreArtists,
    RefreshSearch,
    Navigate(BrowserRoute),
    CreatePlaylist(String),
    AddCurrentToPlaylist(String),
    RemoveCurrentFromPlaylist(String),
    DeletePlaylist(String),
    ToggleCollectionFavorite(String),
}

#[derive(Clone, Debug, Default)]
pub struct BrowserPlaybackState {
    pub playing: bool,
    pub collection_kind: String,
    pub collection_id: String,
    pub collection_title: String,
    pub loading_collections: HashSet<String>,
}

impl BrowserPlaybackState {
    fn matches_collection(&self, kind: &str, id: &str, title: &str) -> bool {
        if !self.collection_kind.eq_ignore_ascii_case(kind) {
            return false;
        }

        let stored_id = self.collection_id.trim();
        let stored_title = self.collection_title.trim();
        let normalized_title = title.trim().to_lowercase();

        (!id.trim().is_empty() && stored_id.eq_ignore_ascii_case(id.trim()))
            || (!normalized_title.is_empty() && stored_id.eq_ignore_ascii_case(&normalized_title))
            || (!title.trim().is_empty() && stored_title.eq_ignore_ascii_case(title.trim()))
    }

    fn collection_is_loading(&self, kind: &str, id: &str, title: &str) -> bool {
        let normalized_id = id.trim().to_lowercase();
        let normalized_title = title.trim().to_lowercase();
        let typed_title = format!("{}:{normalized_title}", kind.trim().to_lowercase());

        (!normalized_id.is_empty() && self.loading_collections.contains(&normalized_id))
            || (!normalized_title.is_empty()
                && self.loading_collections.contains(&normalized_title))
            || (!normalized_title.is_empty() && self.loading_collections.contains(&typed_title))
    }
}

pub struct BrowserRenderContext<'a> {
    pub history: &'a ListeningHistory,
    pub playback: &'a BrowserPlaybackState,
}

#[derive(Clone, Debug)]
enum VisibleTrack {
    Local(usize),
    YouTube(Box<YouTubeItem>),
}

#[derive(Clone, Debug)]
enum HomeHistoryTrack {
    LocalTrack {
        index: usize,
        track: Track,
        position_seconds: u64,
        duration_seconds: u64,
        completed: bool,
    },
    YouTubeTrack {
        item: YouTubeItem,
        position_seconds: u64,
        duration_seconds: u64,
        completed: bool,
    },
    LocalAlbum(String),
    LocalPlaylist(String),
    YouTubeAlbum {
        item: YouTubeItem,
        cover_path: Option<PathBuf>,
    },
    YouTubePlaylist(YouTubeItem),
}

#[derive(Clone, Debug)]
enum PlaylistRef {
    Local(String),
    YouTube(Box<YouTubeItem>),
}

#[derive(Clone, Debug)]
enum HomeCard {
    LocalAlbum {
        title: String,
        subtitle: String,
        detail: String,
        cover_path: Option<std::path::PathBuf>,
    },
    YouTubeAlbum {
        item: YouTubeItem,
        subtitle: String,
        detail: String,
        cover_path: Option<PathBuf>,
    },
    LocalArtist {
        title: String,
        subtitle: String,
        detail: String,
        cover_path: Option<PathBuf>,
    },
    YouTubeArtist {
        item: YouTubeItem,
        subtitle: String,
        detail: String,
        cover_path: Option<std::path::PathBuf>,
    },
    LocalPlaylist {
        title: String,
        subtitle: String,
    },
    YouTubePlaylist(YouTubeItem),
}

struct CollectionCardDescriptor<'a> {
    cover_path: Option<&'a Path>,
    title: &'a str,
    subtitle: &'a str,
    detail: &'a str,
    online: bool,
    artist: bool,
    placeholder_icon: &'static str,
    placeholder_class: &'static str,
}

impl HomeCard {
    fn descriptor(&self, language: AppLanguage) -> CollectionCardDescriptor<'_> {
        match self {
            Self::LocalAlbum {
                title,
                subtitle,
                detail,
                cover_path,
            } => CollectionCardDescriptor {
                cover_path: cover_path.as_deref(),
                title,
                subtitle,
                detail,
                online: false,
                artist: false,
                placeholder_icon: "media-optical-symbolic",
                placeholder_class: "album-placeholder",
            },
            Self::YouTubeAlbum {
                item,
                subtitle,
                detail,
                cover_path,
            } => CollectionCardDescriptor {
                cover_path: cover_path.as_deref(),
                title: &item.title,
                subtitle,
                detail,
                online: true,
                artist: false,
                placeholder_icon: "media-optical-symbolic",
                placeholder_class: "album-placeholder",
            },
            Self::LocalArtist {
                title,
                subtitle,
                detail,
                cover_path,
            } => CollectionCardDescriptor {
                cover_path: cover_path.as_deref(),
                title,
                subtitle,
                detail,
                online: false,
                artist: true,
                placeholder_icon: "avatar-default-symbolic",
                placeholder_class: "artist-placeholder",
            },
            Self::YouTubeArtist {
                item,
                subtitle,
                detail,
                cover_path,
            } => CollectionCardDescriptor {
                cover_path: cover_path.as_deref(),
                title: &item.title,
                subtitle,
                detail,
                online: true,
                artist: true,
                placeholder_icon: "avatar-default-symbolic",
                placeholder_class: "artist-placeholder",
            },
            Self::LocalPlaylist { title, subtitle } => CollectionCardDescriptor {
                cover_path: None,
                title,
                subtitle,
                detail: home_copy(language).local_playlist,
                online: false,
                artist: false,
                placeholder_icon: "view-list-symbolic",
                placeholder_class: "playlist-placeholder",
            },
            Self::YouTubePlaylist(item) => CollectionCardDescriptor {
                cover_path: item.cached_cover(),
                title: &item.title,
                subtitle: home_youtube_playlist_subtitle(item, language),
                detail: home_youtube_playlist_detail(item, language),
                online: true,
                artist: false,
                placeholder_icon: "view-list-symbolic",
                placeholder_class: "playlist-placeholder",
            },
        }
    }

    fn open_event(&self) -> BrowserEvent {
        match self {
            Self::LocalAlbum { title, .. } => {
                BrowserEvent::Navigate(BrowserRoute::Album(title.clone()))
            }
            Self::YouTubeAlbum { item, .. } => BrowserEvent::OpenYouTubeCollection(item.clone()),
            Self::LocalArtist { title, .. } => {
                BrowserEvent::Navigate(BrowserRoute::Artist(title.clone()))
            }
            Self::YouTubeArtist { item, .. } => BrowserEvent::OpenYouTubeCollection(item.clone()),
            Self::LocalPlaylist { title, .. } => {
                BrowserEvent::Navigate(BrowserRoute::Playlist(title.clone()))
            }
            Self::YouTubePlaylist(item) => BrowserEvent::OpenYouTubePlaylist(item.clone()),
        }
    }

    fn identity(&self) -> String {
        match self {
            Self::LocalAlbum { title, .. } => {
                format!("local-album:{}", title.to_lowercase())
            }
            Self::YouTubeAlbum { item, .. } => {
                format!("youtube-album:{}", item.title.to_lowercase())
            }
            Self::LocalArtist { title, .. } => {
                format!("local-artist:{}", title.to_lowercase())
            }
            Self::YouTubeArtist { item, .. } => {
                format!("youtube-artist:{}", item.title.to_lowercase())
            }
            Self::LocalPlaylist { title, .. } => {
                format!("local-playlist:{}", title.to_lowercase())
            }
            Self::YouTubePlaylist(item) => {
                format!("youtube-playlist:{}", item.title.to_lowercase())
            }
        }
    }
}

// complete_surface_localization_v3
#[derive(Clone, Copy)]
struct HomeCopy {
    recent_activity_title: &'static str,
    recent_activity_subtitle: &'static str,
    recently_added_title: &'static str,
    recently_added_subtitle: &'static str,
    recently_added_detail: &'static str,
    mixtapes_title: &'static str,
    mixtapes_subtitle: &'static str,
    albums_title: &'static str,
    albums_subtitle: &'static str,
    artists_title: &'static str,
    artists_subtitle: &'static str,
    playlists_title: &'static str,
    playlists_subtitle: &'static str,
    waiting_content: &'static str,
    empty_hint: &'static str,
    syncing_library: &'static str,
    local_playlist: &'static str,
    youtube_mix: &'static str,
    youtube_recommendation: &'static str,
    synchronized_playlist: &'static str,
}

fn home_copy(language: AppLanguage) -> HomeCopy {
    match language {
        AppLanguage::Portuguese => HomeCopy {
            recent_activity_title: "Ouvidos recentemente",
            recent_activity_subtitle: "Faixas, álbuns e playlists em ordem cronológica",
            recently_added_title: "Adicionados recentemente",
            recently_added_subtitle: "Álbuns locais mais novos na sua biblioteca",
            recently_added_detail: "Adicionado recentemente",
            mixtapes_title: "Mixtapes criadas para você",
            mixtapes_subtitle: "Mixes e rádios sincronizadas do YouTube Music",
            albums_title: "Seus álbuns",
            albums_subtitle: "Mais ouvidos e reproduzidos recentemente",
            artists_title: "Seus artistas",
            artists_subtitle: "Com base no que você mais escuta",
            playlists_title: "Playlists sugeridas",
            playlists_subtitle: "Playlists e recomendações sincronizadas",
            waiting_content: "Aguardando conteúdo sincronizado",
            empty_hint: "Sincronize o YouTube Music ou escolha uma pasta local",
            syncing_library: "Sincronizando sua biblioteca do YouTube Music...",
            local_playlist: "Playlist local",
            youtube_mix: "Mix gerado pelo YouTube Music",
            youtube_recommendation: "Recomendação do YouTube Music",
            synchronized_playlist: "Playlist sincronizada",
        },
        AppLanguage::English => HomeCopy {
            recent_activity_title: "Recently listened",
            recent_activity_subtitle: "Tracks, albums and playlists in chronological order",
            recently_added_title: "Recently added",
            recently_added_subtitle: "Newest local albums in your library",
            recently_added_detail: "Recently added",
            mixtapes_title: "Mixtapes made for you",
            mixtapes_subtitle: "Mixes and radio stations synchronized from YouTube Music",
            albums_title: "Your albums",
            albums_subtitle: "Most played and recently listened to",
            artists_title: "Your artists",
            artists_subtitle: "Based on what you listen to most",
            playlists_title: "Suggested playlists",
            playlists_subtitle: "Synchronized playlists and recommendations",
            waiting_content: "Waiting for synchronized content",
            empty_hint: "Synchronize YouTube Music or choose a local folder",
            syncing_library: "Synchronizing your YouTube Music library...",
            local_playlist: "Local playlist",
            youtube_mix: "Mix generated by YouTube Music",
            youtube_recommendation: "YouTube Music recommendation",
            synchronized_playlist: "Synchronized playlist",
        },
        AppLanguage::Spanish => HomeCopy {
            recent_activity_title: "Escuchados recientemente",
            recent_activity_subtitle: "Canciones, álbumes y playlists en orden cronológico",
            recently_added_title: "Añadidos recientemente",
            recently_added_subtitle: "Álbumes locales más nuevos de tu biblioteca",
            recently_added_detail: "Añadido recientemente",
            mixtapes_title: "Mixtapes creadas para ti",
            mixtapes_subtitle: "Mixes y radios sincronizadas de YouTube Music",
            albums_title: "Tus álbumes",
            albums_subtitle: "Más escuchados y reproducidos recientemente",
            artists_title: "Tus artistas",
            artists_subtitle: "Según lo que más escuchas",
            playlists_title: "Playlists sugeridas",
            playlists_subtitle: "Playlists y recomendaciones sincronizadas",
            waiting_content: "Esperando contenido sincronizado",
            empty_hint: "Sincroniza YouTube Music o elige una carpeta local",
            syncing_library: "Sincronizando tu biblioteca de YouTube Music...",
            local_playlist: "Playlist local",
            youtube_mix: "Mix generado por YouTube Music",
            youtube_recommendation: "Recomendación de YouTube Music",
            synchronized_playlist: "Playlist sincronizada",
        },
    }
}

fn recently_added_local_album_cards(
    tracks: &[Track],
    language: AppLanguage,
    detail_label: &str,
) -> Vec<HomeCard> {
    let mut groups: HashMap<String, Vec<&Track>> = HashMap::new();
    for track in tracks {
        let album = track.album.trim();
        if album.is_empty() {
            continue;
        }
        groups.entry(album.to_string()).or_default().push(track);
    }

    let mut albums = groups
        .into_iter()
        .map(|(album, album_tracks)| {
            let newest_timestamp = album_tracks
                .iter()
                .filter_map(|track| local_file_timestamp(&track.path))
                .max()
                .unwrap_or_default();

            (newest_timestamp, album, album_tracks)
        })
        .collect::<Vec<_>>();

    albums.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| compare_text(&left.1, &right.1))
    });

    albums
        .into_iter()
        .take(12)
        .map(|(_, album, album_tracks)| {
            let artists = album_tracks
                .iter()
                .map(|track| track.artist.trim())
                .filter(|artist| !artist.is_empty())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
                .join(", ");
            let cover_path = album_tracks
                .iter()
                .find_map(|track| track.cover_path.clone());
            let track_count = format_track_count(language, album_tracks.len());

            HomeCard::LocalAlbum {
                title: album,
                subtitle: artists,
                detail: format!("{detail_label} • {track_count}"),
                cover_path,
            }
        })
        .collect()
}

fn local_file_timestamp(path: &Path) -> Option<u64> {
    let metadata = fs::metadata(path).ok()?;
    let timestamp = metadata.created().or_else(|_| metadata.modified()).ok()?;
    timestamp
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
}

fn format_track_count(language: AppLanguage, count: usize) -> String {
    match language {
        AppLanguage::Portuguese => {
            format!("{count} {}", if count == 1 { "faixa" } else { "faixas" })
        }
        AppLanguage::English => {
            format!("{count} {}", if count == 1 { "track" } else { "tracks" })
        }
        AppLanguage::Spanish => {
            format!("{count} {}", if count == 1 { "pista" } else { "pistas" })
        }
    }
}

fn format_local_track_count(language: AppLanguage, count: usize) -> String {
    match language {
        AppLanguage::Portuguese => {
            format!(
                "{count} {}",
                if count == 1 {
                    "faixa local"
                } else {
                    "faixas locais"
                }
            )
        }
        AppLanguage::English => {
            format!(
                "{count} {}",
                if count == 1 {
                    "local track"
                } else {
                    "local tracks"
                }
            )
        }
        AppLanguage::Spanish => {
            format!(
                "{count} {}",
                if count == 1 {
                    "pista local"
                } else {
                    "pistas locales"
                }
            )
        }
    }
}

fn format_album_count(language: AppLanguage, count: usize) -> String {
    match language {
        AppLanguage::Portuguese => {
            format!("{count} {}", if count == 1 { "álbum" } else { "álbuns" })
        }
        AppLanguage::English => {
            format!("{count} {}", if count == 1 { "album" } else { "albums" })
        }
        AppLanguage::Spanish => {
            format!("{count} {}", if count == 1 { "álbum" } else { "álbumes" })
        }
    }
}

fn localized_listening_detail(language: AppLanguage, play_count: u64, minutes: u64) -> String {
    match language {
        AppLanguage::Portuguese => {
            let plays = if play_count == 1 {
                "1 reprodução".to_string()
            } else {
                format!("{play_count} reproduções")
            };
            format!("{plays} • {minutes} min ouvidos")
        }
        AppLanguage::English => {
            let plays = if play_count == 1 {
                "1 play".to_string()
            } else {
                format!("{play_count} plays")
            };
            format!("{plays} • {minutes} min listened")
        }
        AppLanguage::Spanish => {
            let plays = if play_count == 1 {
                "1 reproducción".to_string()
            } else {
                format!("{play_count} reproducciones")
            };
            format!("{plays} • {minutes} min escuchados")
        }
    }
}

fn home_youtube_playlist_detail(item: &YouTubeItem, language: AppLanguage) -> &'static str {
    let text = home_copy(language);
    match item.playlist_kind.as_str() {
        "mix" => text.youtube_mix,
        "recommended" => text.youtube_recommendation,
        _ => "YouTube Music",
    }
}

fn home_youtube_playlist_subtitle(item: &YouTubeItem, language: AppLanguage) -> &str {
    if !item.subtitle.is_empty() {
        return item.subtitle.as_str();
    }

    let text = home_copy(language);
    match item.playlist_kind.as_str() {
        "mix" => text.youtube_mix,
        "recommended" => text.youtube_recommendation,
        _ => text.synchronized_playlist,
    }
}

pub struct LibraryBrowser {
    root: gtk::Stack,
    home_stack: gtk::Stack,
    home_generation: Rc<Cell<u64>>,
    search_content: gtk::Box,
    last_search_query: RefCell<String>,
    search_track_limit: Rc<Cell<usize>>,
    search_album_limit: Rc<Cell<usize>>,
    search_artist_limit: Rc<Cell<usize>>,
    search_playlist_limit: Rc<Cell<usize>>,
    queue: gtk::ListBox,
    queue_title: gtk::Label,
    queue_context_header: gtk::Box,
    albums_grid: gtk::FlowBox,
    artists_grid: gtk::FlowBox,
    playlists_list: gtk::ListBox,
    playlist_model: gtk::StringList,
    playlist_dropdown: gtk::DropDown,
    route: RefCell<BrowserRoute>,
    visible_tracks: Rc<RefCell<Vec<VisibleTrack>>>,
    queue_render_generation: Rc<Cell<u64>>,
    album_display_limit: Cell<usize>,
    artist_display_limit: Cell<usize>,
    playlist_names: Rc<RefCell<Vec<String>>>,
    playlist_row_refs: Rc<RefCell<Vec<Option<PlaylistRef>>>>,
    event_tx: Sender<BrowserEvent>,
    events: Receiver<BrowserEvent>,
}

fn collect_scrolled_windows(widget: &gtk::Widget, output: &mut Vec<gtk::ScrolledWindow>) {
    if let Ok(scrolled) = widget.clone().downcast::<gtk::ScrolledWindow>() {
        output.push(scrolled);
    }

    let mut child = widget.first_child();
    while let Some(current) = child {
        collect_scrolled_windows(&current, output);
        child = current.next_sibling();
    }
}

impl LibraryBrowser {
    pub fn home_scroll_positions(&self) -> Vec<f64> {
        let Some(content) = self.home_stack.visible_child() else {
            return Vec::new();
        };

        let mut scrolled_windows = Vec::new();
        collect_scrolled_windows(&content, &mut scrolled_windows);
        scrolled_windows
            .into_iter()
            .map(|scrolled| scrolled.hadjustment().value())
            .collect()
    }

    pub fn restore_home_scroll_positions(&self, positions: Vec<f64>) {
        if positions.is_empty() {
            return;
        }

        let home_stack = self.home_stack.clone();
        glib::idle_add_local_once(move || {
            let Some(content) = home_stack.visible_child() else {
                return;
            };

            let mut scrolled_windows = Vec::new();
            collect_scrolled_windows(&content, &mut scrolled_windows);

            for (scrolled, value) in scrolled_windows.into_iter().zip(positions) {
                let adjustment = scrolled.hadjustment();
                let maximum = (adjustment.upper() - adjustment.page_size()).max(0.0);
                adjustment.set_value(value.clamp(0.0, maximum));
            }
        });
    }

    pub fn new() -> Self {
        let (event_tx, events) = mpsc::channel();
        let visible_tracks = Rc::new(RefCell::new(Vec::new()));
        let queue_render_generation = Rc::new(Cell::new(0_u64));
        let playlist_names = Rc::new(RefCell::new(Vec::new()));
        let playlist_row_refs = Rc::new(RefCell::new(Vec::new()));

        let home_generation = Rc::new(Cell::new(0_u64));
        let home_stack = gtk::Stack::new();
        home_stack.set_hexpand(true);
        home_stack.set_vexpand(false);
        home_stack.set_transition_type(gtk::StackTransitionType::Crossfade);
        home_stack.set_transition_duration(180);
        home_stack.set_interpolate_size(true);

        let home_content = gtk::Box::new(gtk::Orientation::Vertical, 22);
        home_content.set_hexpand(true);
        home_content.set_vexpand(false);
        // material_expressive_library_cards_v1
        home_content.add_css_class("library-home");
        home_content.add_css_class("expressive-library-home");
        home_stack.add_named(&home_content, Some("home-0"));
        home_stack.set_visible_child_name("home-0");

        let search_content = gtk::Box::new(gtk::Orientation::Vertical, 22);
        search_content.set_hexpand(true);
        search_content.set_vexpand(false);
        search_content.add_css_class("library-home");
        search_content.add_css_class("expressive-search-results");
        search_content.add_css_class("search-results-page");

        let search_scroll = gtk::ScrolledWindow::new();
        search_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        search_scroll.set_vexpand(true);
        search_scroll.set_child(Some(&search_content));

        let search_track_limit = Rc::new(Cell::new(SEARCH_BATCH_SIZE));
        let search_album_limit = Rc::new(Cell::new(SEARCH_BATCH_SIZE));
        let search_artist_limit = Rc::new(Cell::new(SEARCH_BATCH_SIZE));
        let search_playlist_limit = Rc::new(Cell::new(SEARCH_BATCH_SIZE));

        let home_scroll = gtk::ScrolledWindow::new();
        home_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        home_scroll.set_vexpand(true);
        home_scroll.set_child(Some(&home_stack));

        let queue = gtk::ListBox::new();
        queue.set_selection_mode(gtk::SelectionMode::Single);
        queue.add_css_class("queue-list");
        queue.add_css_class("expressive-media-list");

        {
            let tx = event_tx.clone();
            let entries = visible_tracks.clone();
            queue.connect_row_activated(move |_, row| {
                let Some(entry) = entries.borrow().get(row.index() as usize).cloned() else {
                    return;
                };
                match entry {
                    VisibleTrack::Local(index) => {
                        let _ = tx.send(BrowserEvent::TrackActivated(index));
                    }
                    VisibleTrack::YouTube(item) => {
                        let queue = entries
                            .borrow()
                            .iter()
                            .filter_map(|entry| match entry {
                                VisibleTrack::YouTube(item) if item.playable() => {
                                    Some(item.as_ref().clone())
                                }
                                _ => None,
                            })
                            .collect::<Vec<_>>();
                        let index = queue
                            .iter()
                            .position(|candidate| candidate.video_id == item.video_id)
                            .unwrap_or(0);
                        let _ = tx.send(BrowserEvent::YouTubeTrackActivated {
                            item: *item,
                            queue,
                            index,
                        });
                    }
                }
            });
        }

        let queue_title = gtk::Label::new(Some("BIBLIOTECA"));
        queue_title.set_xalign(0.0);
        queue_title.add_css_class("section-title");

        let queue_context_header = gtk::Box::new(gtk::Orientation::Vertical, 0);
        queue_context_header.set_hexpand(true);
        queue_context_header.set_visible(false);
        queue_context_header.add_css_class("collection-context-header");

        let queue_scroll = gtk::ScrolledWindow::new();
        queue_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        queue_scroll.set_vexpand(true);
        queue_scroll.set_child(Some(&queue));
        install_vertical_edge_spring(&queue_scroll);

        let tracks_page = gtk::Box::new(gtk::Orientation::Vertical, 12);
        tracks_page.set_hexpand(true);
        tracks_page.set_vexpand(true);
        tracks_page.add_css_class("library-panel");
        tracks_page.add_css_class("expressive-library-page");
        tracks_page.append(&queue_context_header);
        tracks_page.append(&queue_title);
        tracks_page.append(&queue_scroll);

        let albums_grid = collection_grid();
        let albums_page = collection_page(
            "ÁLBUNS",
            "Sua coleção local e os álbuns salvos no YouTube Music",
            "media-optical-symbolic",
            &albums_grid,
        );

        let artists_grid = artist_list_grid();
        let artists_page = collection_page(
            "ARTISTAS",
            "Selecione um artista para abrir sua discografia",
            "avatar-default-symbolic",
            &artists_grid,
        );

        let playlist_model = gtk::StringList::new(&[]);
        let playlist_dropdown = gtk::DropDown::builder()
            .model(&playlist_model)
            .hexpand(true)
            .build();
        let playlist_entry = gtk::Entry::builder()
            .placeholder_text("Nome da nova playlist local")
            .hexpand(true)
            .build();
        let create_button = gtk::Button::with_label("Criar");
        create_button.add_css_class("suggested-action");
        {
            let tx = event_tx.clone();
            let entry = playlist_entry.clone();
            create_button.connect_clicked(move |_| {
                let name = entry.text().trim().to_string();
                if !name.is_empty() {
                    let _ = tx.send(BrowserEvent::CreatePlaylist(name));
                    entry.set_text("");
                }
            });
        }
        {
            let tx = event_tx.clone();
            let entry = playlist_entry.clone();
            playlist_entry.connect_activate(move |_| {
                let name = entry.text().trim().to_string();
                if !name.is_empty() {
                    let _ = tx.send(BrowserEvent::CreatePlaylist(name));
                    entry.set_text("");
                }
            });
        }

        let create_row = gtk::Box::new(gtk::Orientation::Vertical, 8);
        create_row.set_hexpand(true);
        playlist_entry.set_hexpand(true);
        playlist_entry.add_css_class("playlist-editor-entry");
        playlist_dropdown.add_css_class("playlist-editor-dropdown");
        create_row.add_css_class("playlist-editor-surface");
        create_button.add_css_class("playlist-editor-action");
        create_button.add_css_class("playlist-create-action");
        create_button.set_hexpand(true);
        create_row.append(&playlist_entry);
        create_row.append(&create_button);

        let add_button = gtk::Button::with_label("Adicionar faixa atual");
        let remove_button = gtk::Button::with_label("Remover faixa atual");
        let delete_button = gtk::Button::with_label("Excluir playlist local");
        delete_button.add_css_class("destructive-action");

        for (button, kind) in [
            (&add_button, 0_u8),
            (&remove_button, 1_u8),
            (&delete_button, 2_u8),
        ] {
            let tx = event_tx.clone();
            let dropdown = playlist_dropdown.clone();
            let names = playlist_names.clone();
            button.connect_clicked(move |_| {
                let selected = dropdown.selected() as usize;
                let Some(name) = names.borrow().get(selected).cloned() else {
                    return;
                };
                let event = match kind {
                    0 => BrowserEvent::AddCurrentToPlaylist(name),
                    1 => BrowserEvent::RemoveCurrentFromPlaylist(name),
                    _ => BrowserEvent::DeletePlaylist(name),
                };
                let _ = tx.send(event);
            });
        }

        let playlist_select_row = gtk::Box::new(gtk::Orientation::Vertical, 8);
        playlist_select_row.set_hexpand(true);
        playlist_dropdown.set_hexpand(true);
        delete_button.set_hexpand(true);
        playlist_select_row.add_css_class("playlist-editor-surface");
        delete_button.add_css_class("playlist-editor-action");
        playlist_select_row.append(&playlist_dropdown);
        playlist_select_row.append(&delete_button);

        let action_row = gtk::Box::new(gtk::Orientation::Vertical, 8);
        action_row.set_hexpand(true);
        action_row.add_css_class("playlist-editor-surface");
        add_button.add_css_class("playlist-editor-action");
        remove_button.add_css_class("playlist-editor-action");
        add_button.set_hexpand(true);
        remove_button.set_hexpand(true);
        action_row.append(&add_button);
        action_row.append(&remove_button);

        let playlists_list = gtk::ListBox::new();
        playlists_list.set_selection_mode(gtk::SelectionMode::Single);
        playlists_list.add_css_class("playlist-list");
        playlists_list.add_css_class("expressive-media-list");
        {
            let tx = event_tx.clone();
            let refs = playlist_row_refs.clone();
            playlists_list.connect_row_activated(move |_, row| {
                let Some(reference) = refs.borrow().get(row.index() as usize).cloned().flatten()
                else {
                    return;
                };
                match reference {
                    PlaylistRef::Local(name) => {
                        let _ = tx.send(BrowserEvent::Navigate(BrowserRoute::Playlist(name)));
                    }
                    PlaylistRef::YouTube(item) => {
                        let _ = tx.send(BrowserEvent::OpenYouTubePlaylist(*item));
                    }
                }
            });
        }

        let playlists_scroll = gtk::ScrolledWindow::new();
        playlists_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        playlists_scroll.set_vexpand(true);
        playlists_scroll.set_child(Some(&playlists_list));
        install_vertical_edge_spring(&playlists_scroll);

        let playlists_header = page_header(
            "PLAYLISTS",
            "Playlists locais editáveis e playlists sincronizadas do YouTube Music",
        );
        let playlists_page = gtk::Box::new(gtk::Orientation::Vertical, 14);
        playlists_page.set_hexpand(true);
        playlists_page.set_vexpand(true);
        playlists_page.add_css_class("library-panel");
        playlists_page.append(&playlists_header);
        playlists_page.append(&create_row);
        playlists_page.append(&playlist_select_row);
        playlists_page.append(&action_row);
        playlists_page.append(&playlists_scroll);

        let root = gtk::Stack::new();
        root.set_hexpand(true);
        root.set_vexpand(true);
        root.set_transition_type(gtk::StackTransitionType::Crossfade);
        root.set_transition_duration(180);
        root.add_named(&home_scroll, Some("home"));
        root.add_named(&search_scroll, Some("search"));
        root.add_named(&tracks_page, Some("tracks"));
        root.add_named(&albums_page, Some("albums"));
        root.add_named(&artists_page, Some("artists"));
        root.add_named(&playlists_page, Some("playlists"));
        root.set_visible_child_name("home");

        Self {
            root,
            home_stack,
            home_generation,
            search_content,
            last_search_query: RefCell::new(String::new()),
            search_track_limit,
            search_album_limit,
            search_artist_limit,
            search_playlist_limit,
            queue,
            queue_title,
            queue_context_header,
            albums_grid,
            artists_grid,
            playlists_list,
            playlist_model,
            playlist_dropdown,
            route: RefCell::new(BrowserRoute::All),
            visible_tracks,
            queue_render_generation,
            album_display_limit: Cell::new(COLLECTION_INITIAL_BATCH),
            artist_display_limit: Cell::new(COLLECTION_INITIAL_BATCH),
            playlist_names,
            playlist_row_refs,
            event_tx,
            events,
        }
    }

    pub fn root(&self) -> &gtk::Stack {
        &self.root
    }

    pub fn route(&self) -> BrowserRoute {
        self.route.borrow().clone()
    }

    pub fn navigate(
        &self,
        route: BrowserRoute,
        tracks: &[Track],
        config: &AppConfig,
        youtube: &YouTubeLibraryCache,
        context: &BrowserRenderContext<'_>,
        query: &str,
    ) {
        let previous = self.route();
        self.root
            .set_transition_type(route_transition(&previous, &route));
        self.route.replace(route);
        self.refresh(tracks, config, youtube, context, query);
    }

    pub fn refresh(
        &self,
        tracks: &[Track],
        config: &AppConfig,
        youtube: &YouTubeLibraryCache,
        context: &BrowserRenderContext<'_>,
        query: &str,
    ) {
        if matches!(self.route(), BrowserRoute::All) && !query.trim().is_empty() {
            self.rebuild_search(tracks, config, youtube, query);
            self.root.set_visible_child_name("search");
            return;
        }

        match self.route() {
            BrowserRoute::Albums => {
                self.rebuild_albums(tracks, youtube, query);
                self.root.set_visible_child_name("albums");
            }
            BrowserRoute::Artists => {
                self.rebuild_artists(tracks, youtube, query);
                self.root.set_visible_child_name("artists");
            }
            BrowserRoute::YouTubeArtist(title) => {
                self.rebuild_artist_albums(youtube, &title, "");
                self.root.set_visible_child_name("albums");
            }
            BrowserRoute::Playlists => {
                self.rebuild_playlists(config, youtube, query);
                self.root.set_visible_child_name("playlists");
            }
            BrowserRoute::All if query.trim().is_empty() => {
                self.rebuild_home(tracks, config, youtube, context.history, context.playback);
                self.root.set_visible_child_name("home");
            }
            route => {
                self.rebuild_queue(tracks, config, youtube, query, &route);
                self.root.set_visible_child_name("tracks");
            }
        }
    }

    pub fn try_recv(&self) -> Option<BrowserEvent> {
        self.events.try_recv().ok()
    }

    pub fn show_more_albums(&self) {
        self.album_display_limit
            .set(self.album_display_limit.get() + COLLECTION_BATCH_INCREMENT);
    }

    pub fn show_more_artists(&self) {
        self.artist_display_limit
            .set(self.artist_display_limit.get() + COLLECTION_BATCH_INCREMENT);
    }

    pub fn visible_indices(&self) -> Vec<usize> {
        self.visible_tracks
            .borrow()
            .iter()
            .filter_map(|entry| match entry {
                VisibleTrack::Local(index) => Some(*index),
                VisibleTrack::YouTube(_) => None,
            })
            .collect()
    }

    pub fn select_track(&self, index: usize) {
        if let Some(position) =
            self.visible_tracks.borrow().iter().position(
                |visible| matches!(visible, VisibleTrack::Local(value) if *value == index),
            )
        {
            if let Some(row) = self.queue.row_at_index(position as i32) {
                self.queue.select_row(Some(&row));
            }
        } else {
            self.queue.unselect_all();
        }
    }

    fn rebuild_search(
        &self,
        tracks: &[Track],
        config: &AppConfig,
        youtube: &YouTubeLibraryCache,
        raw_query: &str,
    ) {
        let query = raw_query.trim().to_lowercase();
        let changed = self.last_search_query.borrow().as_str() != query.as_str();
        if changed {
            self.last_search_query.replace(query.clone());
            self.search_track_limit.set(SEARCH_BATCH_SIZE);
            self.search_album_limit.set(SEARCH_BATCH_SIZE);
            self.search_artist_limit.set(SEARCH_BATCH_SIZE);
            self.search_playlist_limit.set(SEARCH_BATCH_SIZE);
        }

        while let Some(child) = self.search_content.first_child() {
            self.search_content.remove(&child);
        }

        self.search_content.append(&page_header(
            "RESULTADOS DA BUSCA",
            &format!("Resultados para “{}”", raw_query.trim()),
        ));

        let local_mode = config.startup_source != Some(StartupSource::YouTube);
        let online_state_matches =
            !local_mode && youtube.search.query.eq_ignore_ascii_case(raw_query.trim());
        let loading = online_state_matches && youtube.search.loading;

        if online_state_matches && !youtube.search.error.is_empty() && !youtube.search.loading {
            self.search_content
                .append(&search_status_label(&youtube.search.error));
        }

        let mut track_matches = Vec::new();
        if local_mode {
            let mut indices = (0..tracks.len()).collect::<Vec<_>>();
            indices.sort_by(|left, right| compare_library_tracks(&tracks[*left], &tracks[*right]));
            for index in indices {
                let track = &tracks[index];
                let haystack =
                    format!("{} {} {}", track.title, track.artist, track.album).to_lowercase();
                if haystack.contains(&query) {
                    track_matches.push(VisibleTrack::Local(index));
                }
            }
        } else if online_state_matches {
            track_matches.extend(
                youtube
                    .search
                    .songs
                    .iter()
                    .filter(|item| item.playable())
                    .cloned()
                    .map(|item| VisibleTrack::YouTube(Box::new(item))),
            );
        }

        let search_list = gtk::ListBox::new();
        search_list.set_selection_mode(gtk::SelectionMode::Single);
        search_list.add_css_class("queue-list");
        search_list.add_css_class("search-results-list");
        search_list.add_css_class("search-results-surface");
        let visible_entries = Rc::new(RefCell::new(Vec::<VisibleTrack>::new()));
        {
            let tx = self.event_tx.clone();
            let entries = visible_entries.clone();
            search_list.connect_row_activated(move |_, row| {
                let Some(entry) = entries.borrow().get(row.index() as usize).cloned() else {
                    return;
                };
                match entry {
                    VisibleTrack::Local(index) => {
                        let _ = tx.send(BrowserEvent::TrackActivated(index));
                    }
                    VisibleTrack::YouTube(item) => {
                        let queue = entries
                            .borrow()
                            .iter()
                            .filter_map(|entry| match entry {
                                VisibleTrack::YouTube(item) if item.playable() => {
                                    Some(item.as_ref().clone())
                                }
                                _ => None,
                            })
                            .collect::<Vec<_>>();
                        let index = queue
                            .iter()
                            .position(|candidate| candidate.video_id == item.video_id)
                            .unwrap_or(0);
                        let _ = tx.send(BrowserEvent::YouTubeTrackActivated {
                            item: *item,
                            queue,
                            index,
                        });
                    }
                }
            });
        }

        let track_limit = self.search_track_limit.get();
        for (position, entry) in track_matches.iter().take(track_limit).cloned().enumerate() {
            match &entry {
                VisibleTrack::Local(index) => {
                    let track = &tracks[*index];
                    search_list.append(&track_row(
                        position + 1,
                        track,
                        config.is_liked(&track.path),
                        *index,
                        &self.event_tx,
                        config.language,
                    ));
                }
                VisibleTrack::YouTube(item) => {
                    let liked = youtube
                        .liked
                        .iter()
                        .any(|candidate| candidate.video_id == item.video_id);
                    search_list.append(&youtube_track_row(
                        position + 1,
                        item,
                        liked,
                        &self.event_tx,
                        config.language,
                    ));
                }
            }
            visible_entries.borrow_mut().push(entry);
        }

        if track_matches.is_empty() {
            search_list.append(&empty_row(if loading {
                "Buscando faixas no YouTube Music…"
            } else {
                "Nenhuma faixa encontrada"
            }));
        }

        let track_section = gtk::Box::new(gtk::Orientation::Vertical, 10);
        track_section.add_css_class("home-section");
        track_section.add_css_class("search-section-card");
        track_section.append(&search_section_heading(
            "Faixas",
            track_matches.len().min(track_limit),
            track_matches.len(),
            loading,
        ));
        track_section.append(&search_list);
        if track_matches.len() > track_limit {
            track_section.append(&search_more_button(
                "faixas",
                track_matches.len() - track_limit,
                self.search_track_limit.clone(),
                &self.event_tx,
            ));
        }
        self.search_content.append(&track_section);

        self.search_content.append(&search_list_section(
            "Álbuns",
            "álbuns",
            "Nenhum álbum encontrado",
            search_album_cards(tracks, youtube, &query, online_state_matches),
            self.search_album_limit.clone(),
            &self.event_tx,
            loading,
        ));
        self.search_content.append(&search_list_section(
            "Artistas",
            "artistas",
            "Nenhum artista encontrado",
            search_artist_cards(tracks, youtube, &query, online_state_matches),
            self.search_artist_limit.clone(),
            &self.event_tx,
            loading,
        ));
        self.search_content.append(&search_list_section(
            "Playlists",
            "playlists",
            "Nenhuma playlist encontrada",
            search_playlist_cards(tracks, config, youtube, &query, online_state_matches),
            self.search_playlist_limit.clone(),
            &self.event_tx,
            loading,
        ));
    }

    fn rebuild_queue(
        &self,
        tracks: &[Track],
        config: &AppConfig,
        youtube: &YouTubeLibraryCache,
        query: &str,
        route: &BrowserRoute,
    ) {
        let render_token = self.queue_render_generation.get().wrapping_add(1);
        self.queue_render_generation.set(render_token);
        clear_list_box(&self.queue);
        clear_box(&self.queue_context_header);
        self.queue_context_header.set_visible(false);

        let effective_query = match route {
            BrowserRoute::Album(_)
            | BrowserRoute::Artist(_)
            | BrowserRoute::Playlist(_)
            | BrowserRoute::YouTubeAlbum(_)
            | BrowserRoute::YouTubeArtist(_)
            | BrowserRoute::YouTubePlaylist { .. } => "",
            _ => query,
        };

        if let BrowserRoute::YouTubePlaylist { browse_id, .. } = route {
            self.rebuild_youtube_playlist_queue(
                youtube,
                effective_query,
                route,
                browse_id,
                render_token,
                config.language,
            );
            return;
        }

        if matches!(
            route,
            BrowserRoute::YouTubeAlbum(_) | BrowserRoute::YouTubeArtist(_)
        ) {
            self.rebuild_youtube_collection_queue(
                youtube,
                effective_query,
                route,
                render_token,
                config.language,
            );
            return;
        }

        let query = effective_query.trim().to_lowercase();
        let mut entries = Vec::new();

        let mut local_candidates = match route {
            BrowserRoute::Playlist(name) => config
                .playlist(name)
                .map(|playlist| {
                    playlist
                        .tracks
                        .iter()
                        .filter_map(|path| tracks.iter().position(|track| &track.path == path))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            BrowserRoute::Liked if config.startup_source != Some(StartupSource::YouTube) => tracks
                .iter()
                .enumerate()
                .filter_map(|(index, track)| config.is_liked(&track.path).then_some(index))
                .collect::<Vec<_>>(),
            BrowserRoute::Album(album) => tracks
                .iter()
                .enumerate()
                .filter_map(|(index, track)| (track.album == *album).then_some(index))
                .collect::<Vec<_>>(),
            BrowserRoute::Artist(artist) => tracks
                .iter()
                .enumerate()
                .filter_map(|(index, track)| (track.artist == *artist).then_some(index))
                .collect::<Vec<_>>(),
            BrowserRoute::All => (0..tracks.len()).collect::<Vec<_>>(),
            _ => Vec::new(),
        };

        match route {
            BrowserRoute::Playlist(_) => {}
            BrowserRoute::Album(_) => local_candidates
                .sort_by(|left, right| compare_album_tracks(&tracks[*left], &tracks[*right])),
            BrowserRoute::Artist(_) => local_candidates
                .sort_by(|left, right| compare_artist_tracks(&tracks[*left], &tracks[*right])),
            _ => local_candidates
                .sort_by(|left, right| compare_library_tracks(&tracks[*left], &tracks[*right])),
        }

        for index in local_candidates {
            let track = &tracks[index];
            let haystack =
                format!("{} {} {}", track.title, track.artist, track.album).to_lowercase();
            if !query.is_empty() && !haystack.contains(&query) {
                continue;
            }
            let number = entries.len() + 1;
            self.queue.append(&track_row(
                number,
                track,
                config.is_liked(&track.path),
                index,
                &self.event_tx,
                config.language,
            ));
            entries.push(VisibleTrack::Local(index));
        }

        let catalog = youtube_catalog(youtube);
        let mut online_candidates = match route {
            BrowserRoute::All => catalog,
            BrowserRoute::Liked if config.startup_source == Some(StartupSource::YouTube) => youtube
                .liked
                .iter()
                .filter(|item| item.playable())
                .cloned()
                .collect(),
            BrowserRoute::YouTubeAlbum(album) => catalog
                .into_iter()
                .filter(|item| item.album.eq_ignore_ascii_case(album))
                .collect(),
            BrowserRoute::YouTubeArtist(artist) => catalog
                .into_iter()
                .filter(|item| item.artist.eq_ignore_ascii_case(artist))
                .collect(),
            BrowserRoute::YouTubePlaylist { browse_id, .. } => youtube
                .playlist_tracks
                .get(browse_id)
                .cloned()
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        if !matches!(route, BrowserRoute::YouTubePlaylist { .. }) {
            online_candidates.sort_by(compare_youtube_items);
        }

        for item in online_candidates {
            let haystack = format!("{} {} {}", item.title, item.artist, item.album).to_lowercase();
            if !query.is_empty() && !haystack.contains(&query) {
                continue;
            }
            let number = entries.len() + 1;
            let liked = matches!(route, BrowserRoute::Liked)
                || youtube
                    .liked
                    .iter()
                    .any(|candidate| candidate.video_id == item.video_id);
            self.queue.append(&youtube_track_row(
                number,
                &item,
                liked,
                &self.event_tx,
                config.language,
            ));
            entries.push(VisibleTrack::YouTube(Box::new(item)));
        }

        self.queue_title
            .set_text(&route_title(route, config.startup_source, config.language));
        self.queue_title.set_visible(true);
        if entries.is_empty() {
            let message = if youtube.syncing
                && matches!(
                    route,
                    BrowserRoute::All
                        | BrowserRoute::Liked
                        | BrowserRoute::YouTubeAlbum(_)
                        | BrowserRoute::YouTubeArtist(_)
                        | BrowserRoute::YouTubePlaylist { .. }
                ) {
                "Sincronizando sua biblioteca do YouTube Music…"
            } else {
                match route {
                    BrowserRoute::Liked => {
                        liked_empty_message(config.startup_source, config.language)
                    }
                    BrowserRoute::Playlist(_) => "Esta playlist local ainda está vazia",
                    BrowserRoute::YouTubePlaylist { .. } => "Esta playlist ainda está vazia",
                    _ => "Nenhuma faixa encontrada",
                }
            };
            self.queue.append(&empty_row(message));
        }
        self.visible_tracks.replace(entries);
    }

    fn rebuild_youtube_playlist_queue(
        &self,
        youtube: &YouTubeLibraryCache,
        query: &str,
        route: &BrowserRoute,
        browse_id: &str,
        render_token: u64,
        language: AppLanguage,
    ) {
        self.queue_title
            .set_text(&route_title(route, None, language));
        self.visible_tracks.borrow_mut().clear();

        if let Some(playlist) = youtube
            .playlists
            .iter()
            .find(|playlist| playlist.browse_id == browse_id)
        {
            if is_mix_playlist(playlist) {
                let track_count = youtube.playlist_tracks.get(browse_id).map(Vec::len);
                self.queue_context_header.append(&youtube_mix_page_header(
                    playlist,
                    track_count,
                    language,
                ));
                self.queue_context_header.set_visible(true);
                self.queue_title.set_visible(false);
            } else {
                self.queue_title.set_visible(true);
            }
        } else {
            self.queue_title.set_visible(true);
        }

        let query = query.trim().to_lowercase();
        let mut items = youtube
            .playlist_tracks
            .get(browse_id)
            .cloned()
            .unwrap_or_default();
        if !query.is_empty() {
            items.retain(|item| {
                format!("{} {} {}", item.title, item.artist, item.album)
                    .to_lowercase()
                    .contains(&query)
            });
        }

        if items.is_empty() {
            let row = if youtube.playlist_loading.contains(browse_id) {
                loading_row("Carregando playlist do YouTube Music…")
            } else {
                empty_row("Esta playlist ainda está vazia")
            };
            self.queue.append(&row);
            return;
        }

        let liked_ids = youtube
            .liked
            .iter()
            .map(|item| item.video_id.clone())
            .collect::<HashSet<_>>();

        // Paint the first screen immediately, then yield between later batches
        // so large playlists do not freeze animations or input.
        let first_batch = items.len().min(32);
        for item in items.drain(..first_batch) {
            let number = self.visible_tracks.borrow().len() + 1;
            let liked = liked_ids.contains(&item.video_id);
            self.queue.append(&youtube_track_row(
                number,
                &item,
                liked,
                &self.event_tx,
                language,
            ));
            self.visible_tracks
                .borrow_mut()
                .push(VisibleTrack::YouTube(Box::new(item)));
        }

        if items.is_empty() {
            return;
        }

        let pending = Rc::new(RefCell::new(items.into_iter().collect::<VecDeque<_>>()));
        let queue = self.queue.clone();
        let visible_tracks = self.visible_tracks.clone();
        let generation = self.queue_render_generation.clone();
        let event_tx = self.event_tx.clone();

        glib::idle_add_local(move || {
            if generation.get() != render_token {
                return glib::ControlFlow::Break;
            }

            for _ in 0..24 {
                let item = pending.borrow_mut().pop_front();
                let Some(item) = item else {
                    return glib::ControlFlow::Break;
                };
                let number = visible_tracks.borrow().len() + 1;
                let liked = liked_ids.contains(&item.video_id);
                queue.append(&youtube_track_row(
                    number, &item, liked, &event_tx, language,
                ));
                visible_tracks
                    .borrow_mut()
                    .push(VisibleTrack::YouTube(Box::new(item)));
            }

            if pending.borrow().is_empty() {
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });
    }

    fn rebuild_youtube_collection_queue(
        &self,
        youtube: &YouTubeLibraryCache,
        query: &str,
        route: &BrowserRoute,
        render_token: u64,
        language: AppLanguage,
    ) {
        self.queue_title
            .set_text(&route_title(route, None, language));
        self.visible_tracks.borrow_mut().clear();

        let (kind, title) = match route {
            BrowserRoute::YouTubeAlbum(title) => ("album", title.as_str()),
            BrowserRoute::YouTubeArtist(title) => ("artist", title.as_str()),
            _ => return,
        };
        let key = youtube_collection_key(kind, title);
        let catalog = youtube_catalog(youtube);
        let mut items = youtube
            .collection_tracks
            .get(&key)
            .cloned()
            .unwrap_or_else(|| {
                catalog
                    .into_iter()
                    .filter(|item| {
                        if kind == "artist" {
                            item.artist.eq_ignore_ascii_case(title)
                        } else {
                            item.album.eq_ignore_ascii_case(title)
                        }
                    })
                    .collect()
            });

        let query = query.trim().to_lowercase();
        if !query.is_empty() {
            items.retain(|item| {
                format!("{} {} {}", item.title, item.artist, item.album)
                    .to_lowercase()
                    .contains(&query)
            });
        }

        if items.is_empty() {
            let row = if youtube.collection_loading.contains(&key) {
                loading_row(if kind == "artist" {
                    "Carregando faixas do artista…"
                } else {
                    "Carregando faixas do álbum…"
                })
            } else if kind == "artist" {
                empty_row("Nenhuma faixa disponível para este artista")
            } else {
                empty_row("Nenhuma faixa disponível para este álbum")
            };
            self.queue.append(&row);
            return;
        }

        self.append_youtube_rows_progressively(youtube, items, render_token, language);
    }

    fn append_youtube_rows_progressively(
        &self,
        youtube: &YouTubeLibraryCache,
        mut items: Vec<YouTubeItem>,
        render_token: u64,
        language: AppLanguage,
    ) {
        let liked_ids = youtube
            .liked
            .iter()
            .map(|item| item.video_id.clone())
            .collect::<HashSet<_>>();

        let first_batch = items.len().min(32);
        for item in items.drain(..first_batch) {
            let number = self.visible_tracks.borrow().len() + 1;
            let liked = liked_ids.contains(&item.video_id);
            self.queue.append(&youtube_track_row(
                number,
                &item,
                liked,
                &self.event_tx,
                language,
            ));
            self.visible_tracks
                .borrow_mut()
                .push(VisibleTrack::YouTube(Box::new(item)));
        }

        if items.is_empty() {
            return;
        }

        let pending = Rc::new(RefCell::new(items.into_iter().collect::<VecDeque<_>>()));
        let queue = self.queue.clone();
        let visible_tracks = self.visible_tracks.clone();
        let generation = self.queue_render_generation.clone();
        let event_tx = self.event_tx.clone();

        glib::idle_add_local(move || {
            if generation.get() != render_token {
                return glib::ControlFlow::Break;
            }

            for _ in 0..24 {
                let item = pending.borrow_mut().pop_front();
                let Some(item) = item else {
                    return glib::ControlFlow::Break;
                };
                let number = visible_tracks.borrow().len() + 1;
                let liked = liked_ids.contains(&item.video_id);
                queue.append(&youtube_track_row(
                    number, &item, liked, &event_tx, language,
                ));
                visible_tracks
                    .borrow_mut()
                    .push(VisibleTrack::YouTube(Box::new(item)));
            }

            if pending.borrow().is_empty() {
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });
    }

    fn rebuild_home(
        &self,
        tracks: &[Track],
        config: &AppConfig,
        youtube: &YouTubeLibraryCache,
        history: &ListeningHistory,
        playback: &BrowserPlaybackState,
    ) {
        let language = config.language;
        let copy = home_copy(language);
        let card_effects = config.visual_theme == VisualTheme::MaterialExpressive
            && config.expressive_home_card_effects;

        let next_home = gtk::Box::new(gtk::Orientation::Vertical, 22);
        next_home.set_hexpand(true);
        next_home.set_vexpand(false);
        next_home.add_css_class("library-home");
        next_home.add_css_class("expressive-library-home");

        if matches!(config.startup_source, Some(StartupSource::YouTube)) {
            let mixes = youtube
                .playlists
                .iter()
                .filter(|playlist| is_mix_playlist(playlist))
                .cloned()
                .chain(
                    youtube
                        .playlists
                        .iter()
                        .filter(|playlist| !is_mix_playlist(playlist))
                        .cloned(),
                )
                .take(12)
                .map(HomeCard::YouTubePlaylist)
                .collect::<Vec<_>>();

            if !mixes.is_empty() {
                next_home.append(&home_section(
                    copy.mixtapes_title,
                    copy.mixtapes_subtitle,
                    mixes,
                    playback,
                    config,
                    &self.event_tx,
                    language,
                    card_effects,
                ));
            }
        }

        let active_source = match config.startup_source {
            Some(StartupSource::YouTube) => ListeningSource::YouTube,
            _ => ListeningSource::Local,
        };
        let youtube_home = active_source == ListeningSource::YouTube;

        if config.show_personalized_home_history {
            let mut recent_activity = history.recent_activity(ListeningSource::Local, 12);
            recent_activity.extend(history.recent_activity(ListeningSource::YouTube, 12));
            recent_activity.sort_by_key(|item| {
                std::cmp::Reverse(match item {
                    HistoryActivity::Track(track) => track.played_at,
                    HistoryActivity::Collection(collection) => collection.played_at,
                })
            });
            recent_activity.truncate(12);
            let recent = history_home_activity(tracks, youtube, recent_activity);
            if !recent.is_empty() {
                next_home.append(&home_history_section(
                    copy.recent_activity_title,
                    copy.recent_activity_subtitle,
                    recent,
                    &self.event_tx,
                    language,
                    card_effects,
                ));
            }
        }

        if active_source == ListeningSource::Local {
            let recently_added =
                recently_added_local_album_cards(tracks, language, copy.recently_added_detail);
            if !recently_added.is_empty() {
                next_home.append(&home_section(
                    copy.recently_added_title,
                    copy.recently_added_subtitle,
                    recently_added,
                    playback,
                    config,
                    &self.event_tx,
                    language,
                    card_effects,
                ));
            }
        }

        next_home.append(&home_section(
            copy.albums_title,
            copy.albums_subtitle,
            ranked_home_album_cards(tracks, youtube, history, active_source, language),
            playback,
            config,
            &self.event_tx,
            language,
            card_effects,
        ));

        next_home.append(&home_section(
            copy.artists_title,
            copy.artists_subtitle,
            ranked_home_artist_cards(tracks, youtube, history, active_source, language),
            playback,
            config,
            &self.event_tx,
            language,
            card_effects,
        ));

        let playlist_cards = if youtube_home {
            youtube
                .playlists
                .iter()
                .filter(|playlist| !is_mix_playlist(playlist))
                .take(12)
                .cloned()
                .map(HomeCard::YouTubePlaylist)
                .collect::<Vec<_>>()
        } else {
            config
                .playlists
                .iter()
                .take(8)
                .map(|playlist| HomeCard::LocalPlaylist {
                    title: playlist.name.clone(),
                    subtitle: format_local_track_count(language, playlist.tracks.len()),
                })
                .collect::<Vec<_>>()
        };

        if !playlist_cards.is_empty() {
            next_home.append(&home_section(
                copy.playlists_title,
                copy.playlists_subtitle,
                playlist_cards,
                playback,
                config,
                &self.event_tx,
                language,
                card_effects,
            ));
        }

        if youtube_home && youtube.syncing {
            next_home.append(&home_syncing_hint(language));
        }

        let generation = self.home_generation.get().wrapping_add(1);
        self.home_generation.set(generation);
        let child_name = format!("home-{generation}");
        let previous = self.home_stack.visible_child();

        self.home_stack.add_named(&next_home, Some(&child_name));
        self.home_stack.set_visible_child_name(&child_name);

        if let Some(previous) = previous {
            let stack = self.home_stack.clone();
            glib::timeout_add_local_once(Duration::from_millis(220), move || {
                if previous.parent().as_ref() == Some(stack.upcast_ref()) {
                    stack.remove(&previous);
                }
            });
        }
    }

    fn rebuild_albums(&self, tracks: &[Track], youtube: &YouTubeLibraryCache, query: &str) {
        clear_grid(&self.albums_grid);
        let query = query.trim().to_lowercase();
        let limit = if query.is_empty() {
            self.album_display_limit.get()
        } else {
            usize::MAX
        };
        let mut position = 0;
        let mut hidden = 0;

        let mut local_groups: BTreeMap<String, Vec<&Track>> = BTreeMap::new();
        for track in tracks {
            local_groups
                .entry(track.album.clone())
                .or_default()
                .push(track);
        }
        for (album, album_tracks) in local_groups {
            let artists = album_tracks
                .iter()
                .map(|track| track.artist.as_str())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
                .join(", ");
            let haystack = format!("{album} {artists}").to_lowercase();
            if !query.is_empty() && !haystack.contains(&query) {
                continue;
            }
            if position as usize >= limit {
                hidden += 1;
                continue;
            }
            let cover = album_tracks
                .iter()
                .find_map(|track| track.cover_path.as_deref());
            append_collection_grid_card(
                &self.albums_grid,
                position,
                collection_button(
                    collection_card(
                        cover,
                        &album,
                        &artists,
                        &format!("Local • {} faixas", album_tracks.len()),
                        false,
                    ),
                    BrowserRoute::Album(album),
                    &self.event_tx,
                ),
            );
            position += 1;
        }

        for album_entry in &youtube.albums {
            let haystack = format!("{} {}", album_entry.title, album_entry.subtitle).to_lowercase();
            if !query.is_empty() && !haystack.contains(&query) {
                continue;
            }
            if position as usize >= limit {
                hidden += 1;
                continue;
            }
            append_collection_grid_card(
                &self.albums_grid,
                position,
                collection_event_button(
                    collection_card(
                        album_entry.cached_cover(),
                        &album_entry.title,
                        &album_entry.subtitle,
                        &album_entry.detail,
                        true,
                    ),
                    BrowserEvent::OpenYouTubeCollection(album_entry.source.clone()),
                    &self.event_tx,
                ),
            );
            position += 1;
        }

        if hidden > 0 {
            append_collection_grid_card(
                &self.albums_grid,
                position,
                collection_event_button(
                    collection_placeholder("Carregar mais álbuns", &format!("{hidden} restantes")),
                    BrowserEvent::LoadMoreAlbums,
                    &self.event_tx,
                ),
            );
            position += 1;
        }

        if position == 0 && youtube.syncing {
            append_collection_grid_card(
                &self.albums_grid,
                position,
                collection_button(
                    collection_placeholder(
                        "Sincronizando...",
                        "Carregando álbuns do YouTube Music",
                    ),
                    BrowserRoute::Albums,
                    &self.event_tx,
                ),
            );
        }
    }

    fn rebuild_artists(&self, tracks: &[Track], youtube: &YouTubeLibraryCache, query: &str) {
        clear_grid(&self.artists_grid);
        let query = query.trim().to_lowercase();
        let limit = if query.is_empty() {
            self.artist_display_limit.get()
        } else {
            usize::MAX
        };
        let mut position = 0;
        let mut hidden = 0;

        let mut local_names = tracks
            .iter()
            .map(|track| track.artist.trim())
            .filter(|artist| !artist.is_empty())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        local_names.sort_by(|left, right| compare_text(left, right));

        for artist in local_names {
            if !query.is_empty() && !artist.to_lowercase().contains(&query) {
                continue;
            }
            if position as usize >= limit {
                hidden += 1;
                continue;
            }

            append_collection_grid_card(
                &self.artists_grid,
                position,
                artist_list_button(
                    &artist,
                    BrowserEvent::Navigate(BrowserRoute::Artist(artist.clone())),
                    &self.event_tx,
                ),
            );
            position += 1;
        }

        let mut online_artists = youtube.artists.iter().collect::<Vec<_>>();
        online_artists.sort_by(|left, right| compare_text(&left.title, &right.title));

        for artist_entry in online_artists {
            if !query.is_empty() && !artist_entry.title.to_lowercase().contains(&query) {
                continue;
            }
            if position as usize >= limit {
                hidden += 1;
                continue;
            }

            append_collection_grid_card(
                &self.artists_grid,
                position,
                artist_list_button(
                    &artist_entry.title,
                    BrowserEvent::OpenYouTubeCollection(artist_entry.source.clone()),
                    &self.event_tx,
                ),
            );
            position += 1;
        }

        if hidden > 0 {
            append_collection_grid_card(
                &self.artists_grid,
                position,
                artist_list_button(
                    &format!("Carregar mais artistas ({hidden} restantes)"),
                    BrowserEvent::LoadMoreArtists,
                    &self.event_tx,
                ),
            );
            position += 1;
        }

        if position == 0 {
            append_collection_grid_card(
                &self.artists_grid,
                position,
                artist_list_placeholder(if youtube.syncing {
                    "Sincronizando artistas…"
                } else {
                    "Nenhum artista encontrado"
                }),
            );
        }
    }

    fn rebuild_artist_albums(&self, youtube: &YouTubeLibraryCache, artist: &str, query: &str) {
        clear_grid(&self.albums_grid);
        let key = youtube_collection_key("artist", artist);
        let query = query.trim().to_lowercase();
        let mut position = 0;

        if let Some(albums) = youtube.artist_albums.get(&key) {
            for album in albums {
                let haystack =
                    format!("{} {} {}", album.title, album.artist, album.subtitle).to_lowercase();
                if !query.is_empty() && !haystack.contains(&query) {
                    continue;
                }
                append_collection_grid_card(
                    &self.albums_grid,
                    position,
                    collection_event_button(
                        collection_card(
                            album.cached_cover(),
                            &album.title,
                            if album.subtitle.is_empty() {
                                artist
                            } else {
                                &album.subtitle
                            },
                            "YouTube Music • lançamento do artista",
                            true,
                        ),
                        BrowserEvent::OpenYouTubeCollection(album.clone()),
                        &self.event_tx,
                    ),
                );
                position += 1;
            }
        }

        if position == 0 {
            append_collection_grid_card(
                &self.albums_grid,
                position,
                collection_button(
                    collection_placeholder(
                        if youtube.artist_loading.contains(&key) {
                            "Carregando discografia..."
                        } else {
                            "Nenhum álbum encontrado"
                        },
                        artist,
                    ),
                    BrowserRoute::YouTubeArtist(artist.to_string()),
                    &self.event_tx,
                ),
            );
        }
    }

    fn rebuild_playlists(&self, config: &AppConfig, youtube: &YouTubeLibraryCache, query: &str) {
        clear_list_box(&self.playlists_list);
        let query = query.trim().to_lowercase();
        let previous = self.playlist_dropdown.selected() as usize;

        while self.playlist_model.n_items() > 0 {
            self.playlist_model.remove(0);
        }

        let mut all_names = Vec::new();
        let mut row_refs = Vec::new();
        let local_matches = config
            .playlists
            .iter()
            .filter(|playlist| query.is_empty() || playlist.name.to_lowercase().contains(&query))
            .collect::<Vec<_>>();
        if !local_matches.is_empty() {
            self.playlists_list.append(&section_row("PLAYLISTS LOCAIS"));
            row_refs.push(None);
        }
        for playlist in &config.playlists {
            self.playlist_model.append(&playlist.name);
            all_names.push(playlist.name.clone());
            if !query.is_empty() && !playlist.name.to_lowercase().contains(&query) {
                continue;
            }
            self.playlists_list.append(&playlist_row(
                &playlist.name,
                &format!("{} faixas", playlist.tracks.len()),
                false,
            ));
            row_refs.push(Some(PlaylistRef::Local(playlist.name.clone())));
        }

        let online_matches = youtube
            .playlists
            .iter()
            .filter(|playlist| query.is_empty() || playlist.title.to_lowercase().contains(&query))
            .collect::<Vec<_>>();

        let mixes = online_matches
            .iter()
            .copied()
            .filter(|playlist| is_mix_playlist(playlist))
            .collect::<Vec<_>>();
        let regular_playlists = online_matches
            .iter()
            .copied()
            .filter(|playlist| !is_mix_playlist(playlist))
            .collect::<Vec<_>>();

        if !mixes.is_empty() {
            self.playlists_list.append(&section_row("MIXES PARA VOCÊ"));
            row_refs.push(None);
        }

        for mix in mixes {
            let track_count = youtube.playlist_tracks.get(&mix.browse_id).map(Vec::len);

            self.playlists_list
                .append(&youtube_mix_row(mix, track_count));
            row_refs.push(Some(PlaylistRef::YouTube(Box::new(mix.clone()))));
        }

        if !regular_playlists.is_empty() {
            self.playlists_list.append(&section_row("YOUTUBE MUSIC"));
            row_refs.push(None);
        }

        for playlist in regular_playlists {
            self.playlists_list.append(&playlist_row(
                &playlist.title,
                youtube_playlist_subtitle(playlist),
                true,
            ));
            row_refs.push(Some(PlaylistRef::YouTube(Box::new(playlist.clone()))));
        }

        if row_refs.is_empty() {
            self.playlists_list.append(&empty_row(if youtube.syncing {
                "Sincronizando suas playlists do YouTube Music…"
            } else {
                "Nenhuma playlist encontrada"
            }));
            row_refs.push(None);
        }

        self.playlist_names.replace(all_names);
        self.playlist_row_refs.replace(row_refs);

        let count = self.playlist_model.n_items();
        if count > 0 {
            self.playlist_dropdown
                .set_selected((previous.min(count as usize - 1)) as u32);
        }
        self.playlist_dropdown.set_sensitive(count > 0);
    }
}

fn youtube_catalog(youtube: &YouTubeLibraryCache) -> Vec<YouTubeItem> {
    let mut seen = HashSet::new();
    youtube
        .recently_played
        .iter()
        .chain(youtube.library.iter())
        .chain(youtube.liked.iter())
        .filter(|item| item.playable())
        .filter(|item| seen.insert(item.video_id.clone()))
        .cloned()
        .collect()
}

fn history_home_activity(
    tracks: &[Track],
    youtube: &YouTubeLibraryCache,
    activity: Vec<HistoryActivity>,
) -> Vec<HomeHistoryTrack> {
    let catalog = youtube_catalog(youtube);
    let mut entries = Vec::new();

    for item in activity {
        match item {
            HistoryActivity::Track(item) => match item.source {
                ListeningSource::Local => {
                    let exact_path = tracks
                        .iter()
                        .enumerate()
                        .find(|(_, track)| track.path.to_string_lossy().as_ref() == item.media_id);
                    let metadata_match = tracks.iter().enumerate().find(|(_, track)| {
                        track.title.eq_ignore_ascii_case(&item.title)
                            && track.artist.eq_ignore_ascii_case(&item.artist)
                            && (item.album.trim().is_empty()
                                || track.album.eq_ignore_ascii_case(&item.album))
                    });

                    if let Some((index, track)) = exact_path.or(metadata_match) {
                        entries.push(HomeHistoryTrack::LocalTrack {
                            index,
                            track: track.clone(),
                            position_seconds: item.position_seconds,
                            duration_seconds: item.duration_seconds,
                            completed: item.completed,
                        });
                    }
                }
                ListeningSource::YouTube => {
                    let track = catalog
                        .iter()
                        .find(|track| track.video_id == item.media_id)
                        .cloned()
                        .unwrap_or_else(|| YouTubeItem {
                            result_type: "song".to_string(),
                            video_id: item.media_id.clone(),
                            title: item.title.clone(),
                            artist: item.artist.clone(),
                            album: item.album.clone(),
                            duration_seconds: item.duration_seconds,
                            ..YouTubeItem::default()
                        });

                    if !track.video_id.trim().is_empty() && !track.title.trim().is_empty() {
                        entries.push(HomeHistoryTrack::YouTubeTrack {
                            item: track,
                            position_seconds: item.position_seconds,
                            duration_seconds: item.duration_seconds,
                            completed: item.completed,
                        });
                    }
                }
            },
            HistoryActivity::Collection(collection) => match collection.source {
                ListeningSource::Local if collection.kind == "album" => {
                    entries.push(HomeHistoryTrack::LocalAlbum(collection.title));
                }
                ListeningSource::Local if collection.kind == "playlist" => {
                    entries.push(HomeHistoryTrack::LocalPlaylist(collection.title));
                }
                ListeningSource::YouTube if collection.kind == "album" => {
                    let resolved = youtube.albums.iter().find(|entry| {
                        entry.source.browse_id == collection.id
                            || entry.title.eq_ignore_ascii_case(&collection.title)
                    });

                    let (item, cover_path) = if let Some(entry) = resolved {
                        (
                            entry.source.clone(),
                            entry.cached_cover().map(Path::to_path_buf),
                        )
                    } else {
                        let cover_path = catalog
                            .iter()
                            .find(|track| track.album.eq_ignore_ascii_case(&collection.title))
                            .and_then(YouTubeItem::cached_cover)
                            .map(Path::to_path_buf);

                        (
                            YouTubeItem {
                                result_type: "album".to_string(),
                                browse_id: collection.id.clone(),
                                title: collection.title.clone(),
                                album: collection.title.clone(),
                                ..YouTubeItem::default()
                            },
                            cover_path,
                        )
                    };

                    entries.push(HomeHistoryTrack::YouTubeAlbum { item, cover_path });
                }
                ListeningSource::YouTube if collection.kind == "playlist" => {
                    let item = youtube
                        .playlists
                        .iter()
                        .find(|item| {
                            item.browse_id == collection.id
                                || item.title.eq_ignore_ascii_case(&collection.title)
                        })
                        .cloned()
                        .unwrap_or_else(|| YouTubeItem {
                            result_type: "playlist".to_string(),
                            browse_id: collection.id.clone(),
                            title: collection.title.clone(),
                            ..YouTubeItem::default()
                        });
                    entries.push(HomeHistoryTrack::YouTubePlaylist(item));
                }
                _ => {}
            },
        }
    }

    entries
}

fn format_history_position(language: AppLanguage, seconds: u64) -> String {
    let minutes = seconds / 60;
    let remaining = seconds % 60;
    match language {
        AppLanguage::Portuguese => format!("Retomar em {minutes}:{remaining:02}"),
        AppLanguage::English => format!("Resume at {minutes}:{remaining:02}"),
        AppLanguage::Spanish => format!("Retomar en {minutes}:{remaining:02}"),
    }
}

fn recently_played_detail(language: AppLanguage, online: bool) -> String {
    let source = if online { "YouTube Music" } else { "Local" };
    match language {
        AppLanguage::Portuguese => format!("{source} • Tocada recentemente"),
        AppLanguage::English => format!("{source} • Recently played"),
        AppLanguage::Spanish => format!("{source} • Reproducida recientemente"),
    }
}

fn ranked_home_album_cards(
    tracks: &[Track],
    youtube: &YouTubeLibraryCache,
    history: &ListeningHistory,
    source: ListeningSource,
    language: AppLanguage,
) -> Vec<HomeCard> {
    let mut ranked = history.ranked_albums(source, 12);
    for album in history.recent_albums(source, 12) {
        if !ranked
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case(&album))
        {
            ranked.push((album, ListeningStats::default()));
        }
        if ranked.len() == 12 {
            break;
        }
    }

    let fallback = home_album_cards(tracks, youtube, language);
    let catalog = youtube_catalog(youtube);
    let mut cards = Vec::new();

    for (name, stats) in ranked {
        match source {
            ListeningSource::Local => {
                let album_tracks = tracks
                    .iter()
                    .filter(|track| track.album.eq_ignore_ascii_case(&name))
                    .collect::<Vec<_>>();
                if album_tracks.is_empty() {
                    continue;
                }

                let artists = album_tracks
                    .iter()
                    .map(|track| track.artist.as_str())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>()
                    .join(", ");
                let fallback_detail = format!(
                    "Local • {}",
                    format_track_count(language, album_tracks.len())
                );

                cards.push(HomeCard::LocalAlbum {
                    title: name,
                    subtitle: artists,
                    detail: listening_rank_detail(&stats, &fallback_detail, language),
                    cover_path: album_tracks
                        .iter()
                        .find_map(|track| track.cover_path.clone()),
                });
            }
            ListeningSource::YouTube => {
                if let Some(entry) = youtube
                    .albums
                    .iter()
                    .find(|entry| entry.title.eq_ignore_ascii_case(&name))
                {
                    cards.push(HomeCard::YouTubeAlbum {
                        item: entry.source.clone(),
                        subtitle: entry.subtitle.clone(),
                        detail: listening_rank_detail(&stats, &entry.detail, language),
                        cover_path: entry.cached_cover().map(Path::to_path_buf),
                    });
                    continue;
                }

                let matching = catalog
                    .iter()
                    .filter(|item| item.album.eq_ignore_ascii_case(&name))
                    .collect::<Vec<_>>();
                let Some(first) = matching.first() else {
                    continue;
                };
                let artists = matching
                    .iter()
                    .map(|item| item.artist.as_str())
                    .filter(|artist| !artist.is_empty())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>()
                    .join(", ");
                let source_item = YouTubeItem {
                    result_type: "album".to_string(),
                    title: name.clone(),
                    album: name.clone(),
                    artist: artists.clone(),
                    ..YouTubeItem::default()
                };
                let fallback_detail = format!(
                    "YouTube Music • {}",
                    format_track_count(language, matching.len())
                );

                cards.push(HomeCard::YouTubeAlbum {
                    item: source_item,
                    subtitle: artists,
                    detail: listening_rank_detail(&stats, &fallback_detail, language),
                    cover_path: crate::youtube::cached_cover_for_item(first),
                });
            }
        }
    }

    merge_ranked_home_cards(cards, fallback, 12)
}

fn ranked_home_artist_cards(
    tracks: &[Track],
    youtube: &YouTubeLibraryCache,
    history: &ListeningHistory,
    source: ListeningSource,
    language: AppLanguage,
) -> Vec<HomeCard> {
    let ranked = history.ranked_artists(source, 12);
    let fallback = home_artist_cards(tracks, youtube);
    let catalog = youtube_catalog(youtube);
    let mut cards = Vec::new();

    for (name, stats) in ranked {
        match source {
            ListeningSource::Local => {
                let artist_tracks = tracks
                    .iter()
                    .filter(|track| track.artist.eq_ignore_ascii_case(&name))
                    .collect::<Vec<_>>();
                if artist_tracks.is_empty() {
                    continue;
                }

                let fallback_detail = format!(
                    "Local • {}",
                    format_track_count(language, artist_tracks.len())
                );
                cards.push(HomeCard::LocalArtist {
                    title: name,
                    subtitle: String::new(),
                    detail: listening_rank_detail(&stats, &fallback_detail, language),
                    cover_path: artist_tracks
                        .iter()
                        .find_map(|track| track.cover_path.clone()),
                });
            }
            ListeningSource::YouTube => {
                if let Some(entry) = youtube
                    .artists
                    .iter()
                    .find(|entry| entry.title.eq_ignore_ascii_case(&name))
                {
                    cards.push(HomeCard::YouTubeArtist {
                        item: entry.source.clone(),
                        subtitle: entry.subtitle.clone(),
                        detail: listening_rank_detail(&stats, &entry.detail, language),
                        cover_path: entry.cached_cover().map(Path::to_path_buf),
                    });
                    continue;
                }

                let matching = catalog
                    .iter()
                    .filter(|item| item.artist.eq_ignore_ascii_case(&name))
                    .collect::<Vec<_>>();
                let Some(first) = matching.first() else {
                    continue;
                };
                let album_count = matching
                    .iter()
                    .map(|item| item.album.trim())
                    .filter(|album| !album.is_empty())
                    .collect::<BTreeSet<_>>()
                    .len();
                let source_item = YouTubeItem {
                    result_type: "artist".to_string(),
                    title: name.clone(),
                    artist: name.clone(),
                    ..YouTubeItem::default()
                };
                let fallback_detail = format!(
                    "YouTube Music • {}",
                    format_track_count(language, matching.len())
                );

                cards.push(HomeCard::YouTubeArtist {
                    item: source_item,
                    subtitle: format_album_count(language, album_count),
                    detail: listening_rank_detail(&stats, &fallback_detail, language),
                    cover_path: crate::youtube::cached_cover_for_item(first),
                });
            }
        }
    }

    merge_ranked_home_cards(cards, fallback, 12)
}

fn listening_rank_detail(stats: &ListeningStats, fallback: &str, language: AppLanguage) -> String {
    if stats.play_count == 0 {
        return fallback.to_string();
    }

    let minutes = stats.total_listened_seconds.div_ceil(60);
    localized_listening_detail(language, stats.play_count, minutes)
}

fn merge_ranked_home_cards(
    ranked: Vec<HomeCard>,
    fallback: Vec<HomeCard>,
    limit: usize,
) -> Vec<HomeCard> {
    let mut cards = Vec::new();
    let mut seen = HashSet::new();

    for card in ranked.into_iter().chain(fallback) {
        if seen.insert(home_card_identity(&card)) {
            cards.push(card);
        }
        if cards.len() == limit {
            break;
        }
    }

    cards
}

fn home_card_identity(card: &HomeCard) -> String {
    card.identity()
}

fn search_album_cards(
    tracks: &[Track],
    youtube: &YouTubeLibraryCache,
    query: &str,
    online_state_matches: bool,
) -> Vec<HomeCard> {
    let mut cards = Vec::new();
    if !tracks.is_empty() {
        let mut groups: BTreeMap<String, Vec<&Track>> = BTreeMap::new();
        for track in tracks {
            groups.entry(track.album.clone()).or_default().push(track);
        }
        for (album, album_tracks) in groups {
            let artists = album_tracks
                .iter()
                .map(|track| track.artist.as_str())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
                .join(", ");
            if !format!("{album} {artists}").to_lowercase().contains(query) {
                continue;
            }
            cards.push(HomeCard::LocalAlbum {
                title: album,
                subtitle: artists,
                detail: format!("Local • {} faixas", album_tracks.len()),
                cover_path: album_tracks
                    .iter()
                    .find_map(|track| track.cover_path.clone()),
            });
        }
    } else if online_state_matches {
        cards.extend(
            youtube
                .search
                .albums
                .iter()
                .cloned()
                .map(|item| HomeCard::YouTubeAlbum {
                    subtitle: if item.artist.is_empty() {
                        item.subtitle.clone()
                    } else {
                        item.artist.clone()
                    },
                    detail: "Álbum • YouTube Music".to_string(),
                    cover_path: item.cached_cover().map(Path::to_path_buf),
                    item,
                }),
        );
    }
    cards
}

fn search_artist_cards(
    tracks: &[Track],
    youtube: &YouTubeLibraryCache,
    query: &str,
    online_state_matches: bool,
) -> Vec<HomeCard> {
    let mut cards = Vec::new();
    if !tracks.is_empty() {
        let mut groups: BTreeMap<String, Vec<&Track>> = BTreeMap::new();
        for track in tracks {
            groups.entry(track.artist.clone()).or_default().push(track);
        }
        for (artist, artist_tracks) in groups {
            if !artist.to_lowercase().contains(query) {
                continue;
            }
            cards.push(HomeCard::LocalArtist {
                title: artist,
                subtitle: String::new(),
                detail: format!("Local • {} faixas", artist_tracks.len()),
                cover_path: artist_tracks
                    .iter()
                    .find_map(|track| track.cover_path.clone()),
            });
        }
    } else if online_state_matches {
        cards.extend(
            youtube
                .search
                .artists
                .iter()
                .cloned()
                .map(|item| HomeCard::YouTubeArtist {
                    subtitle: if item.subtitle.is_empty() {
                        "Artista".to_string()
                    } else {
                        item.subtitle.clone()
                    },
                    detail: "Artista • YouTube Music".to_string(),
                    cover_path: item.cached_cover().map(Path::to_path_buf),
                    item,
                }),
        );
    }
    cards
}

fn search_playlist_cards(
    tracks: &[Track],
    config: &AppConfig,
    youtube: &YouTubeLibraryCache,
    query: &str,
    online_state_matches: bool,
) -> Vec<HomeCard> {
    let mut cards = Vec::new();
    if !tracks.is_empty() {
        for playlist in &config.playlists {
            if playlist.name.to_lowercase().contains(query) {
                cards.push(HomeCard::LocalPlaylist {
                    title: playlist.name.clone(),
                    subtitle: format!("{} faixas locais", playlist.tracks.len()),
                });
            }
        }
    } else if online_state_matches {
        cards.extend(
            youtube
                .search
                .playlists
                .iter()
                .cloned()
                .map(HomeCard::YouTubePlaylist),
        );
    }
    cards
}

fn home_album_cards(
    tracks: &[Track],
    youtube: &YouTubeLibraryCache,
    language: AppLanguage,
) -> Vec<HomeCard> {
    let mut cards = Vec::new();

    let mut local_groups: BTreeMap<String, Vec<&Track>> = BTreeMap::new();
    for track in tracks {
        local_groups
            .entry(track.album.clone())
            .or_default()
            .push(track);
    }
    for (album, album_tracks) in local_groups.into_iter().take(12) {
        let artists = album_tracks
            .iter()
            .map(|track| track.artist.as_str())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join(", ");
        let cover_path = album_tracks
            .iter()
            .find_map(|track| track.cover_path.clone());
        cards.push(HomeCard::LocalAlbum {
            title: album,
            subtitle: artists,
            detail: format!(
                "Local • {}",
                format_track_count(language, album_tracks.len())
            ),
            cover_path,
        });
    }

    for album in youtube.albums.iter().take(12) {
        cards.push(youtube_album_home_card(album));
    }

    cards.truncate(18);
    cards
}

fn home_artist_cards(tracks: &[Track], youtube: &YouTubeLibraryCache) -> Vec<HomeCard> {
    let mut cards = Vec::new();

    let mut local_groups: BTreeMap<String, Vec<&Track>> = BTreeMap::new();
    for track in tracks {
        local_groups
            .entry(track.artist.clone())
            .or_default()
            .push(track);
    }
    for (artist, artist_tracks) in local_groups.into_iter().take(12) {
        let cover_path = artist_tracks
            .iter()
            .find_map(|track| track.cover_path.clone());
        cards.push(HomeCard::LocalArtist {
            title: artist,
            subtitle: String::new(),
            detail: String::new(),
            cover_path,
        });
    }

    for artist in youtube.artists.iter().take(12) {
        let key = youtube_collection_key("artist", &artist.title);
        let profile = youtube.artist_profiles.get(&key);
        cards.push(youtube_artist_home_card_from_source(artist, profile));
    }

    cards.truncate(18);
    cards
}

fn youtube_album_home_card(entry: &YouTubeCollectionEntry) -> HomeCard {
    HomeCard::YouTubeAlbum {
        item: entry.source.clone(),
        subtitle: entry.subtitle.clone(),
        detail: entry.detail.clone(),
        cover_path: entry.cached_cover().map(Path::to_path_buf),
    }
}

fn youtube_artist_home_card_from_source(
    entry: &YouTubeCollectionEntry,
    profile: Option<&YouTubeItem>,
) -> HomeCard {
    HomeCard::YouTubeArtist {
        item: entry.source.clone(),
        subtitle: String::new(),
        detail: String::new(),
        cover_path: profile
            .and_then(YouTubeItem::cached_cover)
            .map(Path::to_path_buf),
    }
}

fn is_mix_playlist(playlist: &YouTubeItem) -> bool {
    if playlist.playlist_kind == "mix" {
        return true;
    }
    let title = playlist.title.to_lowercase();
    title.contains("mix") || title.contains("radio") || title.contains("supermix")
}

fn youtube_playlist_detail(item: &YouTubeItem) -> &'static str {
    match item.playlist_kind.as_str() {
        "mix" => "Mix gerado pelo YouTube Music",
        "recommended" => "Recomendação do YouTube Music",
        _ => "YouTube Music",
    }
}

fn youtube_playlist_subtitle(item: &YouTubeItem) -> &str {
    if !item.subtitle.is_empty() {
        return item.subtitle.as_str();
    }
    match item.playlist_kind.as_str() {
        "mix" => "Mix gerado pelo YouTube Music",
        "recommended" => "Recomendação do YouTube Music",
        _ => "Playlist sincronizada",
    }
}

fn search_section_heading(title: &str, visible: usize, total: usize, loading: bool) -> gtk::Box {
    let title_label = gtk::Label::new(Some(title));
    title_label.set_xalign(0.0);
    title_label.add_css_class("home-section-title");

    let subtitle = if loading && total == 0 {
        "Buscando no YouTube Music…".to_string()
    } else if total == 0 {
        "Nenhum resultado".to_string()
    } else {
        format!("Mostrando {visible} de {total} resultados")
    };
    let subtitle_label = gtk::Label::new(Some(&subtitle));
    subtitle_label.set_xalign(0.0);
    subtitle_label.add_css_class("dim-label");

    let heading = gtk::Box::new(gtk::Orientation::Vertical, 2);
    heading.add_css_class("search-section-heading");
    heading.append(&title_label);
    heading.append(&subtitle_label);
    heading
}

fn search_status_label(message: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(message));
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.add_css_class("dim-label");
    label
}

fn search_more_button(
    category: &str,
    remaining: usize,
    limit: Rc<Cell<usize>>,
    event_tx: &Sender<BrowserEvent>,
) -> gtk::Button {
    let next = remaining.min(SEARCH_BATCH_SIZE);
    let button = gtk::Button::with_label(&format!("Carregar mais {next} {category}"));
    button.set_halign(gtk::Align::Start);
    button.add_css_class("pill");
    let sender = event_tx.clone();
    button.connect_clicked(move |_| {
        limit.set(limit.get().saturating_add(SEARCH_BATCH_SIZE));
        let _ = sender.send(BrowserEvent::RefreshSearch);
    });
    button
}

fn search_list_section(
    title: &str,
    category: &str,
    empty_message: &str,
    cards: Vec<HomeCard>,
    limit: Rc<Cell<usize>>,
    event_tx: &Sender<BrowserEvent>,
    loading: bool,
) -> gtk::Box {
    let total = cards.len();
    let visible = total.min(limit.get());
    let section = gtk::Box::new(gtk::Orientation::Vertical, 10);
    section.add_css_class("home-section");
    section.add_css_class("search-section-card");
    section.append(&search_section_heading(title, visible, total, loading));

    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::None);
    list.add_css_class("boxed-list");
    list.add_css_class("search-results-list");
    list.add_css_class("search-results-surface");

    if total == 0 {
        list.append(&empty_row(if loading {
            "Buscando no YouTube Music…"
        } else {
            empty_message
        }));
    } else {
        for card in cards.into_iter().take(visible) {
            list.append(&search_collection_button(card, event_tx));
        }
    }
    section.append(&list);

    if total > visible {
        section.append(&search_more_button(
            category,
            total - visible,
            limit,
            event_tx,
        ));
    }
    section
}

fn search_collection_button(card: HomeCard, event_tx: &Sender<BrowserEvent>) -> gtk::Button {
    let (cover_path, icon_name, title, subtitle, detail, online) = match &card {
        HomeCard::LocalAlbum {
            title,
            subtitle,
            detail,
            cover_path,
        } => (
            cover_path.as_deref(),
            "media-optical-symbolic",
            title.as_str(),
            subtitle.as_str(),
            detail.as_str(),
            false,
        ),
        HomeCard::YouTubeAlbum {
            item,
            subtitle,
            detail,
            cover_path,
        } => (
            cover_path.as_deref(),
            "media-optical-symbolic",
            item.title.as_str(),
            subtitle.as_str(),
            detail.as_str(),
            true,
        ),
        HomeCard::LocalArtist {
            title,
            subtitle,
            detail,
            cover_path,
        } => (
            cover_path.as_deref(),
            "avatar-default-symbolic",
            title.as_str(),
            subtitle.as_str(),
            detail.as_str(),
            false,
        ),
        HomeCard::YouTubeArtist {
            item,
            subtitle,
            detail,
            cover_path,
        } => (
            cover_path.as_deref(),
            "avatar-default-symbolic",
            item.title.as_str(),
            subtitle.as_str(),
            detail.as_str(),
            true,
        ),
        HomeCard::LocalPlaylist { title, subtitle } => (
            None,
            "view-list-symbolic",
            title.as_str(),
            subtitle.as_str(),
            "Playlist local",
            false,
        ),
        HomeCard::YouTubePlaylist(item) => (
            item.cached_cover(),
            "view-list-symbolic",
            item.title.as_str(),
            youtube_playlist_subtitle(item),
            youtube_playlist_detail(item),
            true,
        ),
    };

    let leading = search_result_artwork(cover_path, icon_name);

    let title_label = gtk::Label::new(Some(title));
    title_label.set_xalign(0.0);
    title_label.set_hexpand(true);
    title_label.set_single_line_mode(true);
    title_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title_label.add_css_class("heading");

    let secondary = if subtitle.is_empty() {
        detail
    } else {
        subtitle
    };
    let subtitle_label = gtk::Label::new(Some(secondary));
    subtitle_label.set_xalign(0.0);
    subtitle_label.set_single_line_mode(true);
    subtitle_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    subtitle_label.add_css_class("dim-label");

    let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
    text.set_hexpand(true);
    text.append(&title_label);
    text.append(&subtitle_label);

    let source = gtk::Label::new(Some(if online { "YouTube" } else { "Local" }));
    source.add_css_class("pill");
    source.add_css_class("search-source-badge");

    let arrow = gtk::Image::from_icon_name("go-next-symbolic");
    arrow.set_pixel_size(16);
    arrow.add_css_class("dim-label");
    arrow.add_css_class("search-result-arrow");

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.add_css_class("search-result-row");
    row.set_margin_top(8);
    row.set_margin_bottom(8);
    row.set_margin_start(10);
    row.set_margin_end(10);
    row.append(&leading);
    row.append(&text);
    row.append(&source);
    row.append(&arrow);

    let button = gtk::Button::new();
    button.set_child(Some(&row));
    button.set_hexpand(true);
    button.set_halign(gtk::Align::Fill);
    button.add_css_class("flat");
    button.add_css_class("search-result-button");

    let sender = event_tx.clone();
    button.connect_clicked(move |_| {
        let event = match card.clone() {
            HomeCard::LocalAlbum { title, .. } => {
                BrowserEvent::Navigate(BrowserRoute::Album(title))
            }
            HomeCard::YouTubeAlbum { item, .. } => BrowserEvent::OpenYouTubeCollection(item),
            HomeCard::LocalArtist { title, .. } => {
                BrowserEvent::Navigate(BrowserRoute::Artist(title))
            }
            HomeCard::YouTubeArtist { item, .. } => BrowserEvent::OpenYouTubeCollection(item),
            HomeCard::LocalPlaylist { title, .. } => {
                BrowserEvent::Navigate(BrowserRoute::Playlist(title))
            }
            HomeCard::YouTubePlaylist(item) => BrowserEvent::OpenYouTubePlaylist(item),
        };
        let _ = sender.send(event);
    });

    button
}

fn search_result_artwork(cover_path: Option<&Path>, icon_name: &str) -> gtk::Stack {
    let placeholder = gtk::Image::from_icon_name(icon_name);
    placeholder.set_pixel_size(24);
    placeholder.set_halign(gtk::Align::Center);
    placeholder.set_valign(gtk::Align::Center);
    placeholder.add_css_class("cover-icon");

    let picture = gtk::Picture::new();
    picture.set_content_fit(gtk::ContentFit::Cover);
    picture.set_size_request(48, 48);

    let stack = gtk::Stack::new();
    stack.set_size_request(48, 48);
    stack.set_overflow(gtk::Overflow::Hidden);
    stack.add_named(&placeholder, Some("placeholder"));
    stack.add_named(&picture, Some("picture"));
    stack.add_css_class("collection-artwork");

    if let Some(path) = cover_path.filter(|path| path.is_file()) {
        if let Some(texture) = cached_square_texture(path, 48) {
            picture.set_paintable(Some(&texture));
            stack.set_visible_child_name("picture");
        } else {
            stack.set_visible_child_name("placeholder");
        }
    } else {
        stack.set_visible_child_name("placeholder");
    }

    stack
}

fn home_history_section(
    title: &str,
    subtitle: &str,
    entries: Vec<HomeHistoryTrack>,
    event_tx: &Sender<BrowserEvent>,
    language: AppLanguage,
    card_effects: bool,
) -> gtk::Box {
    let title_label = gtk::Label::new(Some(title));
    title_label.set_xalign(0.0);
    title_label.add_css_class("home-section-title");

    let subtitle_label = gtk::Label::new(Some(subtitle));
    subtitle_label.set_xalign(0.0);
    subtitle_label.add_css_class("dim-label");

    let heading = gtk::Box::new(gtk::Orientation::Vertical, 2);
    heading.add_css_class("home-section-heading");
    heading.append(&title_label);
    heading.append(&subtitle_label);

    let rail = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    rail.add_css_class("home-carousel");

    for entry in entries {
        let (card, event) = match entry {
            HomeHistoryTrack::LocalTrack {
                index,
                track,
                position_seconds,
                duration_seconds,
                completed,
            } => {
                let resumable = !completed
                    && duration_seconds > 0
                    && position_seconds >= 30
                    && position_seconds.saturating_mul(100) <= duration_seconds.saturating_mul(90);
                let detail = if resumable {
                    format_history_position(language, position_seconds)
                } else {
                    recently_played_detail(language, false)
                };
                let event = if resumable {
                    BrowserEvent::ResumeLocalTrack {
                        index,
                        position_seconds,
                    }
                } else {
                    BrowserEvent::TrackActivated(index)
                };
                (
                    collection_card(
                        track.cover_path.as_deref(),
                        &track.title,
                        &track.artist,
                        &detail,
                        false,
                    ),
                    event,
                )
            }
            HomeHistoryTrack::YouTubeTrack {
                item,
                position_seconds,
                duration_seconds,
                completed,
            } => {
                let resumable = !completed
                    && duration_seconds > 0
                    && position_seconds >= 30
                    && position_seconds.saturating_mul(100) <= duration_seconds.saturating_mul(90);
                let detail = if resumable {
                    format_history_position(language, position_seconds)
                } else {
                    recently_played_detail(language, true)
                };
                let event = if resumable {
                    BrowserEvent::ResumeYouTubeTrack {
                        item: item.clone(),
                        position_seconds,
                    }
                } else {
                    BrowserEvent::YouTubeTrackActivated {
                        item: item.clone(),
                        queue: vec![item.clone()],
                        index: 0,
                    }
                };
                (
                    collection_card(
                        item.cached_cover(),
                        &item.title,
                        &item.artist,
                        &detail,
                        true,
                    ),
                    event,
                )
            }
            HomeHistoryTrack::LocalAlbum(title) => (
                collection_card(
                    None,
                    &title,
                    match language {
                        AppLanguage::Portuguese => "Álbum ouvido recentemente",
                        AppLanguage::English => "Recently listened album",
                        AppLanguage::Spanish => "Álbum escuchado recientemente",
                    },
                    "Local",
                    false,
                ),
                BrowserEvent::Navigate(BrowserRoute::Album(title)),
            ),
            HomeHistoryTrack::LocalPlaylist(title) => (
                collection_card(
                    None,
                    &title,
                    match language {
                        AppLanguage::Portuguese => "Playlist ouvida recentemente",
                        AppLanguage::English => "Recently listened playlist",
                        AppLanguage::Spanish => "Playlist escuchada recientemente",
                    },
                    "Local",
                    false,
                ),
                BrowserEvent::Navigate(BrowserRoute::Playlist(title)),
            ),
            HomeHistoryTrack::YouTubeAlbum { item, cover_path } => (
                collection_card(
                    cover_path.as_deref().or_else(|| item.cached_cover()),
                    &item.title,
                    match language {
                        AppLanguage::Portuguese => "Álbum ouvido recentemente",
                        AppLanguage::English => "Recently listened album",
                        AppLanguage::Spanish => "Álbum escuchado recientemente",
                    },
                    "YouTube Music",
                    true,
                ),
                BrowserEvent::OpenYouTubeCollection(item),
            ),
            HomeHistoryTrack::YouTubePlaylist(item) => (
                collection_card(
                    item.cached_cover(),
                    &item.title,
                    match language {
                        AppLanguage::Portuguese => "Playlist ouvida recentemente",
                        AppLanguage::English => "Recently listened playlist",
                        AppLanguage::Spanish => "Playlist escuchada recientemente",
                    },
                    "YouTube Music",
                    true,
                ),
                BrowserEvent::OpenYouTubePlaylist(item),
            ),
        };

        let button = collection_event_button(card, event, event_tx);
        button.add_css_class("home-card-button");
        rail.append(&button);
    }

    let item_count = rail.observe_children().n_items();
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(
        if item_count > 1 {
            gtk::PolicyType::Always
        } else {
            gtk::PolicyType::Never
        },
        gtk::PolicyType::Never,
    );
    scroll.set_min_content_height(190);
    scroll.set_child(Some(&rail));
    scroll.add_css_class("home-carousel-scroll");
    scroll.add_css_class("material-carousel-scroll");
    scroll.set_overlay_scrolling(false);

    if item_count > 1 {
        install_home_carousel_edge_spring(&scroll, &rail, card_effects);
    }

    let section = gtk::Box::new(gtk::Orientation::Vertical, 10);
    section.add_css_class("home-section");
    section.append(&heading);
    section.append(&scroll);
    section
}

#[expect(
    clippy::too_many_arguments,
    reason = "Home section rendering keeps its source-aware dependencies explicit"
)]
fn home_section(
    title: &str,
    subtitle: &str,
    cards: Vec<HomeCard>,
    playback: &BrowserPlaybackState,
    config: &AppConfig,
    event_tx: &Sender<BrowserEvent>,
    language: AppLanguage,
    card_effects: bool,
) -> gtk::Box {
    let title_label = gtk::Label::new(Some(title));
    title_label.set_xalign(0.0);
    title_label.add_css_class("home-section-title");
    let subtitle_label = gtk::Label::new(Some(subtitle));
    subtitle_label.set_xalign(0.0);
    subtitle_label.add_css_class("dim-label");

    let heading = gtk::Box::new(gtk::Orientation::Vertical, 2);
    heading.add_css_class("home-section-heading");
    heading.append(&title_label);
    heading.append(&subtitle_label);

    let rail = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    rail.add_css_class("home-carousel");

    if cards.is_empty() {
        rail.append(&home_empty_card(language));
    } else {
        for card in cards {
            rail.append(&home_card_button(
                card,
                playback,
                config,
                event_tx,
                language,
                card_effects,
            ));
        }
    }

    let scroll = gtk::ScrolledWindow::new();
    // functional_carousel_queue_blur_fix_v1
    // Keep the thin Material position indicator visible whenever the
    // carousel is presented, instead of relying on GTK auto-hide.
    scroll.set_policy(gtk::PolicyType::Always, gtk::PolicyType::Never);
    scroll.set_min_content_height(190);
    scroll.set_child(Some(&rail));
    scroll.add_css_class("home-carousel-scroll");
    scroll.add_css_class("material-carousel-scroll");
    scroll.set_overlay_scrolling(false);

    // nocky_home_cards_edge_spring_v4
    // nocky_home_cards_edge_spring_v6
    install_home_carousel_edge_spring(&scroll, &rail, card_effects);

    let section = gtk::Box::new(gtk::Orientation::Vertical, 10);
    section.add_css_class("home-section");
    section.append(&heading);
    section.append(&scroll);
    section
}

// nocky_home_cards_edge_spring_v6: reliable bounded carousel edge spring

#[derive(Clone)]
struct HomeCarouselEdgeCard {
    button: gtk::Widget,
    surface: Option<gtk::Widget>,
    original_button_width_request: i32,
    original_surface_width_request: Option<i32>,
    base_button_width: i32,
    base_surface_width: i32,
}

type HomeCarouselEdgeCards = Rc<RefCell<Vec<HomeCarouselEdgeCard>>>;

fn home_card_surface(widget: &gtk::Widget) -> Option<gtk::Widget> {
    let first = widget.first_child()?;
    if first.has_css_class("collection-card") {
        return Some(first);
    }

    let second = first.first_child()?;
    if second.has_css_class("collection-card") {
        return Some(second);
    }

    let third = second.first_child()?;
    third.has_css_class("collection-card").then_some(third)
}

fn install_home_carousel_edge_spring(scroll: &gtk::ScrolledWindow, rail: &gtk::Box, enabled: bool) {
    if !enabled {
        return;
    }

    scroll.set_kinetic_scrolling(true);

    let ready = Rc::new(Cell::new(false));
    let active = Rc::new(Cell::new(false));
    let generation = Rc::new(Cell::new(0_u64));
    let active_cards: HomeCarouselEdgeCards = Rc::new(RefCell::new(Vec::new()));

    let trigger: Rc<dyn Fn(gtk::PositionType)> = {
        let scroll_weak = scroll.downgrade();
        let rail_weak = rail.downgrade();
        let ready = ready.clone();
        let active = active.clone();
        let generation = generation.clone();
        let active_cards = active_cards.clone();

        Rc::new(move |position| {
            if !ready.get() || active.replace(true) {
                return;
            }

            let from_start = match position {
                gtk::PositionType::Left => true,
                gtk::PositionType::Right => false,
                _ => {
                    active.set(false);
                    return;
                }
            };

            let Some(scroll) = scroll_weak.upgrade() else {
                active.set(false);
                return;
            };
            let Some(rail) = rail_weak.upgrade() else {
                active.set(false);
                return;
            };

            let cards = home_carousel_edge_cards(&rail, from_start, 3);
            if cards.is_empty() {
                active.set(false);
                return;
            }

            let token = generation.get().wrapping_add(1);
            generation.set(token);

            {
                let mut stored = active_cards.borrow_mut();
                stored.clear();

                for button in cards {
                    let surface = home_card_surface(&button);

                    let base_button_width = button.width().max(COLLECTION_CARD_MAX_WIDTH);
                    let base_surface_width = surface
                        .as_ref()
                        .map(|surface| surface.width())
                        .unwrap_or(base_button_width)
                        .max(COLLECTION_CARD_MAX_WIDTH);

                    button.add_css_class("home-card-edge-spring");
                    if let Some(surface) = surface.as_ref() {
                        surface.add_css_class("home-card-edge-spring-surface");
                    }

                    stored.push(HomeCarouselEdgeCard {
                        button: button.clone(),
                        surface: surface.clone(),
                        original_button_width_request: button.width_request(),
                        original_surface_width_request: surface
                            .as_ref()
                            .map(|surface| surface.width_request()),
                        base_button_width,
                        base_surface_width,
                    });
                }
            }

            let started_at = Rc::new(Cell::new(0_i64));
            let active = active.clone();
            let generation = generation.clone();
            let active_cards = active_cards.clone();

            scroll.add_tick_callback(move |scroll, frame_clock| {
                if generation.get() != token {
                    restore_home_carousel_edge_cards(&active_cards);
                    active.set(false);
                    return glib::ControlFlow::Break;
                }

                let now = frame_clock.frame_time();
                let start = started_at.get();

                if start == 0 {
                    started_at.set(now);
                    return glib::ControlFlow::Continue;
                }

                let progress = ((now - start) as f64 / 520_000.0).clamp(0.0, 1.0);
                let displacement = home_carousel_spring_displacement(progress);
                let strengths: [f64; 3] = [1.0, 0.60, 0.32];

                {
                    let stored = active_cards.borrow();
                    for (index, card) in stored.iter().enumerate() {
                        let stretch = (displacement * strengths[index.min(2)]).round() as i32;

                        card.button.set_width_request(
                            (card.base_button_width + stretch).max(COLLECTION_CARD_MIN_WIDTH),
                        );

                        if let Some(surface) = card.surface.as_ref() {
                            surface.set_width_request(
                                (card.base_surface_width + stretch).max(COLLECTION_CARD_MIN_WIDTH),
                            );
                        }
                    }
                }

                if !from_start {
                    let adjustment = scroll.hadjustment();
                    let end = (adjustment.upper() - adjustment.page_size()).max(adjustment.lower());
                    adjustment.set_value(end);
                }

                if progress >= 1.0 {
                    restore_home_carousel_edge_cards(&active_cards);
                    active.set(false);
                    glib::ControlFlow::Break
                } else {
                    glib::ControlFlow::Continue
                }
            });
        })
    };

    {
        let ready = ready.clone();
        scroll.connect_map(move |scroll| {
            ready.set(false);
            let ready = ready.clone();
            let weak_scroll = scroll.downgrade();

            glib::timeout_add_local_once(Duration::from_millis(180), move || {
                if weak_scroll.upgrade().is_some() {
                    ready.set(true);
                }
            });
        });
    }

    {
        let trigger = trigger.clone();
        scroll.connect_edge_reached(move |_, position| {
            trigger(position);
        });
    }

    {
        let trigger = trigger.clone();
        scroll.connect_edge_overshot(move |_, position| {
            trigger(position);
        });
    }

    {
        let adjustment = scroll.hadjustment();
        let last_value = Rc::new(Cell::new(adjustment.value()));
        let ready = ready.clone();
        let trigger = trigger.clone();

        adjustment.connect_value_changed(move |adjustment| {
            let value = adjustment.value();
            let previous = last_value.replace(value);

            if !ready.get() {
                return;
            }

            let lower = adjustment.lower();
            let upper = (adjustment.upper() - adjustment.page_size()).max(lower);
            const EDGE_EPSILON: f64 = 0.75;

            if value <= lower + EDGE_EPSILON && previous > value + EDGE_EPSILON {
                trigger(gtk::PositionType::Left);
            } else if value >= upper - EDGE_EPSILON && previous < value - EDGE_EPSILON {
                trigger(gtk::PositionType::Right);
            }
        });
    }

    {
        let ready = ready.clone();
        let active = active.clone();
        let generation = generation.clone();
        let active_cards = active_cards.clone();

        scroll.connect_unmap(move |_| {
            ready.set(false);
            generation.set(generation.get().wrapping_add(1));
            restore_home_carousel_edge_cards(&active_cards);
            active.set(false);
        });
    }
}

fn home_carousel_edge_cards(rail: &gtk::Box, from_start: bool, limit: usize) -> Vec<gtk::Widget> {
    let mut cards = Vec::new();
    let mut current = if from_start {
        rail.first_child()
    } else {
        rail.last_child()
    };

    while let Some(card) = current {
        let next = if from_start {
            card.next_sibling()
        } else {
            card.prev_sibling()
        };

        cards.push(card);

        if cards.len() == limit {
            break;
        }

        current = next;
    }

    cards
}

fn restore_home_carousel_edge_cards(cards: &HomeCarouselEdgeCards) {
    for card in cards.borrow_mut().drain(..) {
        card.button
            .set_width_request(card.original_button_width_request);
        card.button.remove_css_class("home-card-edge-spring");

        if let Some(surface) = card.surface {
            surface.set_width_request(card.original_surface_width_request.unwrap_or(-1));
            surface.remove_css_class("home-card-edge-spring-surface");
        }
    }
}

fn home_carousel_spring_displacement(progress: f64) -> f64 {
    if progress < 0.20 {
        24.0 * home_edge_ease_out_cubic(progress / 0.20)
    } else if progress < 0.48 {
        home_edge_lerp(
            24.0,
            -7.0,
            home_edge_ease_in_out_cubic((progress - 0.20) / 0.28),
        )
    } else if progress < 0.73 {
        home_edge_lerp(
            -7.0,
            4.0,
            home_edge_ease_in_out_cubic((progress - 0.48) / 0.25),
        )
    } else {
        home_edge_lerp(4.0, 0.0, home_edge_ease_out_cubic((progress - 0.73) / 0.27))
    }
}

fn home_edge_ease_out_cubic(value: f64) -> f64 {
    1.0 - (1.0 - value.clamp(0.0, 1.0)).powi(3)
}

fn home_edge_ease_in_out_cubic(value: f64) -> f64 {
    let value = value.clamp(0.0, 1.0);

    if value < 0.5 {
        4.0 * value.powi(3)
    } else {
        1.0 - (-2.0 * value + 2.0).powi(3) / 2.0
    }
}

fn home_edge_lerp(start: f64, end: f64, progress: f64) -> f64 {
    start + (end - start) * progress
}

fn home_card_button(
    card: HomeCard,
    playback: &BrowserPlaybackState,
    config: &AppConfig,
    event_tx: &Sender<BrowserEvent>,
    language: AppLanguage,
    card_effects: bool,
) -> gtk::Widget {
    let descriptor = card.descriptor(language);
    let open_event = card.open_event();

    let (play_event, queue_events, collection_kind, collection_id, collection_title) = match &card {
        HomeCard::LocalAlbum { title, .. } => (
            Some(BrowserEvent::PlayLocalAlbum(title.clone())),
            Some((
                BrowserEvent::QueueLocalCollection {
                    kind: "album".to_string(),
                    title: title.clone(),
                    play_next: true,
                },
                BrowserEvent::QueueLocalCollection {
                    kind: "album".to_string(),
                    title: title.clone(),
                    play_next: false,
                },
            )),
            "album",
            title.to_lowercase(),
            title.clone(),
        ),
        HomeCard::LocalPlaylist { title, .. } => (
            Some(BrowserEvent::PlayLocalPlaylist(title.clone())),
            Some((
                BrowserEvent::QueueLocalCollection {
                    kind: "playlist".to_string(),
                    title: title.clone(),
                    play_next: true,
                },
                BrowserEvent::QueueLocalCollection {
                    kind: "playlist".to_string(),
                    title: title.clone(),
                    play_next: false,
                },
            )),
            "playlist",
            title.to_lowercase(),
            title.clone(),
        ),
        HomeCard::YouTubeAlbum { item, .. } => (
            Some(BrowserEvent::PlayYouTubeAlbum(item.clone())),
            Some((
                BrowserEvent::QueueYouTubeCollection {
                    item: item.clone(),
                    playlist: false,
                    play_next: true,
                },
                BrowserEvent::QueueYouTubeCollection {
                    item: item.clone(),
                    playlist: false,
                    play_next: false,
                },
            )),
            "album",
            if item.browse_id.trim().is_empty() {
                item.title.to_lowercase()
            } else {
                item.browse_id.clone()
            },
            item.title.clone(),
        ),
        HomeCard::YouTubePlaylist(item) => (
            Some(BrowserEvent::PlayYouTubePlaylist(item.clone())),
            Some((
                BrowserEvent::QueueYouTubeCollection {
                    item: item.clone(),
                    playlist: true,
                    play_next: true,
                },
                BrowserEvent::QueueYouTubeCollection {
                    item: item.clone(),
                    playlist: true,
                    play_next: false,
                },
            )),
            "playlist",
            if item.browse_id.trim().is_empty() {
                item.title.to_lowercase()
            } else {
                item.browse_id.clone()
            },
            item.title.clone(),
        ),
        HomeCard::LocalArtist { .. } | HomeCard::YouTubeArtist { .. } => {
            (None, None, "", String::new(), String::new())
        }
    };

    let is_active = play_event.is_some()
        && playback.matches_collection(collection_kind, &collection_id, &collection_title);
    let is_loading = play_event.is_some()
        && playback.collection_is_loading(collection_kind, &collection_id, &collection_title);
    let inline_loading_on_click = !is_active
        && matches!(
            &card,
            HomeCard::YouTubeAlbum { .. } | HomeCard::YouTubePlaylist(_)
        );

    let card_widget = collection_card_with_placeholder(
        descriptor.cover_path,
        descriptor.title,
        descriptor.subtitle,
        descriptor.detail,
        descriptor.online,
        descriptor.placeholder_icon,
        descriptor.placeholder_class,
    );
    if descriptor.artist {
        card_widget.add_css_class("artist-collection-card");
    }

    card_widget.add_css_class("home-card");
    card_widget.add_css_class("expressive-collection-card");
    if is_active {
        card_widget.add_css_class("collection-card-playing");
    }
    if is_loading {
        card_widget.add_css_class("collection-card-loading");
        card_widget.add_css_class("collection-card-skeleton");

        if let Some(artwork) = card_widget
            .first_child()
            .and_then(|child| child.downcast::<gtk::Stack>().ok())
        {
            artwork.set_opacity(0.58);
            artwork.add_css_class("collection-card-skeleton-artwork");
        }

        let mut child = card_widget
            .first_child()
            .and_then(|widget| widget.next_sibling());
        while let Some(widget) = child {
            widget.set_opacity(0.52);
            widget.add_css_class("collection-card-skeleton-line");
            child = widget.next_sibling();
        }
    }

    let main_button = gtk::Button::new();
    main_button.add_css_class("flat");
    main_button.add_css_class("home-card-button");
    main_button.add_css_class("expressive-collection-button");
    if card_effects {
        main_button.add_css_class("home-card-motion-requested");
    }
    main_button.add_css_class("home-card-no-hover-scale");
    main_button.set_child(Some(&card_widget));

    {
        let sender = event_tx.clone();
        let event = open_event.clone();
        main_button.connect_clicked(move |_| {
            if card_effects {
                let sender = sender.clone();
                let event = event.clone();
                glib::timeout_add_local_once(Duration::from_millis(120), move || {
                    let _ = sender.send(event);
                });
            } else {
                let _ = sender.send(event.clone());
            }
        });
    }

    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&main_button));
    overlay.add_css_class("home-card-context-overlay");

    if let Some(play_event) = play_event {
        let control = gtk::Button::new();
        control.set_halign(gtk::Align::End);
        control.set_valign(gtk::Align::Start);
        control.set_margin_top(12);
        control.set_margin_end(12);
        control.add_css_class("circular");
        control.add_css_class("collection-card-context-action");

        if is_loading {
            let spinner = gtk::Spinner::new();
            spinner.set_spinning(true);
            spinner.set_size_request(18, 18);
            control.set_child(Some(&spinner));
            control.set_sensitive(false);
            control.add_css_class("loading");
            control.set_tooltip_text(Some(match language {
                AppLanguage::Portuguese => "Carregando coleção…",
                AppLanguage::English => "Loading collection…",
                AppLanguage::Spanish => "Cargando colección…",
            }));
        } else {
            let control_event = if is_active {
                BrowserEvent::TogglePlayback
            } else {
                play_event
            };
            let icon_name = if is_active && playback.playing {
                "media-playback-pause-symbolic"
            } else {
                "media-playback-start-symbolic"
            };
            let tooltip = match (language, is_active, playback.playing) {
                (AppLanguage::Portuguese, true, true) => "Pausar coleção",
                (AppLanguage::Portuguese, true, false) => "Continuar coleção",
                (AppLanguage::Portuguese, false, _) => "Reproduzir coleção",
                (AppLanguage::English, true, true) => "Pause collection",
                (AppLanguage::English, true, false) => "Resume collection",
                (AppLanguage::English, false, _) => "Play collection",
                (AppLanguage::Spanish, true, true) => "Pausar colección",
                (AppLanguage::Spanish, true, false) => "Continuar colección",
                (AppLanguage::Spanish, false, _) => "Reproducir colección",
            };

            control.set_icon_name(icon_name);
            control.set_tooltip_text(Some(tooltip));
            if is_active {
                control.add_css_class("active");
            }

            let sender = event_tx.clone();
            control.connect_clicked(move |button| {
                if inline_loading_on_click {
                    let spinner = gtk::Spinner::new();
                    spinner.set_spinning(true);
                    spinner.set_size_request(18, 18);
                    button.set_child(Some(&spinner));
                    button.set_sensitive(false);
                    button.add_css_class("loading");
                    button.set_tooltip_text(Some(match language {
                        AppLanguage::Portuguese => "Carregando coleção…",
                        AppLanguage::English => "Loading collection…",
                        AppLanguage::Spanish => "Cargando colección…",
                    }));
                }

                let _ = sender.send(control_event.clone());
            });
        }

        overlay.add_overlay(&control);
    }

    if let Some((play_next_event, append_event)) = queue_events {
        let menu_button = gtk::MenuButton::builder()
            .icon_name("view-more-symbolic")
            .tooltip_text(match language {
                AppLanguage::Portuguese => "Mais opções",
                AppLanguage::English => "More options",
                AppLanguage::Spanish => "Más opciones",
            })
            .build();
        menu_button.set_halign(gtk::Align::Start);
        menu_button.set_valign(gtk::Align::Start);
        menu_button.set_margin_top(12);
        menu_button.set_margin_start(12);
        menu_button.add_css_class("circular");
        menu_button.add_css_class("collection-card-overflow-button");
        menu_button.set_sensitive(!is_loading);

        let popover = gtk::Popover::new();
        popover.add_css_class("collection-card-overflow-popover");

        let actions = gtk::Box::new(gtk::Orientation::Vertical, 4);
        actions.set_margin_top(8);
        actions.set_margin_bottom(8);
        actions.set_margin_start(8);
        actions.set_margin_end(8);

        let is_favorite = config.is_collection_favorite(&card.identity());
        let labels = match language {
            AppLanguage::Portuguese => (
                "Reproduzir em seguida",
                "Adicionar ao fim da fila",
                "Abrir coleção",
                if is_favorite {
                    "Remover dos favoritos"
                } else {
                    "Adicionar aos favoritos"
                },
            ),
            AppLanguage::English => (
                "Play next",
                "Add to queue",
                "Open collection",
                if is_favorite {
                    "Remove from favorites"
                } else {
                    "Add to favorites"
                },
            ),
            AppLanguage::Spanish => (
                "Reproducir a continuación",
                "Añadir al final de la cola",
                "Abrir colección",
                if is_favorite {
                    "Quitar de favoritos"
                } else {
                    "Añadir a favoritos"
                },
            ),
        };

        let favorite_event = BrowserEvent::ToggleCollectionFavorite(card.identity());
        for (label, event, icon_name) in [
            (labels.0, play_next_event, "media-skip-forward-symbolic"),
            (labels.1, append_event, "list-add-symbolic"),
            (labels.2, open_event, "go-next-symbolic"),
            (
                labels.3,
                favorite_event,
                if is_favorite {
                    "emblem-favorite-symbolic"
                } else {
                    "non-starred-symbolic"
                },
            ),
        ] {
            let icon = gtk::Image::from_icon_name(icon_name);
            icon.set_pixel_size(18);
            icon.add_css_class("collection-card-overflow-action-icon");

            let text = gtk::Label::new(Some(label));
            text.set_xalign(0.0);
            text.set_hexpand(true);
            text.add_css_class("collection-card-overflow-action-label");

            let content = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            content.set_hexpand(true);
            content.set_halign(gtk::Align::Fill);
            content.append(&icon);
            content.append(&text);

            let button = gtk::Button::new();
            button.set_child(Some(&content));
            button.set_halign(gtk::Align::Fill);
            button.set_hexpand(true);
            button.add_css_class("flat");
            button.add_css_class("collection-card-overflow-action");

            let sender = event_tx.clone();
            let popover = popover.clone();
            button.connect_clicked(move |_| {
                popover.popdown();
                let _ = sender.send(event.clone());
            });
            actions.append(&button);
        }

        popover.set_child(Some(&actions));
        menu_button.set_popover(Some(&popover));
        overlay.add_overlay(&menu_button);
    }

    overlay.upcast()
}

fn home_empty_card(language: AppLanguage) -> gtk::Box {
    let text = home_copy(language);
    collection_card(None, text.waiting_content, text.empty_hint, "", false)
}

fn home_syncing_hint(language: AppLanguage) -> gtk::Box {
    let label = gtk::Label::new(Some(home_copy(language).syncing_library));
    label.set_xalign(0.0);
    label.add_css_class("dim-label");
    let row = gtk::Box::new(gtk::Orientation::Vertical, 0);
    row.add_css_class("home-syncing-hint");
    row.append(&label);
    row
}

fn compare_library_tracks(left: &Track, right: &Track) -> Ordering {
    compare_text(&left.artist, &right.artist)
        .then_with(|| compare_text(&left.album, &right.album))
        .then_with(|| compare_album_tracks(left, right))
}

fn compare_artist_tracks(left: &Track, right: &Track) -> Ordering {
    compare_text(&left.album, &right.album).then_with(|| compare_album_tracks(left, right))
}

fn compare_album_tracks(left: &Track, right: &Track) -> Ordering {
    left.disc_number
        .unwrap_or(u32::MAX)
        .cmp(&right.disc_number.unwrap_or(u32::MAX))
        .then_with(|| {
            left.track_number
                .unwrap_or(u32::MAX)
                .cmp(&right.track_number.unwrap_or(u32::MAX))
        })
        .then_with(|| compare_text(&left.title, &right.title))
        .then_with(|| {
            left.path
                .to_string_lossy()
                .to_lowercase()
                .cmp(&right.path.to_string_lossy().to_lowercase())
        })
}

fn compare_youtube_items(left: &YouTubeItem, right: &YouTubeItem) -> Ordering {
    compare_text(&left.artist, &right.artist)
        .then_with(|| compare_text(&left.album, &right.album))
        .then_with(|| compare_text(&left.title, &right.title))
        .then_with(|| left.video_id.cmp(&right.video_id))
}

fn compare_text(left: &str, right: &str) -> Ordering {
    left.to_lowercase().cmp(&right.to_lowercase())
}

const COLLECTION_CARD_MIN_WIDTH: i32 = 156;
const COLLECTION_CARD_MAX_WIDTH: i32 = 220;
const COLLECTION_CARD_MIN_HEIGHT: i32 = 210;
const COLLECTION_ARTWORK_MIN_SIZE: i32 = 124;
const COLLECTION_ARTWORK_MAX_SIZE: i32 = 216;
const COLLECTION_INITIAL_BATCH: usize = 48;
const COLLECTION_BATCH_INCREMENT: usize = 48;

fn artist_list_grid() -> gtk::FlowBox {
    let list = gtk::FlowBox::new();
    list.set_column_spacing(0);
    list.set_row_spacing(4);
    list.set_min_children_per_line(1);
    list.set_max_children_per_line(1);
    list.set_homogeneous(false);
    list.set_selection_mode(gtk::SelectionMode::None);
    list.set_halign(gtk::Align::Fill);
    list.set_valign(gtk::Align::Start);
    list.set_hexpand(true);
    list.add_css_class("artist-list");
    list
}

fn artist_list_button(
    artist: &str,
    event: BrowserEvent,
    event_tx: &Sender<BrowserEvent>,
) -> gtk::Button {
    let name = gtk::Label::new(Some(artist));
    name.set_xalign(0.0);
    name.set_hexpand(true);
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);

    let arrow = gtk::Image::from_icon_name("go-next-symbolic");
    arrow.set_pixel_size(16);
    arrow.add_css_class("dim-label");

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.set_margin_start(14);
    row.set_margin_end(14);
    row.set_margin_top(10);
    row.set_margin_bottom(10);
    row.append(&name);
    row.append(&arrow);

    let button = gtk::Button::new();
    button.set_child(Some(&row));
    button.set_hexpand(true);
    button.set_halign(gtk::Align::Fill);
    button.add_css_class("flat");
    button.add_css_class("artist-list-button");
    button.add_css_class("expressive-list-card");

    let sender = event_tx.clone();
    button.connect_clicked(move |_| {
        let _ = sender.send(event.clone());
    });

    button
}

fn artist_list_placeholder(message: &str) -> gtk::Button {
    let label = gtk::Label::new(Some(message));
    label.set_xalign(0.0);
    label.set_hexpand(true);
    label.set_margin_start(14);
    label.set_margin_end(14);
    label.set_margin_top(12);
    label.set_margin_bottom(12);
    label.add_css_class("dim-label");

    let button = gtk::Button::new();
    button.set_child(Some(&label));
    button.set_hexpand(true);
    button.set_sensitive(false);
    button.add_css_class("flat");
    button.add_css_class("artist-list-button");
    button
}

fn collection_grid() -> gtk::FlowBox {
    let grid = gtk::FlowBox::new();
    grid.set_column_spacing(14);
    grid.set_row_spacing(18);
    grid.set_min_children_per_line(2);
    grid.set_max_children_per_line(8);
    grid.set_homogeneous(true);
    grid.set_selection_mode(gtk::SelectionMode::None);
    grid.set_halign(gtk::Align::Start);
    grid.set_valign(gtk::Align::Start);
    grid.set_hexpand(true);
    grid.add_css_class("collection-grid");
    grid
}

fn install_vertical_edge_spring(scroll: &gtk::ScrolledWindow) {
    scroll.set_kinetic_scrolling(true);

    let ready = Rc::new(Cell::new(false));
    let active = Rc::new(Cell::new(false));
    let internal_update = Rc::new(Cell::new(false));
    let generation = Rc::new(Cell::new(0_u64));

    let trigger: Rc<dyn Fn(gtk::PositionType)> = {
        let scroll_weak = scroll.downgrade();
        let ready = ready.clone();
        let active = active.clone();
        let internal_update = internal_update.clone();
        let generation = generation.clone();

        Rc::new(move |position| {
            if !ready.get() || active.replace(true) {
                return;
            }

            let from_top = match position {
                gtk::PositionType::Top => true,
                gtk::PositionType::Bottom => false,
                _ => {
                    active.set(false);
                    return;
                }
            };

            let Some(scroll) = scroll_weak.upgrade() else {
                active.set(false);
                return;
            };

            let adjustment = scroll.vadjustment();
            let lower = adjustment.lower();
            let upper = (adjustment.upper() - adjustment.page_size()).max(lower);
            if upper <= lower + 1.0 {
                active.set(false);
                return;
            }

            let token = generation.get().wrapping_add(1);
            generation.set(token);

            let started_at = Rc::new(Cell::new(0_i64));
            let active = active.clone();
            let internal_update = internal_update.clone();
            let generation = generation.clone();

            scroll.add_tick_callback(move |_, frame_clock| {
                if generation.get() != token {
                    active.set(false);
                    return glib::ControlFlow::Break;
                }

                let now = frame_clock.frame_time();
                let start = started_at.get();

                if start == 0 {
                    started_at.set(now);
                    return glib::ControlFlow::Continue;
                }

                // Exactly the same duration and displacement curve used by
                // the Home carousel edge spring.
                let progress = ((now - start) as f64 / 520_000.0).clamp(0.0, 1.0);
                let displacement = home_carousel_spring_displacement(progress);

                let lower = adjustment.lower();
                let upper = (adjustment.upper() - adjustment.page_size()).max(lower);

                // A GtkAdjustment cannot move outside its valid range, so the
                // negative overshoot phase rests at the edge. The positive
                // phases retain the Home carousel's 24 -> 0 -> 4 -> 0 rhythm.
                let inward = displacement.max(0.0);
                let value = if from_top {
                    (lower + inward).clamp(lower, upper)
                } else {
                    (upper - inward).clamp(lower, upper)
                };

                internal_update.set(true);
                adjustment.set_value(value);
                internal_update.set(false);

                if progress >= 1.0 {
                    internal_update.set(true);
                    adjustment.set_value(if from_top { lower } else { upper });
                    internal_update.set(false);
                    active.set(false);
                    glib::ControlFlow::Break
                } else {
                    glib::ControlFlow::Continue
                }
            });
        })
    };

    {
        let ready = ready.clone();
        scroll.connect_map(move |scroll| {
            ready.set(false);
            let ready = ready.clone();
            let weak_scroll = scroll.downgrade();

            glib::timeout_add_local_once(Duration::from_millis(180), move || {
                if weak_scroll.upgrade().is_some() {
                    ready.set(true);
                }
            });
        });
    }

    {
        let trigger = trigger.clone();
        scroll.connect_edge_reached(move |_, position| {
            trigger(position);
        });
    }

    {
        let trigger = trigger.clone();
        scroll.connect_edge_overshot(move |_, position| {
            trigger(position);
        });
    }

    {
        let adjustment = scroll.vadjustment();
        let last_value = Rc::new(Cell::new(adjustment.value()));
        let ready = ready.clone();
        let active = active.clone();
        let internal_update = internal_update.clone();
        let trigger = trigger.clone();

        adjustment.connect_value_changed(move |adjustment| {
            let value = adjustment.value();
            let previous = last_value.replace(value);

            if !ready.get() || active.get() || internal_update.get() {
                return;
            }

            let lower = adjustment.lower();
            let upper = (adjustment.upper() - adjustment.page_size()).max(lower);
            const EDGE_EPSILON: f64 = 0.75;

            if value <= lower + EDGE_EPSILON && previous > value + EDGE_EPSILON {
                trigger(gtk::PositionType::Top);
            } else if value >= upper - EDGE_EPSILON && previous < value - EDGE_EPSILON {
                trigger(gtk::PositionType::Bottom);
            }
        });
    }

    {
        let ready = ready.clone();
        let active = active.clone();
        let generation = generation.clone();

        scroll.connect_unmap(move |_| {
            ready.set(false);
            generation.set(generation.get().wrapping_add(1));
            active.set(false);
        });
    }
}

fn collection_page(title: &str, subtitle: &str, icon_name: &str, grid: &gtk::FlowBox) -> gtk::Box {
    let header = collection_page_header(title, subtitle, icon_name);
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.set_vexpand(true);
    scroll.set_child(Some(grid));
    install_vertical_edge_spring(&scroll);

    let page = gtk::Box::new(gtk::Orientation::Vertical, 14);
    page.set_hexpand(true);
    page.set_vexpand(true);
    page.add_css_class("library-panel");
    page.add_css_class("collection-page");
    page.append(&header);
    page.append(&scroll);
    page
}

fn collection_page_header(title: &str, subtitle: &str, icon_name: &str) -> gtk::Box {
    let icon = gtk::Image::from_icon_name(icon_name);
    icon.set_pixel_size(30);
    icon.add_css_class("collection-page-icon");

    let title_label = gtk::Label::new(Some(title));
    title_label.set_xalign(0.0);
    title_label.add_css_class("collection-page-title");
    let subtitle_label = gtk::Label::new(Some(subtitle));
    subtitle_label.set_xalign(0.0);
    subtitle_label.set_wrap(true);
    subtitle_label.add_css_class("dim-label");

    let text = gtk::Box::new(gtk::Orientation::Vertical, 3);
    text.set_hexpand(true);
    text.append(&title_label);
    text.append(&subtitle_label);

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    header.add_css_class("collection-page-header");
    header.add_css_class("expressive-page-header");
    header.append(&icon);
    header.append(&text);
    header
}

fn append_collection_grid_card(grid: &gtk::FlowBox, _position: i32, button: gtk::Button) {
    button.set_opacity(0.0);
    button.set_margin_top(14);
    button.add_css_class("collection-card-entering");
    grid.insert(&button, -1);

    let button_weak = button.downgrade();
    let started_at = Rc::new(Cell::new(None::<i64>));
    button.add_tick_callback(move |_, frame_clock| {
        let Some(button) = button_weak.upgrade() else {
            return glib::ControlFlow::Break;
        };

        let now = frame_clock.frame_time();
        let start = started_at.get().unwrap_or_else(|| {
            started_at.set(Some(now));
            now
        });
        let progress = ((now - start) as f64 / 420_000.0).clamp(0.0, 1.0);

        // Damped spring entrance: fast arrival, subtle overshoot and settle.
        let damping = (-6.5 * progress).exp();
        let oscillation = (progress * std::f64::consts::TAU * 1.65).cos();
        let spring = 1.0 - damping * oscillation;

        let opacity = (progress / 0.42).clamp(0.0, 1.0);
        let displacement = (1.0 - spring) * 18.0;

        button.set_opacity(opacity);
        button.set_margin_top(displacement.round().clamp(-4.0, 18.0) as i32);

        if progress >= 1.0 {
            button.set_opacity(1.0);
            button.set_margin_top(0);
            button.remove_css_class("collection-card-entering");
            glib::ControlFlow::Break
        } else {
            glib::ControlFlow::Continue
        }
    });
}

fn collection_button(
    card: gtk::Box,
    route: BrowserRoute,
    event_tx: &Sender<BrowserEvent>,
) -> gtk::Button {
    let button = gtk::Button::new();
    button.set_child(Some(&card));
    button.set_size_request(
        COLLECTION_CARD_MAX_WIDTH + 20,
        COLLECTION_CARD_MIN_HEIGHT + 12,
    );
    button.set_hexpand(true);
    button.set_halign(gtk::Align::Fill);
    button.set_valign(gtk::Align::Start);
    button.add_css_class("flat");
    button.add_css_class("collection-card-button");
    button.add_css_class("expressive-collection-button");

    let sender = event_tx.clone();
    button.connect_clicked(move |_| {
        let _ = sender.send(BrowserEvent::Navigate(route.clone()));
    });

    button
}

fn collection_event_button(
    card: gtk::Box,
    event: BrowserEvent,
    event_tx: &Sender<BrowserEvent>,
) -> gtk::Button {
    let button = gtk::Button::new();
    button.set_child(Some(&card));
    button.set_size_request(
        COLLECTION_CARD_MAX_WIDTH + 20,
        COLLECTION_CARD_MIN_HEIGHT + 12,
    );
    button.set_hexpand(true);
    button.set_halign(gtk::Align::Fill);
    button.set_valign(gtk::Align::Start);
    button.add_css_class("flat");
    button.add_css_class("collection-card-button");
    button.add_css_class("expressive-collection-button");

    let sender = event_tx.clone();
    button.connect_clicked(move |_| {
        let _ = sender.send(event.clone());
    });

    button
}

fn page_header(title: &str, subtitle: &str) -> gtk::Box {
    let title_label = gtk::Label::new(Some(title));
    title_label.set_xalign(0.0);
    title_label.add_css_class("section-title");
    let subtitle_label = gtk::Label::new(Some(subtitle));
    subtitle_label.set_xalign(0.0);
    subtitle_label.set_wrap(true);
    subtitle_label.add_css_class("dim-label");

    let header = gtk::Box::new(gtk::Orientation::Vertical, 3);
    header.append(&title_label);
    header.append(&subtitle_label);
    header
}

fn collection_card(
    cover_path: Option<&Path>,
    title: &str,
    subtitle: &str,
    _detail: &str,
    online: bool,
) -> gtk::Box {
    let artwork = artwork(cover_path, COLLECTION_ARTWORK_MIN_SIZE);
    let title_label = gtk::Label::new(Some(title));
    title_label.set_xalign(0.0);
    title_label.set_single_line_mode(true);
    title_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title_label.set_width_chars(18);
    title_label.set_max_width_chars(18);
    title_label.add_css_class("collection-card-title");
    title_label.add_css_class("expressive-card-title");
    let subtitle_label = gtk::Label::new(Some(subtitle));
    subtitle_label.set_xalign(0.0);
    subtitle_label.set_single_line_mode(true);
    subtitle_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    subtitle_label.set_width_chars(18);
    subtitle_label.set_max_width_chars(18);
    subtitle_label.add_css_class("dim-label");
    subtitle_label.add_css_class("expressive-card-subtitle");
    let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
    card.set_size_request(COLLECTION_CARD_MAX_WIDTH, COLLECTION_CARD_MIN_HEIGHT);
    card.set_hexpand(true);
    card.set_vexpand(false);
    card.set_halign(gtk::Align::Fill);
    card.set_valign(gtk::Align::Start);
    card.add_css_class("collection-card");
    card.add_css_class("expressive-collection-card");
    if online {
        card.add_css_class("youtube-collection-card");
    }
    card.append(&artwork);
    card.append(&title_label);
    if !subtitle.is_empty() {
        card.append(&subtitle_label);
    }
    bind_responsive_collection_artwork(&card, &artwork, cover_path.map(Path::to_path_buf));
    card
}

fn collection_card_with_placeholder(
    cover_path: Option<&Path>,
    title: &str,
    subtitle: &str,
    detail: &str,
    online: bool,
    placeholder_icon: &str,
    placeholder_class: &str,
) -> gtk::Box {
    let card = collection_card(cover_path, title, subtitle, detail, online);

    if cover_path.is_none() {
        if let Some(artwork) = card
            .first_child()
            .and_then(|child| child.downcast::<gtk::Stack>().ok())
        {
            artwork.add_css_class("typed-collection-placeholder");
            artwork.add_css_class(placeholder_class);

            if let Some(icon) = artwork
                .first_child()
                .and_then(|child| child.downcast::<gtk::Image>().ok())
            {
                icon.set_icon_name(Some(placeholder_icon));
                icon.add_css_class("typed-placeholder-icon");
            }
        }
    }

    card
}

fn collection_placeholder(title: &str, subtitle: &str) -> gtk::Box {
    collection_card(None, title, subtitle, "YouTube Music", true)
}

fn artwork(path: Option<&Path>, size: i32) -> gtk::Stack {
    let placeholder = gtk::Image::from_icon_name("folder-music-symbolic");
    placeholder.set_pixel_size(size / 3);
    placeholder.set_halign(gtk::Align::Center);
    placeholder.set_valign(gtk::Align::Center);
    placeholder.set_hexpand(true);
    placeholder.set_vexpand(true);
    placeholder.add_css_class("cover-icon");

    let picture = gtk::Picture::new();
    picture.set_content_fit(gtk::ContentFit::Cover);
    picture.set_size_request(size, size);
    picture.set_can_shrink(true);

    let stack = gtk::Stack::new();
    stack.set_size_request(size, size);
    stack.set_halign(gtk::Align::Center);
    stack.set_overflow(gtk::Overflow::Hidden);
    stack.add_named(&placeholder, Some("placeholder"));
    stack.add_named(&picture, Some("picture"));
    stack.add_css_class("collection-artwork");
    stack.add_css_class("expressive-artwork");

    if let Some(path) = path.filter(|path| path.is_file()) {
        if let Some(texture) = cached_square_texture(path, size) {
            picture.set_paintable(Some(&texture));
            stack.set_visible_child_name("picture");
        } else {
            stack.set_visible_child_name("placeholder");
        }
    } else {
        stack.set_visible_child_name("placeholder");
    }
    stack
}

fn bind_responsive_collection_artwork(
    card: &gtk::Box,
    artwork: &gtk::Stack,
    cover_path: Option<PathBuf>,
) {
    // expressive_home_card_motion_stability_v1: bounded artwork settling
    //
    // The old implementation kept one tick callback alive for every card for
    // the entire lifetime of the widget. Card hover/click motion could then
    // overlap with responsive artwork resizing and texture replacement.
    //
    // This callback exists only while the initial allocation settles. It is
    // invalidated on unmap, uses weak widget references and stops after three
    // stable frames.
    let current_size = Rc::new(Cell::new(0_i32));
    let generation = Rc::new(Cell::new(0_u64));

    let schedule: Rc<dyn Fn()> = {
        let card_weak = card.downgrade();
        let artwork_weak = artwork.downgrade();
        let cover_path = cover_path.clone();
        let current_size = current_size.clone();
        let generation = generation.clone();

        Rc::new(move || {
            let Some(card) = card_weak.upgrade() else {
                return;
            };

            let token = generation.get().wrapping_add(1);
            generation.set(token);

            let card_weak = card.downgrade();
            let artwork_weak = artwork_weak.clone();
            let cover_path = cover_path.clone();
            let current_size = current_size.clone();
            let generation = generation.clone();
            let stable_frames = Rc::new(Cell::new(0_u8));

            card.add_tick_callback(move |_, _| {
                if generation.get() != token {
                    return glib::ControlFlow::Break;
                }

                let Some(card) = card_weak.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                let Some(artwork) = artwork_weak.upgrade() else {
                    return glib::ControlFlow::Break;
                };

                let width = card.width().max(COLLECTION_CARD_MIN_WIDTH);
                let target = responsive_collection_artwork_size(width);

                if target == current_size.get() {
                    let next = stable_frames.get().saturating_add(1);
                    stable_frames.set(next);
                    return if next >= 3 {
                        glib::ControlFlow::Break
                    } else {
                        glib::ControlFlow::Continue
                    };
                }

                stable_frames.set(0);
                current_size.set(target);
                artwork.set_size_request(target, target);

                if let Some(placeholder) = artwork
                    .first_child()
                    .and_then(|child| child.downcast::<gtk::Image>().ok())
                {
                    placeholder.set_pixel_size(target / 3);
                }

                if let Some(picture) = artwork
                    .last_child()
                    .and_then(|child| child.downcast::<gtk::Picture>().ok())
                {
                    picture.set_size_request(target, target);

                    if let Some(path) = cover_path.as_deref().filter(|path| path.is_file()) {
                        if let Some(texture) = cached_square_texture(path, target) {
                            picture.set_paintable(Some(&texture));
                            artwork.set_visible_child_name("picture");
                        }
                    }
                }

                glib::ControlFlow::Continue
            });
        })
    };

    {
        let schedule = schedule.clone();
        card.connect_map(move |_| schedule());
    }

    {
        let generation = generation.clone();
        card.connect_unmap(move |_| {
            generation.set(generation.get().wrapping_add(1));
        });
    }

    schedule();
}

fn responsive_collection_artwork_size(card_width: i32) -> i32 {
    let raw = (card_width - 16).clamp(COLLECTION_ARTWORK_MIN_SIZE, COLLECTION_ARTWORK_MAX_SIZE);
    raw - raw % 8
}

fn cached_square_texture(path: &Path, size: i32) -> Option<gdk::Texture> {
    let stamp = fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    let key = (path.to_path_buf(), size, stamp);

    if let Some(texture) = ARTWORK_TEXTURES.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache.clock = cache.clock.wrapping_add(1);
        let now = cache.clock;
        cache.entries.get_mut(&key).map(|entry| {
            entry.last_used = now;
            entry.texture.clone()
        })
    }) {
        return Some(texture);
    }

    let texture = square_pixbuf(path, size).map(|pixbuf| gdk::Texture::for_pixbuf(&pixbuf))?;
    ARTWORK_TEXTURES.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache.clock = cache.clock.wrapping_add(1);
        let now = cache.clock;

        if cache.entries.len() >= ARTWORK_TEXTURE_CACHE_LIMIT {
            if let Some(oldest) = cache
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(key, _)| key.clone())
            {
                cache.entries.remove(&oldest);
            }
        }

        cache.entries.insert(
            key,
            CachedArtworkTexture {
                texture: texture.clone(),
                last_used: now,
            },
        );
    });
    Some(texture)
}

fn square_pixbuf(path: &Path, size: i32) -> Option<gdk_pixbuf::Pixbuf> {
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

fn queue_action_menu(
    entry: VisibleTrack,
    liked: bool,
    event_tx: &Sender<BrowserEvent>,
    language: AppLanguage,
) -> gtk::MenuButton {
    let labels = match language {
        AppLanguage::Portuguese => (
            "Mais ações",
            "Reproduzir em seguida",
            "Adicionar ao fim da fila",
            "Curtir",
            "Remover curtida",
        ),
        AppLanguage::English => (
            "More actions",
            "Play next",
            "Add to end of queue",
            "Like",
            "Remove like",
        ),
        AppLanguage::Spanish => (
            "Más acciones",
            "Reproducir a continuación",
            "Añadir al final de la cola",
            "Me gusta",
            "Quitar Me gusta",
        ),
    };

    let popover = gtk::Popover::new();
    popover.set_autohide(true);

    let actions = gtk::Box::new(gtk::Orientation::Vertical, 4);
    actions.set_margin_top(6);
    actions.set_margin_bottom(6);
    actions.set_margin_start(6);
    actions.set_margin_end(6);
    actions.add_css_class("queue2-browser-actions");

    let play_next = gtk::Button::with_label(labels.1);
    play_next.set_halign(gtk::Align::Fill);
    play_next.add_css_class("flat");

    let append = gtk::Button::with_label(labels.2);
    append.set_halign(gtk::Align::Fill);
    append.add_css_class("flat");

    let favorite = gtk::Button::with_label(if liked { labels.4 } else { labels.3 });
    favorite.set_halign(gtk::Align::Fill);
    favorite.add_css_class("flat");

    {
        let tx = event_tx.clone();
        let entry = entry.clone();
        let action_popover = popover.clone();
        play_next.connect_clicked(move |_| {
            let event = match entry.clone() {
                VisibleTrack::Local(index) => BrowserEvent::QueueLocalPlayNext(index),
                VisibleTrack::YouTube(item) => BrowserEvent::QueueYouTubePlayNext(*item),
            };
            let _ = tx.send(event);
            action_popover.popdown();
        });
    }

    {
        let tx = event_tx.clone();
        let action_popover = popover.clone();
        let append_entry = entry.clone();
        append.connect_clicked(move |_| {
            let event = match append_entry.clone() {
                VisibleTrack::Local(index) => BrowserEvent::QueueLocalAppend(index),
                VisibleTrack::YouTube(item) => BrowserEvent::QueueYouTubeAppend(*item),
            };
            let _ = tx.send(event);
            action_popover.popdown();
        });
    }

    {
        let tx = event_tx.clone();
        let action_popover = popover.clone();
        let favorite_entry = entry.clone();
        favorite.connect_clicked(move |_| {
            let event = match favorite_entry.clone() {
                VisibleTrack::Local(index) => BrowserEvent::ToggleLocalTrackFavorite(index),
                VisibleTrack::YouTube(item) => BrowserEvent::ToggleYouTubeTrackFavorite(*item),
            };
            let _ = tx.send(event);
            action_popover.popdown();
        });
    }

    actions.append(&play_next);
    actions.append(&append);
    actions.append(&favorite);
    popover.set_child(Some(&actions));

    let menu = gtk::MenuButton::builder()
        .icon_name("view-more-symbolic")
        .tooltip_text(labels.0)
        .build();
    menu.add_css_class("flat");
    menu.add_css_class("circular");
    menu.add_css_class("queue2-browser-menu");
    menu.set_popover(Some(&popover));
    menu
}

fn track_row(
    number: usize,
    track: &Track,
    liked: bool,
    index: usize,
    event_tx: &Sender<BrowserEvent>,
    language: AppLanguage,
) -> gtk::ListBoxRow {
    let number_label = gtk::Label::new(Some(&number.to_string()));
    number_label.set_width_chars(3);
    number_label.add_css_class("track-number");
    number_label.add_css_class("track-position-indicator");

    let title = gtk::Label::new(Some(&track.title));
    title.set_xalign(0.0);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title.add_css_class("track-title");
    let subtitle = gtk::Label::new(Some(&format!("{} — {}", track.artist, track.album)));
    subtitle.set_xalign(0.0);
    subtitle.set_ellipsize(gtk::pango::EllipsizeMode::End);
    subtitle.add_css_class("dim-label");
    let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
    text.set_hexpand(true);
    text.append(&title);
    text.append(&subtitle);

    let source = source_badge("Local", false);
    let favorite = gtk::Image::from_icon_name("emblem-favorite-symbolic");
    favorite.set_opacity(if liked { 0.9 } else { 0.20 });

    let lyric_status = gtk::Image::from_icon_name(if track.lyrics.is_empty() {
        "audio-input-microphone-symbolic"
    } else {
        "emblem-ok-symbolic"
    });
    lyric_status.set_opacity(if track.lyrics.is_empty() { 0.25 } else { 0.8 });

    let duration = gtk::Label::new(Some(&format_duration(track.duration_seconds)));
    duration.add_css_class("time-label");

    let menu = queue_action_menu(VisibleTrack::Local(index), liked, event_tx, language);

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    content.set_margin_top(10);
    content.set_margin_bottom(10);
    content.set_margin_start(12);
    content.set_margin_end(8);
    content.append(&number_label);
    content.append(&text);
    content.append(&source);
    content.append(&favorite);
    content.append(&lyric_status);
    content.append(&duration);
    content.append(&menu);

    let row = gtk::ListBoxRow::new();
    row.add_css_class("media-list-row");
    row.set_child(Some(&content));
    row
}

fn youtube_track_row(
    number: usize,
    item: &YouTubeItem,
    liked: bool,
    event_tx: &Sender<BrowserEvent>,
    language: AppLanguage,
) -> gtk::ListBoxRow {
    let number_label = gtk::Label::new(Some(&number.to_string()));
    number_label.set_width_chars(3);
    number_label.add_css_class("track-number");
    number_label.add_css_class("track-position-indicator");

    let title = gtk::Label::new(Some(&item.title));
    title.set_xalign(0.0);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title.add_css_class("track-title");
    let subtitle_text = if !item.subtitle.is_empty() {
        item.subtitle.clone()
    } else {
        format!("{} — {}", item.artist, item.album)
    };
    let subtitle = gtk::Label::new(Some(&subtitle_text));
    subtitle.set_xalign(0.0);
    subtitle.set_ellipsize(gtk::pango::EllipsizeMode::End);
    subtitle.add_css_class("dim-label");
    let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
    text.set_hexpand(true);
    text.append(&title);
    text.append(&subtitle);

    let source = source_badge("YouTube", true);
    let favorite = gtk::Image::from_icon_name("emblem-favorite-symbolic");
    favorite.set_opacity(if liked { 0.95 } else { 0.20 });
    let duration = gtk::Label::new(Some(&format_duration(item.duration_seconds)));
    duration.add_css_class("time-label");

    let menu = queue_action_menu(
        VisibleTrack::YouTube(Box::new(item.clone())),
        liked,
        event_tx,
        language,
    );

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    content.set_margin_top(10);
    content.set_margin_bottom(10);
    content.set_margin_start(12);
    content.set_margin_end(8);
    content.append(&number_label);
    content.append(&text);
    content.append(&source);
    content.append(&favorite);
    content.append(&duration);
    content.append(&menu);

    let row = gtk::ListBoxRow::new();
    row.add_css_class("media-list-row");
    row.add_css_class("youtube-media-row");
    row.add_css_class("youtube-track-row");
    row.set_child(Some(&content));
    row
}

fn source_badge(text: &str, online: bool) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("source-badge");
    if online {
        label.add_css_class("youtube-source-badge");
    }
    label
}

fn clear_box(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}

fn youtube_mix_page_header(
    item: &YouTubeItem,
    track_count: Option<usize>,
    language: AppLanguage,
) -> gtk::Box {
    let cover = artwork(item.cached_cover(), 112);
    cover.set_size_request(112, 112);
    cover.set_halign(gtk::Align::Start);
    cover.add_css_class("mix-page-cover");

    let eyebrow = gtk::Label::new(Some(match language {
        AppLanguage::Portuguese => "YOUTUBE MUSIC · MIX",
        AppLanguage::English => "YOUTUBE MUSIC · MIX",
        AppLanguage::Spanish => "YOUTUBE MUSIC · MIX",
    }));
    eyebrow.set_xalign(0.0);
    eyebrow.add_css_class("dim-label");
    eyebrow.add_css_class("collection-context-eyebrow");

    let title = gtk::Label::new(Some(&item.title));
    title.set_xalign(0.0);
    title.set_wrap(true);
    title.add_css_class("collection-page-title");
    title.add_css_class("mix-page-title");

    let subtitle_text = if item.subtitle.trim().is_empty() {
        match language {
            AppLanguage::Portuguese => "Mix personalizado para você",
            AppLanguage::English => "A personalized mix for you",
            AppLanguage::Spanish => "Un mix personalizado para ti",
        }
    } else {
        item.subtitle.as_str()
    };
    let subtitle = gtk::Label::new(Some(subtitle_text));
    subtitle.set_xalign(0.0);
    subtitle.set_wrap(true);
    subtitle.add_css_class("dim-label");

    let detail_text = match track_count {
        Some(count) => match language {
            AppLanguage::Portuguese => {
                format!("{count} {}", if count == 1 { "faixa" } else { "faixas" })
            }
            AppLanguage::English => {
                format!("{count} {}", if count == 1 { "track" } else { "tracks" })
            }
            AppLanguage::Spanish => {
                format!("{count} {}", if count == 1 { "pista" } else { "pistas" })
            }
        },
        None => match language {
            AppLanguage::Portuguese => "Seleção personalizada do YouTube Music".to_string(),
            AppLanguage::English => "Personalized selection from YouTube Music".to_string(),
            AppLanguage::Spanish => "Selección personalizada de YouTube Music".to_string(),
        },
    };
    let detail = gtk::Label::new(Some(&detail_text));
    detail.set_xalign(0.0);
    detail.add_css_class("dim-label");
    detail.add_css_class("mix-page-detail");

    let text = gtk::Box::new(gtk::Orientation::Vertical, 5);
    text.set_hexpand(true);
    text.set_valign(gtk::Align::Center);
    text.append(&eyebrow);
    text.append(&title);
    text.append(&subtitle);
    text.append(&detail);

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 18);
    header.set_hexpand(true);
    header.set_margin_top(4);
    header.set_margin_bottom(14);
    header.set_margin_start(2);
    header.set_margin_end(2);
    header.append(&cover);
    header.append(&text);
    header.add_css_class("mix-page-header");
    header.add_css_class("expressive-page-header");
    header
}

fn youtube_mix_row(item: &YouTubeItem, track_count: Option<usize>) -> gtk::ListBoxRow {
    let cover = artwork(item.cached_cover(), 56);
    cover.set_size_request(56, 56);
    cover.add_css_class("playlist-row-artwork");
    cover.add_css_class("mix-row-artwork");

    let title = gtk::Label::new(Some(&item.title));
    title.set_xalign(0.0);
    title.set_hexpand(true);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title.add_css_class("track-title");
    title.add_css_class("playlist-row-title");

    let subtitle_text = if item.subtitle.trim().is_empty() {
        "Mix criado para você"
    } else {
        item.subtitle.as_str()
    };
    let subtitle = gtk::Label::new(Some(subtitle_text));
    subtitle.set_xalign(0.0);
    subtitle.set_ellipsize(gtk::pango::EllipsizeMode::End);
    subtitle.add_css_class("dim-label");
    subtitle.add_css_class("playlist-row-subtitle");

    let detail_text = track_count
        .map(|count| format!("{count} {}", if count == 1 { "faixa" } else { "faixas" }))
        .unwrap_or_else(|| "Seleção personalizada do YouTube Music".to_string());
    let detail = gtk::Label::new(Some(&detail_text));
    detail.set_xalign(0.0);
    detail.set_ellipsize(gtk::pango::EllipsizeMode::End);
    detail.add_css_class("dim-label");
    detail.add_css_class("playlist-row-detail");

    let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
    text.set_hexpand(true);
    text.set_valign(gtk::Align::Center);
    text.append(&title);
    text.append(&subtitle);
    text.append(&detail);

    let mix_badge = gtk::Label::new(Some("MIX"));
    mix_badge.add_css_class("source-badge");
    mix_badge.add_css_class("youtube-source-badge");
    mix_badge.add_css_class("mix-source-badge");

    let arrow = gtk::Image::from_icon_name("go-next-symbolic");
    arrow.add_css_class("dim-label");

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    content.set_margin_top(8);
    content.set_margin_bottom(8);
    content.set_margin_start(10);
    content.set_margin_end(12);
    content.append(&cover);
    content.append(&text);
    content.append(&mix_badge);
    content.append(&arrow);

    let row = gtk::ListBoxRow::new();
    row.add_css_class("playlist-card-row");
    row.add_css_class("youtube-playlist-row");
    row.add_css_class("youtube-mix-row");
    row.set_child(Some(&content));
    row
}

fn playlist_row(name: &str, detail: &str, online: bool) -> gtk::ListBoxRow {
    let icon = gtk::Image::from_icon_name(if online {
        "network-server-symbolic"
    } else {
        "view-list-symbolic"
    });
    icon.set_pixel_size(24);
    let title = gtk::Label::new(Some(name));
    title.set_xalign(0.0);
    title.set_hexpand(true);
    title.add_css_class("track-title");
    let count = gtk::Label::new(Some(detail));
    count.add_css_class("dim-label");
    let arrow = gtk::Image::from_icon_name("go-next-symbolic");

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(12);
    content.set_margin_end(12);
    content.append(&icon);
    content.append(&title);
    if online {
        content.append(&source_badge("YouTube Music", true));
    }
    content.append(&count);
    content.append(&arrow);

    let row = gtk::ListBoxRow::new();
    row.add_css_class("playlist-card-row");
    if online {
        row.add_css_class("youtube-playlist-row");
    }
    row.set_child(Some(&content));
    row
}

fn section_row(text: &str) -> gtk::ListBoxRow {
    let label = gtk::Label::new(Some(text));
    label.set_xalign(0.0);
    label.set_margin_top(14);
    label.set_margin_bottom(6);
    label.set_margin_start(12);
    label.add_css_class("section-title");
    let row = gtk::ListBoxRow::new();
    row.add_css_class("list-section-row");
    row.set_activatable(false);
    row.set_selectable(false);
    row.set_child(Some(&label));
    row
}

fn empty_row(message: &str) -> gtk::ListBoxRow {
    let label = gtk::Label::new(Some(message));
    label.set_margin_top(30);
    label.set_margin_bottom(30);
    label.add_css_class("dim-label");
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    row.set_selectable(false);
    row.set_child(Some(&label));
    row
}

fn loading_row(message: &str) -> gtk::ListBoxRow {
    let spinner = gtk::Spinner::new();
    spinner.start();
    let label = gtk::Label::new(Some(message));
    label.add_css_class("dim-label");

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    content.set_halign(gtk::Align::Center);
    content.set_margin_top(30);
    content.set_margin_bottom(30);
    content.append(&spinner);
    content.append(&label);

    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    row.set_selectable(false);
    row.set_child(Some(&content));
    row
}

fn route_transition(previous: &BrowserRoute, next: &BrowserRoute) -> gtk::StackTransitionType {
    match (route_is_detail(previous), route_is_detail(next)) {
        (false, true) => gtk::StackTransitionType::SlideLeft,
        (true, false) => gtk::StackTransitionType::SlideRight,
        _ => gtk::StackTransitionType::Crossfade,
    }
}

fn route_is_detail(route: &BrowserRoute) -> bool {
    matches!(
        route,
        BrowserRoute::Album(_)
            | BrowserRoute::Artist(_)
            | BrowserRoute::Playlist(_)
            | BrowserRoute::YouTubeAlbum(_)
            | BrowserRoute::YouTubeArtist(_)
            | BrowserRoute::YouTubePlaylist { .. }
    )
}

fn route_title(
    route: &BrowserRoute,
    source: Option<StartupSource>,
    language: AppLanguage,
) -> String {
    match route {
        BrowserRoute::All => "BIBLIOTECA".to_string(),
        BrowserRoute::Liked => liked_route_title(source, language).to_string(),
        BrowserRoute::Album(name) => format!("ÁLBUM LOCAL · {name}"),
        BrowserRoute::Artist(name) => format!("ARTISTA LOCAL · {name}"),
        BrowserRoute::Playlist(name) => format!("PLAYLIST LOCAL · {name}"),
        BrowserRoute::YouTubeAlbum(name) => format!("YOUTUBE MUSIC · ÁLBUM · {name}"),
        BrowserRoute::YouTubeArtist(name) => format!("YOUTUBE MUSIC · ARTISTA · {name}"),
        BrowserRoute::YouTubePlaylist { title, .. } => {
            format!("YOUTUBE MUSIC · PLAYLIST · {title}")
        }
        _ => "BIBLIOTECA".to_string(),
    }
}

fn liked_route_title(source: Option<StartupSource>, language: AppLanguage) -> &'static str {
    match (language, source == Some(StartupSource::YouTube)) {
        (AppLanguage::Portuguese, false) => "MÚSICAS CURTIDAS LOCAIS",
        (AppLanguage::Portuguese, true) => "MÚSICAS CURTIDAS · YOUTUBE MUSIC",
        (AppLanguage::English, false) => "LOCAL LIKED SONGS",
        (AppLanguage::English, true) => "LIKED SONGS · YOUTUBE MUSIC",
        (AppLanguage::Spanish, false) => "CANCIONES LOCALES FAVORITAS",
        (AppLanguage::Spanish, true) => "CANCIONES FAVORITAS · YOUTUBE MUSIC",
    }
}

fn liked_empty_message(source: Option<StartupSource>, language: AppLanguage) -> &'static str {
    match (language, source == Some(StartupSource::YouTube)) {
        (AppLanguage::Portuguese, false) => "Nenhuma música local curtida ainda",
        (AppLanguage::Portuguese, true) => "Nenhuma música curtida no YouTube Music",
        (AppLanguage::English, false) => "No local liked songs yet",
        (AppLanguage::English, true) => "No liked songs on YouTube Music",
        (AppLanguage::Spanish, false) => "Aún no hay canciones locales favoritas",
        (AppLanguage::Spanish, true) => "No hay canciones favoritas en YouTube Music",
    }
}

fn clear_list_box(list: &gtk::ListBox) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
}

fn clear_grid(grid: &gtk::FlowBox) {
    while let Some(child) = grid.first_child() {
        grid.remove(&child);
    }
}

fn format_duration(seconds: u64) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

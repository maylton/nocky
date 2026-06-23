use crate::{
    config::AppConfig,
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
    time::UNIX_EPOCH,
};

const ARTWORK_TEXTURE_CACHE_LIMIT: usize = 160;

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
    YouTubeTrackActivated {
        item: YouTubeItem,
        queue: Vec<YouTubeItem>,
        index: usize,
    },
    OpenYouTubePlaylist(YouTubeItem),
    OpenYouTubeCollection(YouTubeItem),
    Navigate(BrowserRoute),
    CreatePlaylist(String),
    AddCurrentToPlaylist(String),
    RemoveCurrentFromPlaylist(String),
    DeletePlaylist(String),
}

#[derive(Clone, Debug)]
enum VisibleTrack {
    Local(usize),
    YouTube(YouTubeItem),
}

#[derive(Clone, Debug)]
enum PlaylistRef {
    Local(String),
    YouTube(YouTubeItem),
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

pub struct LibraryBrowser {
    root: gtk::Stack,
    home_content: gtk::Box,
    queue: gtk::ListBox,
    queue_title: gtk::Label,
    albums_grid: gtk::FlowBox,
    artists_grid: gtk::FlowBox,
    playlists_list: gtk::ListBox,
    playlist_model: gtk::StringList,
    playlist_dropdown: gtk::DropDown,
    route: RefCell<BrowserRoute>,
    visible_tracks: Rc<RefCell<Vec<VisibleTrack>>>,
    queue_render_generation: Rc<Cell<u64>>,
    playlist_names: Rc<RefCell<Vec<String>>>,
    playlist_row_refs: Rc<RefCell<Vec<Option<PlaylistRef>>>>,
    event_tx: Sender<BrowserEvent>,
    events: Receiver<BrowserEvent>,
}

impl LibraryBrowser {
    pub fn new() -> Self {
        let (event_tx, events) = mpsc::channel();
        let visible_tracks = Rc::new(RefCell::new(Vec::new()));
        let queue_render_generation = Rc::new(Cell::new(0_u64));
        let playlist_names = Rc::new(RefCell::new(Vec::new()));
        let playlist_row_refs = Rc::new(RefCell::new(Vec::new()));

        let home_content = gtk::Box::new(gtk::Orientation::Vertical, 22);
        home_content.set_hexpand(true);
        home_content.set_vexpand(false);
        home_content.add_css_class("library-home");

        let home_scroll = gtk::ScrolledWindow::new();
        home_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        home_scroll.set_vexpand(true);
        home_scroll.set_child(Some(&home_content));

        let queue = gtk::ListBox::new();
        queue.set_selection_mode(gtk::SelectionMode::Single);
        queue.add_css_class("queue-list");

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
                                    Some(item.clone())
                                }
                                _ => None,
                            })
                            .collect::<Vec<_>>();
                        let index = queue
                            .iter()
                            .position(|candidate| candidate.video_id == item.video_id)
                            .unwrap_or(0);
                        let _ = tx.send(BrowserEvent::YouTubeTrackActivated { item, queue, index });
                    }
                }
            });
        }

        let queue_title = gtk::Label::new(Some("BIBLIOTECA"));
        queue_title.set_xalign(0.0);
        queue_title.add_css_class("section-title");

        let queue_scroll = gtk::ScrolledWindow::new();
        queue_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        queue_scroll.set_vexpand(true);
        queue_scroll.set_child(Some(&queue));

        let tracks_page = gtk::Box::new(gtk::Orientation::Vertical, 12);
        tracks_page.set_hexpand(true);
        tracks_page.set_vexpand(true);
        tracks_page.add_css_class("library-panel");
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
        playlist_select_row.append(&playlist_dropdown);
        playlist_select_row.append(&delete_button);

        let action_row = gtk::Box::new(gtk::Orientation::Vertical, 8);
        action_row.set_hexpand(true);
        add_button.set_hexpand(true);
        remove_button.set_hexpand(true);
        action_row.append(&add_button);
        action_row.append(&remove_button);

        let playlists_list = gtk::ListBox::new();
        playlists_list.set_selection_mode(gtk::SelectionMode::Single);
        playlists_list.add_css_class("playlist-list");
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
                        let _ = tx.send(BrowserEvent::OpenYouTubePlaylist(item));
                    }
                }
            });
        }

        let playlists_scroll = gtk::ScrolledWindow::new();
        playlists_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        playlists_scroll.set_vexpand(true);
        playlists_scroll.set_child(Some(&playlists_list));

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
        root.add_named(&tracks_page, Some("tracks"));
        root.add_named(&albums_page, Some("albums"));
        root.add_named(&artists_page, Some("artists"));
        root.add_named(&playlists_page, Some("playlists"));
        root.set_visible_child_name("home");

        Self {
            root,
            home_content,
            queue,
            queue_title,
            albums_grid,
            artists_grid,
            playlists_list,
            playlist_model,
            playlist_dropdown,
            route: RefCell::new(BrowserRoute::All),
            visible_tracks,
            queue_render_generation,
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
        query: &str,
    ) {
        let previous = self.route();
        self.root
            .set_transition_type(route_transition(&previous, &route));
        self.route.replace(route);
        self.refresh(tracks, config, youtube, query);
    }

    pub fn refresh(
        &self,
        tracks: &[Track],
        config: &AppConfig,
        youtube: &YouTubeLibraryCache,
        query: &str,
    ) {
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
                self.rebuild_artist_albums(youtube, &title, query);
                self.root.set_visible_child_name("albums");
            }
            BrowserRoute::Playlists => {
                self.rebuild_playlists(config, youtube, query);
                self.root.set_visible_child_name("playlists");
            }
            BrowserRoute::All if query.trim().is_empty() => {
                self.rebuild_home(tracks, config, youtube);
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

        if let BrowserRoute::YouTubePlaylist { browse_id, .. } = route {
            self.rebuild_youtube_playlist_queue(youtube, query, route, browse_id, render_token);
            return;
        }

        if matches!(
            route,
            BrowserRoute::YouTubeAlbum(_) | BrowserRoute::YouTubeArtist(_)
        ) {
            self.rebuild_youtube_collection_queue(youtube, query, route, render_token);
            return;
        }

        let query = query.trim().to_lowercase();
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
            BrowserRoute::Liked => tracks
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
            self.queue
                .append(&track_row(number, track, config.is_liked(&track.path)));
            entries.push(VisibleTrack::Local(index));
        }

        let catalog = youtube_catalog(youtube);
        let mut online_candidates = match route {
            BrowserRoute::All => catalog,
            BrowserRoute::Liked => youtube
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
            self.queue.append(&youtube_track_row(number, &item, liked));
            entries.push(VisibleTrack::YouTube(item));
        }

        self.queue_title.set_text(&route_title(route));
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
                    BrowserRoute::Liked => "Nenhuma música curtida ainda",
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
    ) {
        self.queue_title.set_text(&route_title(route));
        self.visible_tracks.borrow_mut().clear();

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
            self.queue.append(&youtube_track_row(number, &item, liked));
            self.visible_tracks
                .borrow_mut()
                .push(VisibleTrack::YouTube(item));
        }

        if items.is_empty() {
            return;
        }

        let pending = Rc::new(RefCell::new(items.into_iter().collect::<VecDeque<_>>()));
        let queue = self.queue.clone();
        let visible_tracks = self.visible_tracks.clone();
        let generation = self.queue_render_generation.clone();

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
                queue.append(&youtube_track_row(number, &item, liked));
                visible_tracks
                    .borrow_mut()
                    .push(VisibleTrack::YouTube(item));
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
    ) {
        self.queue_title.set_text(&route_title(route));
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

        self.append_youtube_rows_progressively(youtube, items, render_token);
    }

    fn append_youtube_rows_progressively(
        &self,
        youtube: &YouTubeLibraryCache,
        mut items: Vec<YouTubeItem>,
        render_token: u64,
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
            self.queue.append(&youtube_track_row(number, &item, liked));
            self.visible_tracks
                .borrow_mut()
                .push(VisibleTrack::YouTube(item));
        }

        if items.is_empty() {
            return;
        }

        let pending = Rc::new(RefCell::new(items.into_iter().collect::<VecDeque<_>>()));
        let queue = self.queue.clone();
        let visible_tracks = self.visible_tracks.clone();
        let generation = self.queue_render_generation.clone();

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
                queue.append(&youtube_track_row(number, &item, liked));
                visible_tracks
                    .borrow_mut()
                    .push(VisibleTrack::YouTube(item));
            }

            if pending.borrow().is_empty() {
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });
    }

    fn rebuild_home(&self, tracks: &[Track], config: &AppConfig, youtube: &YouTubeLibraryCache) {
        while let Some(child) = self.home_content.first_child() {
            self.home_content.remove(&child);
        }

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
        self.home_content.append(&home_section(
            "Mixtapes criadas para você",
            "Mixes e rádios sincronizadas do YouTube Music",
            mixes,
            &self.event_tx,
        ));

        self.home_content.append(&home_section(
            "Álbuns",
            "Capas e coleções sincronizadas",
            home_album_cards(tracks, youtube),
            &self.event_tx,
        ));

        self.home_content.append(&home_section(
            "Artistas",
            "Artistas organizados pela biblioteca ativa",
            home_artist_cards(tracks, youtube),
            &self.event_tx,
        ));

        let mut playlist_cards = config
            .playlists
            .iter()
            .take(8)
            .map(|playlist| HomeCard::LocalPlaylist {
                title: playlist.name.clone(),
                subtitle: format!("{} faixas locais", playlist.tracks.len()),
            })
            .collect::<Vec<_>>();
        playlist_cards.extend(
            youtube
                .playlists
                .iter()
                .filter(|playlist| !is_mix_playlist(playlist))
                .take(12)
                .cloned()
                .map(HomeCard::YouTubePlaylist),
        );
        self.home_content.append(&home_section(
            "Playlists sugeridas",
            "Playlists e recomendações sincronizadas",
            playlist_cards,
            &self.event_tx,
        ));

        if youtube.syncing {
            self.home_content.append(&home_syncing_hint());
        }
    }

    fn rebuild_albums(&self, tracks: &[Track], youtube: &YouTubeLibraryCache, query: &str) {
        clear_grid(&self.albums_grid);
        let query = query.trim().to_lowercase();
        let mut position = 0;

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
        let mut position = 0;

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
        if !online_matches.is_empty() {
            self.playlists_list.append(&section_row("YOUTUBE MUSIC"));
            row_refs.push(None);
        }
        for playlist in online_matches {
            self.playlists_list.append(&playlist_row(
                &playlist.title,
                youtube_playlist_subtitle(playlist),
                true,
            ));
            row_refs.push(Some(PlaylistRef::YouTube(playlist.clone())));
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
        .library
        .iter()
        .chain(youtube.liked.iter())
        .filter(|item| item.playable())
        .filter(|item| seen.insert(item.video_id.clone()))
        .cloned()
        .collect()
}

fn home_album_cards(tracks: &[Track], youtube: &YouTubeLibraryCache) -> Vec<HomeCard> {
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
            detail: format!("Local • {} faixas", album_tracks.len()),
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
        let albums = artist_tracks
            .iter()
            .map(|track| track.album.as_str())
            .collect::<BTreeSet<_>>()
            .len();
        let cover_path = artist_tracks
            .iter()
            .find_map(|track| track.cover_path.clone());
        cards.push(HomeCard::LocalArtist {
            title: artist,
            subtitle: format!("{albums} álbuns"),
            detail: format!("Local • {} faixas", artist_tracks.len()),
            cover_path,
        });
    }

    for artist in youtube.artists.iter().take(12) {
        let key = youtube_collection_key("artist", &artist.title);
        let source = youtube.artist_profiles.get(&key).unwrap_or(&artist.source);
        cards.push(youtube_artist_home_card_from_source(artist, source));
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
    source: &YouTubeItem,
) -> HomeCard {
    HomeCard::YouTubeArtist {
        item: source.clone(),
        subtitle: entry.subtitle.clone(),
        detail: entry.detail.clone(),
        cover_path: source
            .cached_cover()
            .or_else(|| entry.cached_cover())
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

fn home_section(
    title: &str,
    subtitle: &str,
    cards: Vec<HomeCard>,
    event_tx: &Sender<BrowserEvent>,
) -> gtk::Box {
    let title_label = gtk::Label::new(Some(title));
    title_label.set_xalign(0.0);
    title_label.add_css_class("home-section-title");
    let subtitle_label = gtk::Label::new(Some(subtitle));
    subtitle_label.set_xalign(0.0);
    subtitle_label.add_css_class("dim-label");

    let heading = gtk::Box::new(gtk::Orientation::Vertical, 2);
    heading.append(&title_label);
    heading.append(&subtitle_label);

    let rail = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    rail.add_css_class("home-carousel");

    if cards.is_empty() {
        rail.append(&home_empty_card("Aguardando conteúdo sincronizado"));
    } else {
        for card in cards {
            rail.append(&home_card_button(card, event_tx));
        }
    }

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
    scroll.set_min_content_height(190);
    scroll.set_child(Some(&rail));
    scroll.add_css_class("home-carousel-scroll");

    let section = gtk::Box::new(gtk::Orientation::Vertical, 10);
    section.add_css_class("home-section");
    section.append(&heading);
    section.append(&scroll);
    section
}

fn home_card_button(card: HomeCard, event_tx: &Sender<BrowserEvent>) -> gtk::Button {
    let (cover_path, title, subtitle, detail, online) = match &card {
        HomeCard::LocalAlbum {
            title,
            subtitle,
            detail,
            cover_path,
        }
        | HomeCard::LocalArtist {
            title,
            subtitle,
            detail,
            cover_path,
        } => (
            cover_path.as_deref(),
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
        }
        | HomeCard::YouTubeArtist {
            item,
            subtitle,
            detail,
            cover_path,
        } => (
            cover_path.as_deref(),
            item.title.as_str(),
            subtitle.as_str(),
            detail.as_str(),
            true,
        ),
        HomeCard::LocalPlaylist { title, subtitle } => (
            None,
            title.as_str(),
            subtitle.as_str(),
            "Playlist local",
            false,
        ),
        HomeCard::YouTubePlaylist(item) => (
            item.cached_cover(),
            item.title.as_str(),
            youtube_playlist_subtitle(item),
            youtube_playlist_detail(item),
            true,
        ),
    };

    let card_widget = collection_card(cover_path, title, subtitle, detail, online);
    card_widget.add_css_class("home-card");

    let button = gtk::Button::new();
    button.set_child(Some(&card_widget));
    button.add_css_class("flat");
    button.add_css_class("home-card-button");

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

fn home_empty_card(message: &str) -> gtk::Box {
    collection_card(
        None,
        message,
        "Sincronize o YouTube Music ou escolha uma pasta local",
        "",
        false,
    )
}

fn home_syncing_hint() -> gtk::Box {
    let label = gtk::Label::new(Some("Sincronizando sua biblioteca do YouTube Music..."));
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

fn collection_page(title: &str, subtitle: &str, icon_name: &str, grid: &gtk::FlowBox) -> gtk::Box {
    let header = collection_page_header(title, subtitle, icon_name);
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.set_vexpand(true);
    scroll.set_child(Some(grid));

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
    header.append(&icon);
    header.append(&text);
    header
}

fn append_collection_grid_card(grid: &gtk::FlowBox, _position: i32, button: gtk::Button) {
    grid.insert(&button, -1);
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
    let subtitle_label = gtk::Label::new(Some(subtitle));
    subtitle_label.set_xalign(0.0);
    subtitle_label.set_single_line_mode(true);
    subtitle_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    subtitle_label.set_width_chars(18);
    subtitle_label.set_max_width_chars(18);
    subtitle_label.add_css_class("dim-label");
    let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
    card.set_size_request(COLLECTION_CARD_MAX_WIDTH, COLLECTION_CARD_MIN_HEIGHT);
    card.set_hexpand(true);
    card.set_vexpand(false);
    card.set_halign(gtk::Align::Fill);
    card.set_valign(gtk::Align::Start);
    card.add_css_class("collection-card");
    if online {
        card.add_css_class("youtube-collection-card");
    }
    card.append(&artwork);
    card.append(&title_label);
    card.append(&subtitle_label);
    bind_responsive_collection_artwork(&card, &artwork, cover_path.map(Path::to_path_buf));
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
    let card = card.clone();
    let artwork = artwork.clone();
    let current_size = Rc::new(Cell::new(COLLECTION_ARTWORK_MIN_SIZE));
    let observed_card = card.clone();
    card.add_tick_callback(move |_, _| {
        let width = observed_card.width().max(COLLECTION_CARD_MIN_WIDTH);
        let target = responsive_collection_artwork_size(width);
        if target == current_size.get() {
            return glib::ControlFlow::Continue;
        }

        current_size.set(target);
        artwork.set_size_request(target, target);
        if let Some(placeholder) = artwork
            .first_child()
            .and_then(|child| child.downcast::<gtk::Image>().ok())
        {
            placeholder.set_pixel_size(target / 3);
        }
        if let Some(path) = cover_path.as_deref().filter(|path| path.is_file()) {
            if let Some(picture) = artwork
                .last_child()
                .and_then(|child| child.downcast::<gtk::Picture>().ok())
            {
                picture.set_size_request(target, target);
                if let Some(texture) = cached_square_texture(path, target) {
                    picture.set_paintable(Some(&texture));
                    artwork.set_visible_child_name("picture");
                }
            }
        }

        glib::ControlFlow::Continue
    });
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

fn track_row(number: usize, track: &Track, liked: bool) -> gtk::ListBoxRow {
    let number_label = gtk::Label::new(Some(&number.to_string()));
    number_label.set_width_chars(3);
    number_label.add_css_class("track-number");

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

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    content.set_margin_top(10);
    content.set_margin_bottom(10);
    content.set_margin_start(12);
    content.set_margin_end(12);
    content.append(&number_label);
    content.append(&text);
    content.append(&source);
    content.append(&favorite);
    content.append(&lyric_status);
    content.append(&duration);

    let row = gtk::ListBoxRow::new();
    row.set_child(Some(&content));
    row
}

fn youtube_track_row(number: usize, item: &YouTubeItem, liked: bool) -> gtk::ListBoxRow {
    let number_label = gtk::Label::new(Some(&number.to_string()));
    number_label.set_width_chars(3);
    number_label.add_css_class("track-number");

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

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    content.set_margin_top(10);
    content.set_margin_bottom(10);
    content.set_margin_start(12);
    content.set_margin_end(12);
    content.append(&number_label);
    content.append(&text);
    content.append(&source);
    content.append(&favorite);
    content.append(&duration);

    let row = gtk::ListBoxRow::new();
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

fn route_title(route: &BrowserRoute) -> String {
    match route {
        BrowserRoute::All => "BIBLIOTECA".to_string(),
        BrowserRoute::Liked => "MÚSICAS CURTIDAS".to_string(),
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

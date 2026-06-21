use crate::{config::AppConfig, model::Track};
use gtk::{gdk, gio::prelude::ListModelExt, prelude::*};
use std::{
    cell::RefCell,
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    path::Path,
    rc::Rc,
    sync::mpsc::{self, Receiver},
};

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
}

#[derive(Clone, Debug)]
pub enum BrowserEvent {
    TrackActivated(usize),
    Navigate(BrowserRoute),
    CreatePlaylist(String),
    AddCurrentToPlaylist(String),
    RemoveCurrentFromPlaylist(String),
    DeletePlaylist(String),
}

pub struct LibraryBrowser {
    root: gtk::Stack,
    queue: gtk::ListBox,
    queue_title: gtk::Label,
    albums_flow: gtk::FlowBox,
    artists_flow: gtk::FlowBox,
    playlists_list: gtk::ListBox,
    playlist_model: gtk::StringList,
    playlist_dropdown: gtk::DropDown,
    route: RefCell<BrowserRoute>,
    visible_indices: Rc<RefCell<Vec<usize>>>,
    album_names: Rc<RefCell<Vec<String>>>,
    artist_names: Rc<RefCell<Vec<String>>>,
    playlist_names: Rc<RefCell<Vec<String>>>,
    playlist_row_names: Rc<RefCell<Vec<String>>>,
    events: Receiver<BrowserEvent>,
}

impl LibraryBrowser {
    pub fn new() -> Self {
        let (event_tx, events) = mpsc::channel();
        let visible_indices = Rc::new(RefCell::new(Vec::new()));
        let album_names = Rc::new(RefCell::new(Vec::new()));
        let artist_names = Rc::new(RefCell::new(Vec::new()));
        let playlist_names = Rc::new(RefCell::new(Vec::new()));
        let playlist_row_names = Rc::new(RefCell::new(Vec::new()));

        let queue = gtk::ListBox::new();
        queue.set_selection_mode(gtk::SelectionMode::Single);
        queue.add_css_class("queue-list");

        {
            let tx = event_tx.clone();
            let indices = visible_indices.clone();
            queue.connect_row_activated(move |_, row| {
                if let Some(index) = indices.borrow().get(row.index() as usize).copied() {
                    let _ = tx.send(BrowserEvent::TrackActivated(index));
                }
            });
        }

        let queue_title = gtk::Label::new(Some("BIBLIOTECA LOCAL"));
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

        let albums_flow = collection_flow();
        {
            let tx = event_tx.clone();
            let names = album_names.clone();
            albums_flow.connect_child_activated(move |_, child| {
                if let Some(name) = names.borrow().get(child.index() as usize).cloned() {
                    let _ = tx.send(BrowserEvent::Navigate(BrowserRoute::Album(name)));
                }
            });
        }
        let albums_page = collection_page(
            "ÁLBUNS",
            "Organizados automaticamente pelas tags dos arquivos",
            &albums_flow,
        );

        let artists_flow = collection_flow();
        {
            let tx = event_tx.clone();
            let names = artist_names.clone();
            artists_flow.connect_child_activated(move |_, child| {
                if let Some(name) = names.borrow().get(child.index() as usize).cloned() {
                    let _ = tx.send(BrowserEvent::Navigate(BrowserRoute::Artist(name)));
                }
            });
        }
        let artists_page = collection_page(
            "ARTISTAS",
            "Explore sua biblioteca por artista",
            &artists_flow,
        );

        let playlist_model = gtk::StringList::new(&[]);
        let playlist_dropdown = gtk::DropDown::builder()
            .model(&playlist_model)
            .hexpand(true)
            .build();
        let playlist_entry = gtk::Entry::builder()
            .placeholder_text("Nome da nova playlist")
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

        let create_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        create_row.append(&playlist_entry);
        create_row.append(&create_button);

        let add_button = gtk::Button::with_label("Adicionar faixa atual");
        let remove_button = gtk::Button::with_label("Remover faixa atual");
        let delete_button = gtk::Button::with_label("Excluir playlist");
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

        let playlist_select_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        playlist_select_row.append(&playlist_dropdown);
        playlist_select_row.append(&delete_button);

        let action_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        action_row.append(&add_button);
        action_row.append(&remove_button);

        let playlists_list = gtk::ListBox::new();
        playlists_list.set_selection_mode(gtk::SelectionMode::Single);
        playlists_list.add_css_class("playlist-list");
        {
            let tx = event_tx.clone();
            let names = playlist_row_names.clone();
            playlists_list.connect_row_activated(move |_, row| {
                if let Some(name) = names.borrow().get(row.index() as usize).cloned() {
                    let _ = tx.send(BrowserEvent::Navigate(BrowserRoute::Playlist(name)));
                }
            });
        }

        let playlists_scroll = gtk::ScrolledWindow::new();
        playlists_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        playlists_scroll.set_vexpand(true);
        playlists_scroll.set_child(Some(&playlists_list));

        let playlists_header = page_header(
            "PLAYLISTS",
            "Crie coleções e adicione a faixa que estiver tocando",
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
        root.add_named(&tracks_page, Some("tracks"));
        root.add_named(&albums_page, Some("albums"));
        root.add_named(&artists_page, Some("artists"));
        root.add_named(&playlists_page, Some("playlists"));
        root.set_visible_child_name("tracks");

        Self {
            root,
            queue,
            queue_title,
            albums_flow,
            artists_flow,
            playlists_list,
            playlist_model,
            playlist_dropdown,
            route: RefCell::new(BrowserRoute::All),
            visible_indices,
            album_names,
            artist_names,
            playlist_names,
            playlist_row_names,
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
        query: &str,
    ) {
        self.route.replace(route);
        self.refresh(tracks, config, query);
    }

    pub fn refresh(&self, tracks: &[Track], config: &AppConfig, query: &str) {
        match self.route() {
            BrowserRoute::Albums => {
                self.rebuild_albums(tracks, query);
                self.root.set_visible_child_name("albums");
            }
            BrowserRoute::Artists => {
                self.rebuild_artists(tracks, query);
                self.root.set_visible_child_name("artists");
            }
            BrowserRoute::Playlists => {
                self.rebuild_playlists(config, query);
                self.root.set_visible_child_name("playlists");
            }
            route => {
                self.rebuild_queue(tracks, config, query, &route);
                self.root.set_visible_child_name("tracks");
            }
        }
    }

    pub fn try_recv(&self) -> Option<BrowserEvent> {
        self.events.try_recv().ok()
    }

    pub fn visible_indices(&self) -> Vec<usize> {
        self.visible_indices.borrow().clone()
    }

    pub fn select_track(&self, index: usize) {
        if let Some(position) = self
            .visible_indices
            .borrow()
            .iter()
            .position(|visible| *visible == index)
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
        query: &str,
        route: &BrowserRoute,
    ) {
        clear_list_box(&self.queue);
        let query = query.trim().to_lowercase();

        let mut candidates = match route {
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
            _ => (0..tracks.len()).collect::<Vec<_>>(),
        };

        match route {
            BrowserRoute::Playlist(_) => {
                // A playlist is an ordered collection. Never replace its insertion order
                // with the global library order.
            }
            BrowserRoute::Album(_) => candidates.sort_by(|left, right| {
                compare_album_tracks(&tracks[*left], &tracks[*right])
            }),
            BrowserRoute::Artist(_) => candidates.sort_by(|left, right| {
                compare_artist_tracks(&tracks[*left], &tracks[*right])
            }),
            _ => candidates.sort_by(|left, right| {
                compare_library_tracks(&tracks[*left], &tracks[*right])
            }),
        }

        self.queue_title.set_text(&route_title(route));
        let mut visible = Vec::new();

        for index in candidates {
            let track = &tracks[index];
            if !query.is_empty() {
                let haystack = format!("{} {} {}", track.title, track.artist, track.album)
                    .to_lowercase();
                if !haystack.contains(&query) {
                    continue;
                }
            }

            visible.push(index);
            self.queue.append(&track_row(
                visible.len(),
                track,
                config.is_liked(&track.path),
            ));
        }

        if visible.is_empty() {
            self.queue.append(&empty_row(match route {
                BrowserRoute::Liked => "Nenhuma música curtida ainda",
                BrowserRoute::Playlist(_) => "Esta playlist ainda está vazia",
                _ => "Nenhuma faixa encontrada",
            }));
        }
        self.visible_indices.replace(visible);
    }

    fn rebuild_albums(&self, tracks: &[Track], query: &str) {
        clear_flow_box(&self.albums_flow);
        let query = query.trim().to_lowercase();
        let mut groups: BTreeMap<String, Vec<&Track>> = BTreeMap::new();
        for track in tracks {
            groups.entry(track.album.clone()).or_default().push(track);
        }

        let mut names = Vec::new();
        for (album, album_tracks) in groups {
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
            let cover = album_tracks.iter().find_map(|track| track.cover_path.as_deref());
            self.albums_flow.append(&collection_card(
                cover,
                &album,
                &artists,
                &format!("{} faixas", album_tracks.len()),
            ));
            names.push(album);
        }
        self.album_names.replace(names);
    }

    fn rebuild_artists(&self, tracks: &[Track], query: &str) {
        clear_flow_box(&self.artists_flow);
        let query = query.trim().to_lowercase();
        let mut groups: BTreeMap<String, Vec<&Track>> = BTreeMap::new();
        for track in tracks {
            groups.entry(track.artist.clone()).or_default().push(track);
        }

        let mut names = Vec::new();
        for (artist, artist_tracks) in groups {
            if !query.is_empty() && !artist.to_lowercase().contains(&query) {
                continue;
            }
            let albums = artist_tracks
                .iter()
                .map(|track| track.album.as_str())
                .collect::<BTreeSet<_>>()
                .len();
            let cover = artist_tracks.iter().find_map(|track| track.cover_path.as_deref());
            self.artists_flow.append(&collection_card(
                cover,
                &artist,
                &format!("{albums} álbuns"),
                &format!("{} faixas", artist_tracks.len()),
            ));
            names.push(artist);
        }
        self.artist_names.replace(names);
    }

    fn rebuild_playlists(&self, config: &AppConfig, query: &str) {
        clear_list_box(&self.playlists_list);
        let query = query.trim().to_lowercase();
        let previous = self.playlist_dropdown.selected() as usize;

        while self.playlist_model.n_items() > 0 {
            self.playlist_model.remove(0);
        }

        let mut all_names = Vec::new();
        let mut row_names = Vec::new();
        for playlist in &config.playlists {
            self.playlist_model.append(&playlist.name);
            all_names.push(playlist.name.clone());
            if !query.is_empty() && !playlist.name.to_lowercase().contains(&query) {
                continue;
            }
            self.playlists_list.append(&playlist_row(
                &playlist.name,
                playlist.tracks.len(),
            ));
            row_names.push(playlist.name.clone());
        }
        self.playlist_names.replace(all_names);
        self.playlist_row_names.replace(row_names);

        let count = self.playlist_model.n_items();
        if count > 0 {
            self.playlist_dropdown
                .set_selected((previous.min(count as usize - 1)) as u32);
        }
        self.playlist_dropdown.set_sensitive(count > 0);
    }
}

fn compare_library_tracks(left: &Track, right: &Track) -> Ordering {
    compare_text(&left.artist, &right.artist)
        .then_with(|| compare_text(&left.album, &right.album))
        .then_with(|| compare_album_tracks(left, right))
}

fn compare_artist_tracks(left: &Track, right: &Track) -> Ordering {
    compare_text(&left.album, &right.album)
        .then_with(|| compare_album_tracks(left, right))
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

fn compare_text(left: &str, right: &str) -> Ordering {
    left.to_lowercase().cmp(&right.to_lowercase())
}

fn collection_flow() -> gtk::FlowBox {
    let flow = gtk::FlowBox::new();
    flow.set_selection_mode(gtk::SelectionMode::None);
    flow.set_activate_on_single_click(true);
    flow.set_min_children_per_line(1);
    flow.set_max_children_per_line(5);
    flow.set_column_spacing(14);
    flow.set_row_spacing(14);
    flow.set_homogeneous(true);
    flow.set_valign(gtk::Align::Start);
    flow
}

fn collection_page(title: &str, subtitle: &str, flow: &gtk::FlowBox) -> gtk::Box {
    let header = page_header(title, subtitle);
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.set_vexpand(true);
    scroll.set_child(Some(flow));

    let page = gtk::Box::new(gtk::Orientation::Vertical, 14);
    page.set_hexpand(true);
    page.set_vexpand(true);
    page.add_css_class("library-panel");
    page.append(&header);
    page.append(&scroll);
    page
}

fn page_header(title: &str, subtitle: &str) -> gtk::Box {
    let title_label = gtk::Label::new(Some(title));
    title_label.set_xalign(0.0);
    title_label.add_css_class("section-title");
    let subtitle_label = gtk::Label::new(Some(subtitle));
    subtitle_label.set_xalign(0.0);
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
    detail: &str,
) -> gtk::Box {
    let artwork = artwork(cover_path, 132);
    let title_label = gtk::Label::new(Some(title));
    title_label.set_xalign(0.0);
    title_label.set_single_line_mode(true);
    title_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title_label.add_css_class("collection-card-title");
    let subtitle_label = gtk::Label::new(Some(subtitle));
    subtitle_label.set_xalign(0.0);
    subtitle_label.set_single_line_mode(true);
    subtitle_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    subtitle_label.add_css_class("dim-label");
    let detail_label = gtk::Label::new(Some(detail));
    detail_label.set_xalign(0.0);
    detail_label.add_css_class("time-label");

    let card = gtk::Box::new(gtk::Orientation::Vertical, 7);
    card.set_size_request(154, 205);
    card.add_css_class("collection-card");
    card.append(&artwork);
    card.append(&title_label);
    card.append(&subtitle_label);
    card.append(&detail_label);
    card
}

fn artwork(path: Option<&Path>, size: i32) -> gtk::Stack {
    let placeholder = gtk::Image::from_icon_name("folder-music-symbolic");
    placeholder.set_pixel_size(size / 3);
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
        match gdk_pixbuf::Pixbuf::from_file_at_scale(path, size, size, false) {
            Ok(pixbuf) => {
                let texture = gdk::Texture::for_pixbuf(&pixbuf);
                picture.set_paintable(Some(&texture));
                stack.set_visible_child_name("picture");
            }
            Err(_) => stack.set_visible_child_name("placeholder"),
        }
    } else {
        stack.set_visible_child_name("placeholder");
    }
    stack
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

    let favorite = gtk::Image::from_icon_name(if liked {
        "starred-symbolic"
    } else {
        "non-starred-symbolic"
    });
    favorite.set_opacity(if liked { 0.9 } else { 0.22 });

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
    content.append(&favorite);
    content.append(&lyric_status);
    content.append(&duration);

    let row = gtk::ListBoxRow::new();
    row.set_child(Some(&content));
    row
}

fn playlist_row(name: &str, count: usize) -> gtk::ListBoxRow {
    let icon = gtk::Image::from_icon_name("view-list-symbolic");
    icon.set_pixel_size(24);
    let title = gtk::Label::new(Some(name));
    title.set_xalign(0.0);
    title.set_hexpand(true);
    title.add_css_class("track-title");
    let count = gtk::Label::new(Some(&format!("{count} faixas")));
    count.add_css_class("dim-label");
    let arrow = gtk::Image::from_icon_name("go-next-symbolic");

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(12);
    content.set_margin_end(12);
    content.append(&icon);
    content.append(&title);
    content.append(&count);
    content.append(&arrow);

    let row = gtk::ListBoxRow::new();
    row.set_child(Some(&content));
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

fn route_title(route: &BrowserRoute) -> String {
    match route {
        BrowserRoute::All => "BIBLIOTECA LOCAL".to_string(),
        BrowserRoute::Liked => "MÚSICAS CURTIDAS".to_string(),
        BrowserRoute::Album(name) => format!("ÁLBUM · {name}"),
        BrowserRoute::Artist(name) => format!("ARTISTA · {name}"),
        BrowserRoute::Playlist(name) => format!("PLAYLIST · {name}"),
        _ => "BIBLIOTECA LOCAL".to_string(),
    }
}

fn clear_list_box(list: &gtk::ListBox) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
}

fn clear_flow_box(flow: &gtk::FlowBox) {
    while let Some(child) = flow.first_child() {
        flow.remove(&child);
    }
}

fn format_duration(seconds: u64) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

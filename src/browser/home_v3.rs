//! YouTube Home V3 rendering helpers.
//!
//! This module was mechanically extracted from `browser.rs` to reduce the
//! browser surface before deeper cleanup. Keep behavior changes out of this
//! file move.

use super::{
    empty_row, home_card_button, metrolist_home_section_content, page_header, BrowserEvent,
    BrowserPlaybackState, HomeCard, HomeSectionPresentation,
};
use crate::{
    config::{AppConfig, AppLanguage},
    ui::widgets::MaterialLoadingIndicator,
    youtube::{cached_cover_for_item, HomeV3Item, HomeV3Page, YouTubeItem},
};
use gtk::prelude::*;
use std::{
    path::{Path, PathBuf},
    sync::mpsc::Sender,
};

#[derive(Clone, Copy)]
struct HomeV3Copy {
    eyebrow: &'static str,
    subtitle: &'static str,
    loading_text: &'static str,
    empty_text: &'static str,
    untitled_section: &'static str,
    continuation_text: &'static str,
}

fn home_v3_copy(language: AppLanguage) -> HomeV3Copy {
    match language {
        AppLanguage::Portuguese => HomeV3Copy {
            eyebrow: "YOUTUBE MUSIC",
            subtitle: "Recomendações, playlists e músicas do YouTube Music",
            loading_text: "Carregando feed do YouTube Music…",
            empty_text: "Nenhuma recomendação encontrada no momento.",
            untitled_section: "Recomendações",
            continuation_text: "Carregar mais recomendações",
        },
        AppLanguage::English => HomeV3Copy {
            eyebrow: "YOUTUBE MUSIC",
            subtitle: "Recommendations, playlists and music from YouTube Music",
            loading_text: "Loading YouTube Music feed…",
            empty_text: "No recommendations found right now.",
            untitled_section: "Recommendations",
            continuation_text: "Load more recommendations",
        },
        AppLanguage::Spanish => HomeV3Copy {
            eyebrow: "YOUTUBE MUSIC",
            subtitle: "Recomendaciones, playlists y música de YouTube Music",
            loading_text: "Cargando feed de YouTube Music…",
            empty_text: "No se encontraron recomendaciones por ahora.",
            untitled_section: "Recomendaciones",
            continuation_text: "Cargar más recomendaciones",
        },
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HomeV3CardPresentation {
    Featured,
    Compact,
    TrackRows,
}

pub(super) fn home_v3_section_presentation(
    section_index: usize,
    section_title: &str,
    items: &[HomeV3Item],
) -> HomeV3CardPresentation {
    let title = section_title.trim().to_lowercase();

    if items.iter().all(|item| {
        !item.video_id.trim().is_empty() && item.result_type.eq_ignore_ascii_case("song")
    }) && items.len() >= 6
    {
        return HomeV3CardPresentation::TrackRows;
    }

    if section_index == 0
        || title.contains("ouvir de novo")
        || title.contains("listen again")
        || title.contains("escuchar de nuevo")
    {
        HomeV3CardPresentation::Featured
    } else {
        HomeV3CardPresentation::Compact
    }
}

fn home_v3_item_cover_path(item: &HomeV3Item) -> Option<PathBuf> {
    let path = Path::new(item.cover_path.trim());
    if !item.cover_path.trim().is_empty() && path.is_file() {
        return Some(path.to_path_buf());
    }

    cached_cover_for_item(&home_v3_item_to_youtube_item(item))
}

fn home_v3_stable_hash(value: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(value, &mut hasher);
    std::hash::Hasher::finish(&hasher)
}

fn home_v3_item_identity(item: &HomeV3Item) -> String {
    if !item.video_id.trim().is_empty() {
        return format!("video:{}", item.video_id.trim());
    }

    if !item.browse_id.trim().is_empty() {
        return format!("browse:{}", item.browse_id.trim());
    }

    format!(
        "text:{}:{}:{}",
        item.result_type.trim(),
        item.title.trim(),
        item.subtitle.trim()
    )
}

pub(super) fn home_v3_section_signature(title: &str, items: &[HomeV3Item]) -> String {
    let mut raw = String::new();
    raw.push_str(title.trim());

    for item in items.iter().take(24) {
        raw.push('|');
        raw.push_str(&home_v3_item_identity(item));
    }

    format!("{:016x}", home_v3_stable_hash(&raw))
}

fn home_v3_page_signature(page: &HomeV3Page) -> String {
    let mut raw = String::new();
    raw.push_str("home-v3|selected=");
    raw.push_str(page.selected_chip_params.trim());

    for section in &page.sections {
        raw.push_str("|section=");
        raw.push_str(&home_v3_section_signature(&section.title, &section.items));
    }

    format!("youtube-home-v3:{:016x}", home_v3_stable_hash(&raw))
}

fn home_v3_presentation_to_home_section(
    presentation: HomeV3CardPresentation,
) -> HomeSectionPresentation {
    match presentation {
        HomeV3CardPresentation::Featured => HomeSectionPresentation::Featured,
        HomeV3CardPresentation::Compact => HomeSectionPresentation::Compact,
        HomeV3CardPresentation::TrackRows => HomeSectionPresentation::TrackRows,
    }
}

fn home_v3_item_to_home_card(
    item: &HomeV3Item,
    queue: &[YouTubeItem],
    playback_index: usize,
) -> HomeCard {
    let mut youtube_item = home_v3_item_to_youtube_item(item);
    if youtube_item.cover_path.trim().is_empty() {
        if let Some(path) = home_v3_item_cover_path(item) {
            youtube_item.cover_path = path.to_string_lossy().to_string();
        }
    }

    if !item.video_id.trim().is_empty() {
        return HomeCard::YouTubeTrack {
            item: youtube_item,
            queue: queue.to_vec(),
            index: playback_index,
        };
    }

    if item.result_type.eq_ignore_ascii_case("artist") {
        return HomeCard::YouTubeArtist {
            item: youtube_item,
            subtitle: item.subtitle.clone(),
            detail: "YouTube Music".to_string(),
            cover_path: home_v3_item_cover_path(item),
        };
    }

    if item.result_type.eq_ignore_ascii_case("album") {
        return HomeCard::YouTubeAlbum {
            item: youtube_item,
            subtitle: if item.subtitle.trim().is_empty() {
                item.artist.clone()
            } else {
                item.subtitle.clone()
            },
            detail: "YouTube Music".to_string(),
            cover_path: home_v3_item_cover_path(item),
        };
    }

    HomeCard::YouTubePlaylist(youtube_item)
}

fn home_v3_section_cards(items: &[HomeV3Item]) -> Vec<HomeCard> {
    let queue = items
        .iter()
        .filter(|item| !item.video_id.trim().is_empty())
        .map(home_v3_item_to_youtube_item)
        .collect::<Vec<_>>();

    items
        .iter()
        .take(12)
        .enumerate()
        .map(|(index, item)| {
            let playback_index = queue
                .iter()
                .position(|candidate| candidate.video_id == item.video_id)
                .unwrap_or(index);

            home_v3_item_to_home_card(item, &queue, playback_index)
        })
        .collect()
}

#[expect(
    clippy::too_many_arguments,
    reason = "Home V3 reconciles the native source with the shared Home card renderer"
)]
pub(super) fn home_v3_existing_card_section_content(
    items: &[HomeV3Item],
    presentation: HomeV3CardPresentation,
    empty_detail: &str,
    playback: &BrowserPlaybackState,
    config: &AppConfig,
    event_tx: &Sender<BrowserEvent>,
    language: AppLanguage,
    card_effects: bool,
) -> gtk::ScrolledWindow {
    let home_presentation = home_v3_presentation_to_home_section(presentation);

    let cards = home_v3_section_cards(items)
        .into_iter()
        .map(|card| {
            home_card_button(
                card,
                home_presentation,
                playback,
                config,
                event_tx,
                language,
                card_effects,
            )
        })
        .collect::<Vec<_>>();

    metrolist_home_section_content(cards, home_presentation, language, empty_detail)
}

// Transitional bridge: this is the Home V3 feed shell fed by a HomeV3Page contract.
// The caller may still create that contract from the legacy YouTubeHomePage
// source until the native Home V3 helper/parser is wired.
#[expect(
    clippy::too_many_arguments,
    reason = "Home V3 shell bridges feed data, playback state and renderer dependencies"
)]
pub(super) fn youtube_home_v3_feed_shell(
    page: &HomeV3Page,
    loading: bool,
    playback: &BrowserPlaybackState,
    config: &AppConfig,
    event_tx: &Sender<BrowserEvent>,
    language: AppLanguage,
    card_effects: bool,
    existing_home: Option<&gtk::Box>,
) -> gtk::Box {
    let copy = home_v3_copy(language);

    let page_signature = home_v3_page_signature(page);
    if !loading {
        if let Some(existing_home) = existing_home {
            if existing_home.has_css_class("youtube-home-v3")
                && existing_home.widget_name().as_str() == page_signature
            {
                return existing_home.clone();
            }
        }
    }

    let home = gtk::Box::new(gtk::Orientation::Vertical, 22);
    home.set_hexpand(true);
    home.set_vexpand(false);
    home.add_css_class("library-home");
    home.add_css_class("expressive-library-home");
    home.set_widget_name(&page_signature);
    home.add_css_class("youtube-home-v3");
    home.add_css_class("youtube-home-v3-feed");

    home.append(&page_header(copy.eyebrow, copy.subtitle));

    if !page.chips.is_empty() {
        let chip_section = gtk::Box::new(gtk::Orientation::Vertical, 8);
        chip_section.add_css_class("home-section");
        chip_section.add_css_class("youtube-home-chip-section");
        chip_section.add_css_class("youtube-home-v3-chips");

        let chips = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        chips.add_css_class("youtube-chip-row");
        chips.set_margin_top(4);
        chips.set_margin_start(2);
        chips.set_margin_end(28);
        chips.set_margin_bottom(28);

        for chip in &page.chips {
            let label = if chip.title.trim().is_empty() {
                match language {
                    AppLanguage::Portuguese => "Filtro",
                    AppLanguage::English => "Filter",
                    AppLanguage::Spanish => "Filtro",
                }
            } else {
                chip.title.trim()
            };

            let button = gtk::Button::with_label(label);
            button.add_css_class("pill");
            button.add_css_class("youtube-home-v3-chip");
            if chip.params == page.selected_chip_params {
                button.add_css_class("suggested-action");
            }

            let tx = event_tx.clone();
            let params = chip.params.clone();
            button.connect_clicked(move |_| {
                let _ = tx.send(BrowserEvent::LoadYouTubeHome {
                    continuation: String::new(),
                    params: params.clone(),
                });
            });

            chips.append(&button);
        }

        let scroll = gtk::ScrolledWindow::new();
        scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
        scroll.set_overlay_scrolling(false);
        scroll.set_hexpand(true);
        scroll.set_min_content_height(88);
        scroll.set_propagate_natural_height(true);
        scroll.set_child(Some(&chips));
        scroll.add_css_class("home-carousel-scroll");
        scroll.add_css_class("youtube-chip-scroll");
        scroll.add_css_class("youtube-home-v3-chip-scroll");

        chip_section.append(&scroll);
        home.append(&chip_section);
    }

    if loading {
        let loading_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        loading_row.set_hexpand(true);
        loading_row.add_css_class("youtube-home-v3-loading");

        let indicator = MaterialLoadingIndicator::with_size(20);

        let label = gtk::Label::new(Some(copy.loading_text));
        label.set_xalign(0.0);
        label.set_hexpand(true);

        loading_row.append(indicator.widget());
        loading_row.append(&label);
        home.append(&loading_row);
    }

    if page.sections.is_empty() {
        home.append(&empty_row(copy.empty_text));
        return home;
    }

    for (section_index, section) in page.sections.iter().enumerate() {
        let presentation =
            home_v3_section_presentation(section_index, &section.title, &section.items);

        let section_box = gtk::Box::new(gtk::Orientation::Vertical, 10);
        section_box.set_hexpand(true);
        section_box.set_widget_name(&format!(
            "youtube-home-v3-section:{}",
            home_v3_section_signature(&section.title, &section.items)
        ));
        section_box.add_css_class("home-section");
        section_box.add_css_class("youtube-home-v3-section");
        match presentation {
            HomeV3CardPresentation::Featured => section_box.add_css_class("home-section-featured"),
            HomeV3CardPresentation::Compact => section_box.add_css_class("home-section-compact"),
            HomeV3CardPresentation::TrackRows => {
                section_box.add_css_class("home-section-trackrows")
            }
        }

        let section_title = if !section.title.trim().is_empty() {
            section.title.trim()
        } else {
            copy.untitled_section
        };

        let title = gtk::Label::new(Some(section_title));
        title.set_xalign(0.0);
        title.add_css_class("section-title");
        section_box.append(&title);

        if section.items.is_empty() {
            section_box.append(&empty_row(copy.empty_text));
            home.append(&section_box);
            continue;
        }

        let content = home_v3_existing_card_section_content(
            &section.items,
            presentation,
            copy.empty_text,
            playback,
            config,
            event_tx,
            language,
            card_effects,
        );
        content.set_vexpand(false);
        content.set_valign(gtk::Align::Start);

        section_box.append(&content);
        home.append(&section_box);
    }

    if !page.continuation.trim().is_empty() {
        let loading_label = match language {
            AppLanguage::Portuguese => "Carregando…",
            AppLanguage::English => "Loading…",
            AppLanguage::Spanish => "Cargando…",
        };

        let button = gtk::Button::with_label(copy.continuation_text);
        button.set_halign(gtk::Align::Center);
        button.add_css_class("pill");
        button.add_css_class("suggested-action");
        button.add_css_class("youtube-home-load-more");
        button.add_css_class("youtube-home-v3-continuation");

        let tx = event_tx.clone();
        let continuation = page.continuation.clone();
        let params = page.selected_chip_params.clone();
        button.connect_clicked(move |button| {
            button.set_label(loading_label);
            button.set_sensitive(false);
            let _ = tx.send(BrowserEvent::LoadYouTubeHome {
                continuation: continuation.clone(),
                params: params.clone(),
            });
        });

        home.append(&button);
    }

    home
}

fn home_v3_item_to_youtube_item(item: &HomeV3Item) -> YouTubeItem {
    YouTubeItem {
        result_type: item.result_type.clone(),
        title: item.title.clone(),
        subtitle: item.subtitle.clone(),
        video_id: item.video_id.clone(),
        browse_id: item.browse_id.clone(),
        album: item.album.clone(),
        artist: item.artist.clone(),
        playlist_kind: item.playlist_kind.clone(),
        params: item.params.clone(),
        duration_seconds: item.duration_seconds,
        thumbnail_url: item.thumbnail_url.clone(),
        cover_path: item.cover_path.clone(),
    }
}

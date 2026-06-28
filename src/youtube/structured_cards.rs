use super::{
    feed::YouTubeHomeSection, youtube_item_action, YouTubeItem, YouTubeItemAction, YouTubePageEvent,
};
use gtk::prelude::*;
use std::{rc::Rc, sync::mpsc::Sender};

const CARD_WIDTH: i32 = 176;
const ARTWORK_SIZE: i32 = 152;
const CAROUSEL_HEIGHT: i32 = 238;

pub(crate) fn uses_card_carousel(layout: &str) -> bool {
    matches!(
        layout.trim().to_ascii_lowercase().as_str(),
        "carousel" | "mixed" | "quick_picks"
    )
}

pub(crate) fn youtube_carousel_row(
    section: &YouTubeHomeSection,
    playable_queue: Rc<Vec<YouTubeItem>>,
    event_tx: Sender<YouTubePageEvent>,
) -> gtk::ListBoxRow {
    let cards = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    cards.set_margin_top(6);
    cards.set_margin_bottom(12);
    cards.set_margin_start(12);
    cards.set_margin_end(12);

    for item in &section.items {
        cards.append(&youtube_card(
            item,
            Rc::clone(&playable_queue),
            event_tx.clone(),
        ));
    }

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
    scroll.set_propagate_natural_height(true);
    scroll.set_min_content_height(CAROUSEL_HEIGHT);
    scroll.set_child(Some(&cards));
    scroll.add_css_class("youtube-section-carousel");

    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    row.set_selectable(false);
    row.set_child(Some(&scroll));
    row
}

fn youtube_card(
    item: &YouTubeItem,
    playable_queue: Rc<Vec<YouTubeItem>>,
    event_tx: Sender<YouTubePageEvent>,
) -> gtk::Button {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
    content.set_halign(gtk::Align::Fill);
    content.set_valign(gtk::Align::Start);
    content.set_margin_top(10);
    content.set_margin_bottom(10);
    content.set_margin_start(10);
    content.set_margin_end(10);

    if let Some(path) = item.cached_cover() {
        let artwork = gtk::Picture::for_filename(path);
        artwork.set_content_fit(gtk::ContentFit::Cover);
        artwork.set_can_shrink(true);
        artwork.set_size_request(ARTWORK_SIZE, ARTWORK_SIZE);
        artwork.add_css_class("youtube-card-artwork");
        if item.result_type == "artist" {
            artwork.add_css_class("circular");
        }
        content.append(&artwork);
    } else {
        let placeholder = gtk::Box::new(gtk::Orientation::Vertical, 0);
        placeholder.set_size_request(ARTWORK_SIZE, ARTWORK_SIZE);
        placeholder.set_halign(gtk::Align::Center);
        placeholder.set_valign(gtk::Align::Center);
        placeholder.add_css_class("card");
        placeholder.add_css_class("youtube-card-placeholder");

        let icon = gtk::Image::from_icon_name(icon_name(item));
        icon.set_pixel_size(44);
        icon.set_halign(gtk::Align::Center);
        icon.set_valign(gtk::Align::Center);
        placeholder.append(&icon);
        content.append(&placeholder);
    }

    let title = gtk::Label::new(Some(&item.title));
    title.set_xalign(0.0);
    title.set_wrap(true);
    title.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    title.set_max_width_chars(22);
    title.set_lines(2);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title.add_css_class("heading");
    content.append(&title);

    if !item.subtitle.trim().is_empty() {
        let subtitle = gtk::Label::new(Some(&item.subtitle));
        subtitle.set_xalign(0.0);
        subtitle.set_wrap(true);
        subtitle.set_max_width_chars(22);
        subtitle.set_lines(2);
        subtitle.set_ellipsize(gtk::pango::EllipsizeMode::End);
        subtitle.add_css_class("dim-label");
        content.append(&subtitle);
    }

    let button = gtk::Button::new();
    button.set_width_request(CARD_WIDTH);
    button.set_valign(gtk::Align::Start);
    button.set_child(Some(&content));
    button.add_css_class("card");
    button.add_css_class("youtube-section-card");

    let accessible_hint = if item.subtitle.trim().is_empty() {
        item.title.clone()
    } else {
        format!("{} — {}", item.title, item.subtitle)
    };
    button.set_tooltip_text(Some(&accessible_hint));

    let item = item.clone();
    button.connect_clicked(move |_| {
        dispatch_item_action(&event_tx, item.clone(), playable_queue.as_ref());
    });

    button
}

fn dispatch_item_action(
    event_tx: &Sender<YouTubePageEvent>,
    item: YouTubeItem,
    playable_queue: &[YouTubeItem],
) {
    match youtube_item_action(&item) {
        YouTubeItemAction::Continue => {
            let _ = event_tx.send(YouTubePageEvent::LoadHome {
                continuation: item.params.clone(),
            });
        }
        YouTubeItemAction::Play => {
            let queue = playable_queue.to_vec();
            let index = queue
                .iter()
                .position(|candidate| candidate.video_id == item.video_id)
                .unwrap_or(0);
            let _ = event_tx.send(YouTubePageEvent::Activate { item, queue, index });
        }
        YouTubeItemAction::OpenPlaylist => {
            let _ = event_tx.send(YouTubePageEvent::OpenPlaylist(item));
        }
        YouTubeItemAction::OpenCollection => {
            let _ = event_tx.send(YouTubePageEvent::OpenCollection(item));
        }
        YouTubeItemAction::Unsupported => {
            let _ = event_tx.send(YouTubePageEvent::UnsupportedItem {
                title: item.title,
                result_type: item.result_type,
            });
        }
        YouTubeItemAction::Ignore => {}
    }
}

fn icon_name(item: &YouTubeItem) -> &'static str {
    match item.result_type.as_str() {
        "playlist" => "view-list-symbolic",
        "album" => "media-optical-symbolic",
        "artist" => "avatar-default-symbolic",
        "video" | "episode" => "video-x-generic-symbolic",
        "podcast" => "audio-speakers-symbolic",
        _ => "audio-x-generic-symbolic",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_collection_or_mixed_layouts_use_cards() {
        assert!(uses_card_carousel("carousel"));
        assert!(uses_card_carousel(" MIXED "));
        assert!(!uses_card_carousel("list"));
        assert!(uses_card_carousel("quick_picks"));
    }

    #[test]
    fn card_icon_matches_collection_kind() {
        let album = YouTubeItem {
            result_type: "album".to_string(),
            ..YouTubeItem::default()
        };
        let artist = YouTubeItem {
            result_type: "artist".to_string(),
            ..YouTubeItem::default()
        };
        assert_eq!(icon_name(&album), "media-optical-symbolic");
        assert_eq!(icon_name(&artist), "avatar-default-symbolic");
    }
}

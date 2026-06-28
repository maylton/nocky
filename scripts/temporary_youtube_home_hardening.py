#!/usr/bin/env python3
from pathlib import Path
import re


def replace(path: str, old: str, new: str, expected: int = 1) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != expected:
        raise SystemExit(
            f"{path}: expected {expected} occurrence(s), found {count}: {old[:100]!r}"
        )
    file.write_text(text.replace(old, new), encoding="utf-8")


def regex_replace(path: str, pattern: str, repl, expected: int) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    updated, count = re.subn(pattern, repl, text, flags=re.MULTILINE)
    if count != expected:
        raise SystemExit(
            f"{path}: expected {expected} regex replacement(s), found {count}: {pattern}"
        )
    file.write_text(updated, encoding="utf-8")


# Reject stale asynchronous YouTube Home responses.
replace(
    "src/background.rs",
    "    YouTubeStructuredPage {\n        title: String,\n",
    "    YouTubeStructuredPage {\n        request_id: u64,\n        title: String,\n",
)
replace(
    "src/background.rs",
    "}\n\npub(crate) struct BackgroundChannel {",
    "}\n\npub(crate) fn youtube_home_response_is_current(\n"
    "    home: bool,\n"
    "    request_id: u64,\n"
    "    current_request_id: u64,\n"
    ") -> bool {\n"
    "    !home || request_id == current_request_id\n"
    "}\n\n"
    "pub(crate) struct BackgroundChannel {",
)
with Path("src/background.rs").open("a", encoding="utf-8") as file:
    file.write(
        "\n#[cfg(test)]\n"
        "mod tests {\n"
        "    use super::youtube_home_response_is_current;\n\n"
        "    #[test]\n"
        "    fn rejects_stale_home_responses_but_accepts_non_home_pages() {\n"
        "        assert!(youtube_home_response_is_current(true, 7, 7));\n"
        "        assert!(!youtube_home_response_is_current(true, 6, 7));\n"
        "        assert!(youtube_home_response_is_current(false, 0, 7));\n"
        "    }\n"
        "}\n"
    )

replace(
    "src/app/controller/mod.rs",
    "    pub(crate) youtube_search_request_id: Cell<u64>,\n",
    "    pub(crate) youtube_search_request_id: Cell<u64>,\n"
    "    pub(crate) youtube_home_request_id: Cell<u64>,\n",
)
replace(
    "src/app/controller/construction.rs",
    "                youtube_search_request_id: Cell::new(0),\n",
    "                youtube_search_request_id: Cell::new(0),\n"
    "                youtube_home_request_id: Cell::new(0),\n",
)
replace(
    "src/app/controller/youtube.rs",
    "        let append = !continuation.is_empty();\n"
    "        let filtered = !params.is_empty();\n",
    "        let append = !continuation.is_empty();\n"
    "        let filtered = !params.is_empty();\n"
    "        let request_id = self.youtube_home_request_id.get().wrapping_add(1);\n"
    "        self.youtube_home_request_id.set(request_id);\n",
)
replace(
    "src/app/controller/youtube.rs",
    "            let _ = sender.send(BackgroundMessage::YouTubeStructuredPage {\n"
    "                title: \"Para você\".to_string(),\n"
    "                home: true,\n",
    "            let _ = sender.send(BackgroundMessage::YouTubeStructuredPage {\n"
    "                request_id,\n"
    "                title: \"Para você\".to_string(),\n"
    "                home: true,\n",
)
regex_replace(
    "src/app/controller/youtube.rs",
    r"(BackgroundMessage::YouTubeStructuredPage \{\n)"
    r"(?P<indent>[ \t]+)"
    r"(?P<title>title: [^\n]+\n)"
    r"(?P=indent)home: false,",
    lambda match: (
        f"{match.group(1)}{match.group('indent')}request_id: 0,\n"
        f"{match.group('indent')}{match.group('title')}"
        f"{match.group('indent')}home: false,"
    ),
    3,
)

replace(
    "src/app/controller/background.rs",
    "    background::BackgroundMessage,\n",
    "    background::{youtube_home_response_is_current, BackgroundMessage},\n",
)
replace(
    "src/app/controller/background.rs",
    "                BackgroundMessage::YouTubeStructuredPage {\n"
    "                    title,\n"
    "                    home,\n"
    "                    append,\n"
    "                    result,\n"
    "                } => match result {",
    "                BackgroundMessage::YouTubeStructuredPage {\n"
    "                    request_id,\n"
    "                    title,\n"
    "                    home,\n"
    "                    append,\n"
    "                    result,\n"
    "                } if youtube_home_response_is_current(\n"
    "                    home,\n"
    "                    request_id,\n"
    "                    self.youtube_home_request_id.get(),\n"
    "                ) => match result {",
)
replace(
    "src/app/controller/background.rs",
    "                    Err(error) => self.youtube_page.show_error(&error),\n"
    "                },\n"
    "                BackgroundMessage::YouTubeRecoveryRetry {",
    "                    Err(error) => self.youtube_page.show_error(&error),\n"
    "                },\n"
    "                BackgroundMessage::YouTubeStructuredPage { .. } => {},\n"
    "                BackgroundMessage::YouTubeRecoveryRetry {",
)

# Reuse the feed model as the single source of truth for section queues.
replace(
    "src/youtube/feed.rs",
    "impl YouTubeHomePage {\n",
    "impl YouTubeHomeSection {\n"
    "    pub fn playable_queue(&self) -> Vec<YouTubeItem> {\n"
    "        self.items\n"
    "            .iter()\n"
    "            .filter(|item| item.playable())\n"
    "            .cloned()\n"
    "            .collect()\n"
    "    }\n"
    "}\n\n"
    "impl YouTubeHomePage {\n",
)
replace(
    "src/youtube/feed.rs",
    "    #[test]\n"
    "    fn deserializes_versioned_contract() {",
    "    #[test]\n"
    "    fn playable_queue_preserves_section_order() {\n"
    "        let section = YouTubeHomeSection {\n"
    "            items: vec![\n"
    "                item(\"one\", \"One\"),\n"
    "                YouTubeItem {\n"
    "                    result_type: \"album\".to_string(),\n"
    "                    browse_id: \"MPRE\".to_string(),\n"
    "                    ..YouTubeItem::default()\n"
    "                },\n"
    "                item(\"two\", \"Two\"),\n"
    "            ],\n"
    "            ..YouTubeHomeSection::default()\n"
    "        };\n\n"
    "        let queue = section.playable_queue();\n"
    "        assert_eq!(\n"
    "            queue\n"
    "                .iter()\n"
    "                .map(|item| item.video_id.as_str())\n"
    "                .collect::<Vec<_>>(),\n"
    "            vec![\"one\", \"two\"]\n"
    "        );\n"
    "    }\n\n"
    "    #[test]\n"
    "    fn deserializes_versioned_contract() {",
)

# Carry the full playable section queue in every YouTube Home track card.
replace(
    "src/browser.rs",
    "    YouTubeTrack(YouTubeItem),\n",
    "    YouTubeTrack {\n"
    "        item: YouTubeItem,\n"
    "        queue: Vec<YouTubeItem>,\n"
    "        index: usize,\n"
    "    },\n",
)
replace(
    "src/browser.rs",
    "            Self::YouTubeTrack(item) => CollectionCardDescriptor {\n",
    "            Self::YouTubeTrack { item, .. } => CollectionCardDescriptor {\n",
)
replace(
    "src/browser.rs",
    "            Self::YouTubeTrack(item) => BrowserEvent::YouTubeTrackActivated {\n"
    "                item: item.clone(),\n"
    "                queue: vec![item.clone()],\n"
    "                index: 0,\n"
    "            },\n",
    "            Self::YouTubeTrack { item, queue, index } => {\n"
    "                BrowserEvent::YouTubeTrackActivated {\n"
    "                    item: item.clone(),\n"
    "                    queue: queue.clone(),\n"
    "                    index: *index,\n"
    "                }\n"
    "            },\n",
)
replace(
    "src/browser.rs",
    "            Self::YouTubeTrack(item) => format!(\n"
    "                \"youtube-track:{}:{}\",\n"
    "                item.video_id,\n"
    "                item.title.to_lowercase()\n"
    "            ),\n",
    "            Self::YouTubeTrack { item, .. } => format!(\n"
    "                \"youtube-track:{}:{}\",\n"
    "                item.video_id,\n"
    "                item.title.to_lowercase()\n"
    "            ),\n",
)

# Localized copy for structured YouTube Home controls and metadata.
replace(
    "src/browser.rs",
    "    synchronized_playlist: &'static str,\n",
    "    synchronized_playlist: &'static str,\n"
    "    youtube_all: &'static str,\n"
    "    youtube_album: &'static str,\n"
    "    youtube_artist: &'static str,\n"
    "    youtube_load_more: &'static str,\n",
)
replace(
    "src/browser.rs",
    "            youtube_recommendation: \"Recomendação do YouTube Music\",\n"
    "            synchronized_playlist: \"Playlist sincronizada\",\n",
    "            youtube_recommendation: \"Recomendação do YouTube Music\",\n"
    "            synchronized_playlist: \"Playlist sincronizada\",\n"
    "            youtube_all: \"Tudo\",\n"
    "            youtube_album: \"Álbum\",\n"
    "            youtube_artist: \"Artista\",\n"
    "            youtube_load_more: \"Carregar mais recomendações\",\n",
)
replace(
    "src/browser.rs",
    "            youtube_recommendation: \"YouTube Music recommendation\",\n"
    "            synchronized_playlist: \"Synchronized playlist\",\n",
    "            youtube_recommendation: \"YouTube Music recommendation\",\n"
    "            synchronized_playlist: \"Synchronized playlist\",\n"
    "            youtube_all: \"All\",\n"
    "            youtube_album: \"Album\",\n"
    "            youtube_artist: \"Artist\",\n"
    "            youtube_load_more: \"Load more recommendations\",\n",
)
replace(
    "src/browser.rs",
    "            youtube_recommendation: \"Recomendación de YouTube Music\",\n"
    "            synchronized_playlist: \"Playlist sincronizada\",\n",
    "            youtube_recommendation: \"Recomendación de YouTube Music\",\n"
    "            synchronized_playlist: \"Playlist sincronizada\",\n"
    "            youtube_all: \"Todo\",\n"
    "            youtube_album: \"Álbum\",\n"
    "            youtube_artist: \"Artista\",\n"
    "            youtube_load_more: \"Cargar más recomendaciones\",\n",
)

replace(
    "src/browser.rs",
    "            next_home.append(&youtube_home_chip_bar(youtube_home_page, &self.event_tx));\n",
    "            next_home.append(&youtube_home_chip_bar(\n"
    "                youtube_home_page,\n"
    "                &self.event_tx,\n"
    "                language,\n"
    "            ));\n",
)
replace(
    "src/browser.rs",
    "                let cards = youtube_feed_section_cards(section);\n",
    "                let cards = youtube_feed_section_cards(section, language);\n",
)
replace(
    "src/browser.rs",
    "            }\n\n"
    "            let generation = self.home_generation.get().wrapping_add(1);\n",
    "            }\n\n"
    "            if !youtube_home_page.continuation.trim().is_empty() {\n"
    "                let load_more = gtk::Button::with_label(copy.youtube_load_more);\n"
    "                load_more.set_halign(gtk::Align::Center);\n"
    "                load_more.add_css_class(\"pill\");\n"
    "                load_more.add_css_class(\"suggested-action\");\n"
    "                let continuation = youtube_home_page.continuation.clone();\n"
    "                let params = youtube_home_page.selected_chip_params.clone();\n"
    "                let event_tx = self.event_tx.clone();\n"
    "                load_more.connect_clicked(move |_| {\n"
    "                    let _ = event_tx.send(BrowserEvent::LoadYouTubeHome {\n"
    "                        continuation: continuation.clone(),\n"
    "                        params: params.clone(),\n"
    "                    });\n"
    "                });\n"
    "                next_home.append(&load_more);\n"
    "            }\n\n"
    "            let generation = self.home_generation.get().wrapping_add(1);\n",
)

replace(
    "src/browser.rs",
    "fn youtube_home_chip_bar(page: &YouTubeHomePage, event_tx: &Sender<BrowserEvent>) -> gtk::Box {\n"
    "    let section = gtk::Box::new(gtk::Orientation::Vertical, 8);\n",
    "fn youtube_home_chip_bar(\n"
    "    page: &YouTubeHomePage,\n"
    "    event_tx: &Sender<BrowserEvent>,\n"
    "    language: AppLanguage,\n"
    ") -> gtk::Box {\n"
    "    let copy = home_copy(language);\n"
    "    let section = gtk::Box::new(gtk::Orientation::Vertical, 8);\n",
)
replace(
    "src/browser.rs",
    "    let all = gtk::Button::with_label(\"Tudo\");\n",
    "    let all = gtk::Button::with_label(copy.youtube_all);\n",
)

old_mapping = '''fn youtube_feed_section_cards(section: &YouTubeHomeSection) -> Vec<HomeCard> {
    section
        .items
        .iter()
        .filter_map(youtube_feed_item_card)
        .take(18)
        .collect()
}

fn youtube_feed_item_card(item: &YouTubeItem) -> Option<HomeCard> {
    match item.result_type.as_str() {
        "song" | "video" | "episode" if item.playable() => {
            Some(HomeCard::YouTubeTrack(item.clone()))
        }
        "album" => Some(HomeCard::YouTubeAlbum {
            item: item.clone(),
            subtitle: if item.artist.is_empty() {
                item.subtitle.clone()
            } else {
                item.artist.clone()
            },
            detail: if item.subtitle.is_empty() {
                "Álbum • YouTube Music".to_string()
            } else {
                item.subtitle.clone()
            },
            cover_path: item.cached_cover().map(Path::to_path_buf),
        }),
        "artist" => Some(HomeCard::YouTubeArtist {
            item: item.clone(),
            subtitle: if item.subtitle.is_empty() {
                "Artista".to_string()
            } else {
                item.subtitle.clone()
            },
            detail: "Artista • YouTube Music".to_string(),
            cover_path: item.cached_cover().map(Path::to_path_buf),
        }),
        "playlist" => Some(HomeCard::YouTubePlaylist(item.clone())),
        _ => None,
    }
}
'''
new_mapping = '''fn youtube_feed_section_cards(
    section: &YouTubeHomeSection,
    language: AppLanguage,
) -> Vec<HomeCard> {
    let queue = section.playable_queue();
    section
        .items
        .iter()
        .filter_map(|item| youtube_feed_item_card(item, &queue, language))
        .take(18)
        .collect()
}

fn youtube_feed_item_card(
    item: &YouTubeItem,
    queue: &[YouTubeItem],
    language: AppLanguage,
) -> Option<HomeCard> {
    let copy = home_copy(language);
    match item.result_type.as_str() {
        "song" | "video" | "episode" if item.playable() => {
            let index = queue
                .iter()
                .position(|candidate| candidate.video_id == item.video_id)
                .unwrap_or(0);
            Some(HomeCard::YouTubeTrack {
                item: item.clone(),
                queue: queue.to_vec(),
                index,
            })
        }
        "album" => Some(HomeCard::YouTubeAlbum {
            item: item.clone(),
            subtitle: if item.artist.is_empty() {
                item.subtitle.clone()
            } else {
                item.artist.clone()
            },
            detail: if item.subtitle.is_empty() {
                format!("{} • YouTube Music", copy.youtube_album)
            } else {
                item.subtitle.clone()
            },
            cover_path: item.cached_cover().map(Path::to_path_buf),
        }),
        "artist" => Some(HomeCard::YouTubeArtist {
            item: item.clone(),
            subtitle: if item.subtitle.is_empty() {
                copy.youtube_artist.to_string()
            } else {
                item.subtitle.clone()
            },
            detail: format!("{} • YouTube Music", copy.youtube_artist),
            cover_path: item.cached_cover().map(Path::to_path_buf),
        }),
        "playlist" => Some(HomeCard::YouTubePlaylist(item.clone())),
        _ => None,
    }
}
'''
replace("src/browser.rs", old_mapping, new_mapping)

replace(
    "src/browser.rs",
    "        HomeCard::YouTubeTrack(item) => {\n"
    "            format!(\n"
    "                \"{} {} {} {}\",\n"
    "                item.title, item.subtitle, item.artist, item.album\n"
    "            )\n"
    "        }\n",
    "        HomeCard::YouTubeTrack { item, .. } => {\n"
    "            format!(\n"
    "                \"{} {} {} {}\",\n"
    "                item.title, item.subtitle, item.artist, item.album\n"
    "            )\n"
    "        }\n",
)
replace(
    "src/browser.rs",
    "        HomeCard::YouTubeTrack(item) => (\n"
    "            item.cached_cover(),\n"
    "            \"audio-x-generic-symbolic\",\n"
    "            item.title.as_str(),\n"
    "            item.subtitle.as_str(),\n"
    "            \"YouTube Music\",\n"
    "            true,\n"
    "        ),\n",
    "        HomeCard::YouTubeTrack { item, .. } => (\n"
    "            item.cached_cover(),\n"
    "            \"audio-x-generic-symbolic\",\n"
    "            item.title.as_str(),\n"
    "            item.subtitle.as_str(),\n"
    "            \"YouTube Music\",\n"
    "            true,\n"
    "        ),\n",
)
replace(
    "src/browser.rs",
    "            HomeCard::YouTubeTrack(item) => BrowserEvent::YouTubeTrackActivated {\n"
    "                item: item.clone(),\n"
    "                queue: vec![item],\n"
    "                index: 0,\n"
    "            },\n",
    "            HomeCard::YouTubeTrack { item, queue, index } => {\n"
    "                BrowserEvent::YouTubeTrackActivated { item, queue, index }\n"
    "            },\n",
)
replace(
    "src/browser.rs",
    "        HomeCard::YouTubeTrack(item) => (\n"
    "            Some(BrowserEvent::YouTubeTrackActivated {\n"
    "                item: item.clone(),\n"
    "                queue: vec![item.clone()],\n"
    "                index: 0,\n"
    "            }),\n"
    "            None,\n"
    "            \"track\",\n"
    "            item.video_id.clone(),\n"
    "            item.title.clone(),\n"
    "        ),\n",
    "        HomeCard::YouTubeTrack { item, queue, index } => (\n"
    "            Some(BrowserEvent::YouTubeTrackActivated {\n"
    "                item: item.clone(),\n"
    "                queue: queue.clone(),\n"
    "                index: *index,\n"
    "            }),\n"
    "            None,\n"
    "            \"track\",\n"
    "            item.video_id.clone(),\n"
    "            item.title.clone(),\n"
    "        ),\n",
)

browser = Path("src/browser.rs").read_text(encoding="utf-8")
if "HomeCard::YouTubeTrack(" in browser or "Self::YouTubeTrack(" in browser:
    raise SystemExit("src/browser.rs: legacy tuple YouTubeTrack pattern remains")

with Path("src/browser.rs").open("a", encoding="utf-8") as file:
    file.write(
        "\n#[cfg(test)]\n"
        "mod youtube_home_browser_tests {\n"
        "    use super::*;\n\n"
        "    fn track(video_id: &str, title: &str) -> YouTubeItem {\n"
        "        YouTubeItem {\n"
        "            result_type: \"song\".to_string(),\n"
        "            video_id: video_id.to_string(),\n"
        "            title: title.to_string(),\n"
        "            ..YouTubeItem::default()\n"
        "        }\n"
        "    }\n\n"
        "    #[test]\n"
        "    fn youtube_home_copy_is_localized() {\n"
        "        assert_eq!(home_copy(AppLanguage::Portuguese).youtube_all, \"Tudo\");\n"
        "        assert_eq!(home_copy(AppLanguage::English).youtube_all, \"All\");\n"
        "        assert_eq!(home_copy(AppLanguage::Spanish).youtube_all, \"Todo\");\n"
        "    }\n\n"
        "    #[test]\n"
        "    fn youtube_track_card_keeps_section_queue_and_index() {\n"
        "        let first = track(\"one\", \"One\");\n"
        "        let second = track(\"two\", \"Two\");\n"
        "        let queue = vec![first, second.clone()];\n"
        "        let card = youtube_feed_item_card(\n"
        "            &second,\n"
        "            &queue,\n"
        "            AppLanguage::English,\n"
        "        )\n"
        "        .expect(\"playable card\");\n\n"
        "        match card {\n"
        "            HomeCard::YouTubeTrack { item, queue, index } => {\n"
        "                assert_eq!(item.video_id, \"two\");\n"
        "                assert_eq!(index, 1);\n"
        "                assert_eq!(queue.len(), 2);\n"
        "            }\n"
        "            _ => panic!(\"expected YouTube track card\"),\n"
        "        }\n"
        "    }\n"
        "}\n"
    )

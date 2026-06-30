#!/usr/bin/env python3
"""Prevent collection-heavy Home feeds from starving song artwork downloads."""

from pathlib import Path


def replace_once(path: str, old: str, new: str, label: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one match in {path}, found {count}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


replace_once(
    "src/youtube/mod.rs",
    '''pub fn cache_home_page_covers(page: &mut YouTubeHomePage) {
    const PAGE_COVER_BUDGET: usize = 64;
    let mut remaining = PAGE_COVER_BUDGET;

    // Collection cards are visually dominated by their artwork and often appear
    // after song-heavy rows. Cache them first so a fixed network budget does not
    // leave playlists, albums and artists as placeholders farther down the feed.
    for collection_pass in [true, false] {
        for section in &mut page.sections {
            if !structured_cards::uses_card_carousel(&section.layout) {
                continue;
            }

            for item in &mut section.items {
                if remaining == 0 {
                    return;
                }
                let collection = matches!(
                    item.result_type.as_str(),
                    "playlist" | "album" | "artist" | "podcast"
                );
                if collection != collection_pass || item.thumbnail_url.trim().is_empty() {
                    continue;
                }
                cache_items_for_browser(std::slice::from_mut(item));
                remaining -= 1;
            }
        }
    }
}
''',
    '''fn home_cover_cache_targets(page: &YouTubeHomePage) -> Vec<(usize, usize)> {
    const PAGE_COVER_BUDGET: usize = 96;
    const PER_SECTION_VISIBLE_BUDGET: usize = 12;

    let mut targets = Vec::new();
    // Walk rows in round-robin order. This guarantees that a collection-heavy
    // section cannot consume the entire budget before song rows are reached.
    for item_index in 0..PER_SECTION_VISIBLE_BUDGET {
        for (section_index, section) in page.sections.iter().enumerate() {
            if !structured_cards::uses_card_carousel(&section.layout)
                || item_index >= section.items.len()
            {
                continue;
            }
            targets.push((section_index, item_index));
            if targets.len() == PAGE_COVER_BUDGET {
                return targets;
            }
        }
    }
    targets
}

pub fn cache_home_page_covers(page: &mut YouTubeHomePage) {
    for (section_index, item_index) in home_cover_cache_targets(page) {
        let Some(item) = page
            .sections
            .get_mut(section_index)
            .and_then(|section| section.items.get_mut(item_index))
        else {
            continue;
        };
        // Do not skip empty thumbnail URLs here. cache_items_for_browser can
        // synthesize a canonical image from a valid video ID.
        cache_items_for_browser(std::slice::from_mut(item));
    }
}

#[cfg(test)]
mod home_cover_cache_tests {
    use super::*;
    use crate::youtube::feed::YouTubeHomeSection;

    fn section(id: &str, result_type: &str, count: usize) -> YouTubeHomeSection {
        YouTubeHomeSection {
            id: id.to_string(),
            layout: "carousel".to_string(),
            items: (0..count)
                .map(|index| YouTubeItem {
                    result_type: result_type.to_string(),
                    title: format!("{id}-{index}"),
                    video_id: (result_type == "song")
                        .then(|| format!("song{index:07}"))
                        .unwrap_or_default(),
                    ..YouTubeItem::default()
                })
                .collect(),
            ..YouTubeHomeSection::default()
        }
    }

    #[test]
    fn cover_targets_are_distributed_across_sections() {
        let page = YouTubeHomePage {
            sections: vec![
                section("albums", "album", 30),
                section("songs", "song", 6),
                section("playlists", "playlist", 30),
            ],
            ..YouTubeHomePage::default()
        };
        let targets = home_cover_cache_targets(&page);
        assert!(targets.contains(&(0, 0)));
        assert!(targets.contains(&(1, 0)));
        assert!(targets.contains(&(2, 0)));
        assert!(targets.contains(&(1, 5)));
    }

    #[test]
    fn cover_targets_include_items_without_explicit_thumbnail_urls() {
        let page = YouTubeHomePage {
            sections: vec![section("songs", "song", 1)],
            ..YouTubeHomePage::default()
        };
        assert_eq!(home_cover_cache_targets(&page), vec![(0, 0)]);
    }
}
''',
    "fair Home cover scheduling",
)

replace_once(
    "src/browser.rs",
    '''    rail.set_margin_top(4);
    rail.set_margin_start(2);
    rail.set_margin_end(28);
    rail.set_margin_bottom(24);
''',
    '''    rail.set_margin_top(4);
    rail.set_margin_start(2);
    rail.set_margin_end(28);
    rail.set_margin_bottom(28);
''',
    "chip rail final clearance",
)
replace_once(
    "src/browser.rs",
    '''    scroll.set_min_content_height(80);
''',
    '''    scroll.set_min_content_height(88);
''',
    "chip carousel final height",
)

print("Fair Home cover scheduling and chip clearance applied")

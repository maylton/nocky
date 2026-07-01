use serde::{Deserialize, Serialize};

use super::YouTubeItem;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct YouTubeHomeEndpoint {
    pub browse_id: String,
    pub params: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct YouTubeHomeChip {
    pub title: String,
    pub browse_id: String,
    pub params: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct YouTubeHomeSection {
    pub id: String,
    pub title: String,
    pub label: String,
    pub thumbnail_url: String,
    pub layout: String,
    pub endpoint: YouTubeHomeEndpoint,
    pub items: Vec<YouTubeItem>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct YouTubeHomePage {
    pub version: u32,
    pub generated_at: u64,
    pub stale: bool,
    pub selected_chip_params: String,
    pub chips: Vec<YouTubeHomeChip>,
    pub sections: Vec<YouTubeHomeSection>,
    pub continuation: String,
}

impl YouTubeHomeSection {
    pub fn playable_queue(&self) -> Vec<YouTubeItem> {
        self.items
            .iter()
            .filter(|item| item.playable())
            .cloned()
            .collect()
    }
}

impl YouTubeHomePage {
    #[cfg(test)]
    pub fn item_count(&self) -> usize {
        self.sections
            .iter()
            .map(|section| section.items.len())
            .sum()
    }

    pub fn merge_page(&mut self, incoming: Self) {
        if self.version == 0 {
            self.version = incoming.version;
        }
        self.generated_at = self.generated_at.max(incoming.generated_at);
        self.stale |= incoming.stale;
        if self.chips.is_empty() {
            self.chips = incoming.chips;
        }
        if self.selected_chip_params.is_empty() {
            self.selected_chip_params = incoming.selected_chip_params;
        }

        for mut section in incoming.sections {
            if let Some(existing) = self
                .sections
                .iter_mut()
                .find(|candidate| candidate.id == section.id)
            {
                for item in section.items.drain(..) {
                    let duplicate = existing.items.iter().any(|candidate| {
                        (!item.video_id.is_empty() && candidate.video_id == item.video_id)
                            || (!item.browse_id.is_empty()
                                && candidate.result_type == item.result_type
                                && candidate.browse_id == item.browse_id)
                    });
                    if !duplicate {
                        existing.items.push(item);
                    }
                }
            } else {
                self.sections.push(section);
            }
        }
        self.continuation = incoming.continuation;
    }

    pub fn update_cover_paths(&mut self, incoming: &Self) -> bool {
        let mut changed = false;
        for section in &incoming.sections {
            let Some(existing) = self
                .sections
                .iter_mut()
                .find(|candidate| candidate.id == section.id)
            else {
                continue;
            };

            for item in &section.items {
                let Some(existing_item) = existing
                    .items
                    .iter_mut()
                    .find(|candidate| youtube_home_items_match(candidate, item))
                else {
                    continue;
                };

                if !item.thumbnail_url.trim().is_empty()
                    && existing_item.thumbnail_url != item.thumbnail_url
                {
                    existing_item.thumbnail_url = item.thumbnail_url.clone();
                    changed = true;
                }
                if !item.cover_path.trim().is_empty() && existing_item.cover_path != item.cover_path
                {
                    existing_item.cover_path = item.cover_path.clone();
                    changed = true;
                }
            }
        }
        changed
    }
}

fn youtube_home_items_match(left: &YouTubeItem, right: &YouTubeItem) -> bool {
    (!left.video_id.is_empty() && left.video_id == right.video_id)
        || (!left.browse_id.is_empty()
            && left.result_type == right.result_type
            && left.browse_id == right.browse_id)
        || (left.result_type == right.result_type
            && left.title == right.title
            && left.artist == right.artist
            && left.album == right.album)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(video_id: &str, title: &str) -> YouTubeItem {
        YouTubeItem {
            result_type: "song".to_string(),
            title: title.to_string(),
            video_id: video_id.to_string(),
            ..YouTubeItem::default()
        }
    }

    #[test]
    fn merges_continuation_pages_without_duplicates() {
        let mut first = YouTubeHomePage {
            version: 2,
            sections: vec![YouTubeHomeSection {
                id: "quick".to_string(),
                title: "Quick picks".to_string(),
                items: vec![item("one", "One")],
                ..YouTubeHomeSection::default()
            }],
            continuation: "2".to_string(),
            ..YouTubeHomePage::default()
        };
        first.merge_page(YouTubeHomePage {
            version: 2,
            sections: vec![YouTubeHomeSection {
                id: "quick".to_string(),
                items: vec![item("one", "One"), item("two", "Two")],
                ..YouTubeHomeSection::default()
            }],
            ..YouTubeHomePage::default()
        });

        assert_eq!(first.item_count(), 2);
        assert!(first.continuation.is_empty());
    }

    #[test]
    fn updates_existing_cover_paths_without_adding_duplicates() {
        let mut page = YouTubeHomePage {
            sections: vec![YouTubeHomeSection {
                id: "quick".to_string(),
                title: "Quick picks".to_string(),
                items: vec![item("one", "One")],
                ..YouTubeHomeSection::default()
            }],
            ..YouTubeHomePage::default()
        };
        let incoming = YouTubeHomePage {
            sections: vec![YouTubeHomeSection {
                id: "quick".to_string(),
                items: vec![YouTubeItem {
                    thumbnail_url: "https://i.ytimg.com/vi/one/hqdefault.jpg".to_string(),
                    cover_path: "/tmp/one.jpg".to_string(),
                    ..item("one", "One")
                }],
                ..YouTubeHomeSection::default()
            }],
            ..YouTubeHomePage::default()
        };

        assert!(page.update_cover_paths(&incoming));
        assert_eq!(page.item_count(), 1);
        assert_eq!(page.sections[0].items[0].cover_path, "/tmp/one.jpg");
    }

    #[test]
    fn playable_queue_preserves_section_order() {
        let section = YouTubeHomeSection {
            items: vec![
                item("one", "One"),
                YouTubeItem {
                    result_type: "album".to_string(),
                    browse_id: "MPRE".to_string(),
                    ..YouTubeItem::default()
                },
                item("two", "Two"),
            ],
            ..YouTubeHomeSection::default()
        };

        let queue = section.playable_queue();
        assert_eq!(
            queue
                .iter()
                .map(|item| item.video_id.as_str())
                .collect::<Vec<_>>(),
            vec!["one", "two"]
        );
    }

    #[test]
    fn deserializes_versioned_contract() {
        let page: YouTubeHomePage = serde_json::from_str(
            r#"{
                "version": 2,
                "stale": true,
                "selected_chip_params": "mood-energy",
                "sections": [{
                    "id": "albums",
                    "title": "Albums",
                    "layout": "carousel",
                    "items": [{"result_type": "album", "title": "Example", "browse_id": "MPRE"}]
                }]
            }"#,
        )
        .expect("valid feed fixture");

        assert_eq!(page.version, 2);
        assert!(page.stale);
        assert_eq!(page.selected_chip_params, "mood-energy");
        assert_eq!(page.item_count(), 1);
    }
}

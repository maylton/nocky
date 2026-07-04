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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct YouTubeHomeContinuationDelta {
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
        let _ = self.append_continuation(incoming);
    }

    pub fn append_continuation(&mut self, incoming: Self) -> YouTubeHomeContinuationDelta {
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

        let mut changed_sections = Vec::new();
        for mut section in incoming.sections {
            if let Some(existing) = self
                .sections
                .iter_mut()
                .find(|candidate| youtube_home_sections_match(candidate, &section))
            {
                let mut changed = false;
                for item in section.items.drain(..) {
                    if let Some(existing_item) = existing
                        .items
                        .iter_mut()
                        .find(|candidate| youtube_home_items_match(candidate, &item))
                    {
                        if reconcile_home_item_artwork(existing_item, &item) {
                            changed = true;
                        }
                    } else {
                        existing.items.push(item);
                        changed = true;
                    }
                }
                if changed {
                    changed_sections.push(existing.clone());
                }
            } else {
                changed_sections.push(section.clone());
                self.sections.push(section);
            }
        }
        self.continuation = incoming.continuation;
        YouTubeHomeContinuationDelta {
            sections: changed_sections,
            continuation: self.continuation.clone(),
        }
    }

    pub fn can_request_continuation(
        &self,
        continuation: &str,
        selected_chip_params: &str,
        pending: bool,
    ) -> bool {
        !pending
            && !continuation.trim().is_empty()
            && self.continuation == continuation
            && self.selected_chip_params == selected_chip_params
    }

    pub fn update_cover_paths_delta(&mut self, incoming: &Self) -> YouTubeHomeContinuationDelta {
        let mut changed_sections = Vec::new();
        for section in &incoming.sections {
            let Some(existing) = self
                .sections
                .iter_mut()
                .find(|candidate| youtube_home_sections_match(candidate, section))
            else {
                continue;
            };

            let mut section_changed = false;
            for item in &section.items {
                let Some(existing_item) = existing
                    .items
                    .iter_mut()
                    .find(|candidate| youtube_home_items_match(candidate, item))
                else {
                    continue;
                };

                if reconcile_home_item_artwork(existing_item, item) {
                    section_changed = true;
                }
            }
            if section_changed {
                changed_sections.push(existing.clone());
            }
        }
        YouTubeHomeContinuationDelta {
            sections: changed_sections,
            continuation: self.continuation.clone(),
        }
    }
}

pub fn youtube_home_section_key(section: &YouTubeHomeSection) -> String {
    let parts = [
        section.id.trim(),
        section.layout.trim(),
        section.title.trim(),
        section.endpoint.browse_id.trim(),
        section.endpoint.params.trim(),
    ];
    if let Some(value) = parts.iter().find(|value| !value.is_empty()) {
        return (*value).to_string();
    }

    section
        .items
        .iter()
        .find_map(youtube_home_item_key)
        .unwrap_or_else(|| section.label.clone())
}

fn youtube_home_item_key(item: &YouTubeItem) -> Option<String> {
    if !item.video_id.trim().is_empty() {
        return Some(format!("video:{}", item.video_id.trim()));
    }
    if !item.browse_id.trim().is_empty() {
        return Some(format!(
            "{}:{}",
            item.result_type.trim(),
            item.browse_id.trim()
        ));
    }
    if !item.params.trim().is_empty() {
        return Some(format!(
            "{}:{}",
            item.result_type.trim(),
            item.params.trim()
        ));
    }
    if !item.title.trim().is_empty() {
        return Some(format!(
            "{}:{}:{}:{}",
            item.result_type.trim(),
            item.title.trim(),
            item.artist.trim(),
            item.album.trim()
        ));
    }
    None
}

fn reconcile_home_item_artwork(
    existing_item: &mut YouTubeItem,
    incoming_item: &YouTubeItem,
) -> bool {
    let previous_thumbnail_url = existing_item.thumbnail_url.clone();
    let previous_cover_path = existing_item.cover_path.clone();
    let incoming_thumbnail_url = incoming_item.thumbnail_url.trim();
    let incoming_cover_path = incoming_item.cover_path.trim();
    let mut changed = false;
    let mut thumbnail_changed = false;

    if !incoming_thumbnail_url.is_empty()
        && existing_item.thumbnail_url != incoming_item.thumbnail_url
    {
        existing_item.thumbnail_url = incoming_item.thumbnail_url.clone();
        thumbnail_changed = true;
        changed = true;
    }

    if !incoming_cover_path.is_empty()
        && cover_path_can_be_promoted(
            &previous_thumbnail_url,
            &previous_cover_path,
            existing_item,
            incoming_item,
            thumbnail_changed,
        )
        && existing_item.cover_path != incoming_item.cover_path
    {
        existing_item.cover_path = incoming_item.cover_path.clone();
        changed = true;
    } else if thumbnail_changed && !existing_item.cover_path.is_empty() {
        existing_item.cover_path.clear();
        changed = true;
    }

    changed
}

fn cover_path_can_be_promoted(
    previous_thumbnail_url: &str,
    previous_cover_path: &str,
    existing_item: &YouTubeItem,
    incoming_item: &YouTubeItem,
    thumbnail_changed: bool,
) -> bool {
    let incoming_cover_path = incoming_item.cover_path.trim();
    if incoming_cover_path.is_empty() {
        return false;
    }

    let incoming_thumbnail_url = incoming_item.thumbnail_url.trim();
    if incoming_thumbnail_url.is_empty() {
        return existing_item.thumbnail_url.trim().is_empty();
    }

    if incoming_thumbnail_url != existing_item.thumbnail_url.trim() {
        return false;
    }

    if thumbnail_changed && incoming_cover_path == previous_cover_path.trim() {
        return false;
    }

    if thumbnail_changed && incoming_thumbnail_url != previous_thumbnail_url.trim() {
        return true;
    }

    true
}

fn youtube_home_sections_match(left: &YouTubeHomeSection, right: &YouTubeHomeSection) -> bool {
    youtube_home_section_key(left) == youtube_home_section_key(right)
}

fn youtube_home_items_match(left: &YouTubeItem, right: &YouTubeItem) -> bool {
    (!left.video_id.is_empty() && left.video_id == right.video_id)
        || (!left.browse_id.is_empty()
            && left.result_type == right.result_type
            && left.browse_id == right.browse_id)
        || (!left.params.is_empty()
            && left.result_type == right.result_type
            && left.params == right.params)
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
    fn appending_second_page_preserves_first_page_sections_and_order() {
        let mut page = YouTubeHomePage {
            sections: vec![YouTubeHomeSection {
                id: "first".to_string(),
                title: "First".to_string(),
                items: vec![item("one", "One")],
                ..YouTubeHomeSection::default()
            }],
            continuation: "page-2".to_string(),
            ..YouTubeHomePage::default()
        };

        let delta = page.append_continuation(YouTubeHomePage {
            sections: vec![YouTubeHomeSection {
                id: "second".to_string(),
                title: "Second".to_string(),
                items: vec![item("two", "Two")],
                ..YouTubeHomeSection::default()
            }],
            continuation: "page-3".to_string(),
            ..YouTubeHomePage::default()
        });

        assert_eq!(
            page.sections
                .iter()
                .map(|section| section.id.as_str())
                .collect::<Vec<_>>(),
            ["first", "second"]
        );
        assert_eq!(delta.sections[0].id, "second");
        assert_eq!(page.continuation, "page-3");
    }

    #[test]
    fn continuation_sections_are_appended_in_response_order() {
        let mut page = YouTubeHomePage {
            sections: vec![YouTubeHomeSection {
                id: "first".to_string(),
                items: vec![item("one", "One")],
                ..YouTubeHomeSection::default()
            }],
            ..YouTubeHomePage::default()
        };

        page.append_continuation(YouTubeHomePage {
            sections: vec![
                YouTubeHomeSection {
                    id: "second".to_string(),
                    items: vec![item("two", "Two")],
                    ..YouTubeHomeSection::default()
                },
                YouTubeHomeSection {
                    id: "third".to_string(),
                    items: vec![item("three", "Three")],
                    ..YouTubeHomeSection::default()
                },
            ],
            ..YouTubeHomePage::default()
        });

        assert_eq!(
            page.sections
                .iter()
                .map(|section| section.id.as_str())
                .collect::<Vec<_>>(),
            ["first", "second", "third"]
        );
    }

    #[test]
    fn duplicate_sections_or_items_are_reconciled_without_duplication() {
        let mut page = YouTubeHomePage {
            sections: vec![YouTubeHomeSection {
                id: "quick".to_string(),
                title: "Quick picks".to_string(),
                items: vec![item("one", "One")],
                ..YouTubeHomeSection::default()
            }],
            ..YouTubeHomePage::default()
        };

        let delta = page.append_continuation(YouTubeHomePage {
            sections: vec![YouTubeHomeSection {
                id: "quick".to_string(),
                title: "Quick picks".to_string(),
                items: vec![item("one", "One"), item("two", "Two")],
                ..YouTubeHomeSection::default()
            }],
            continuation: "next".to_string(),
            ..YouTubeHomePage::default()
        });

        assert_eq!(page.sections.len(), 1);
        assert_eq!(
            page.sections[0]
                .items
                .iter()
                .map(|item| item.video_id.as_str())
                .collect::<Vec<_>>(),
            ["one", "two"]
        );
        assert_eq!(delta.sections.len(), 1);
        assert_eq!(delta.sections[0].items.len(), 2);
    }

    #[test]
    fn empty_successful_continuation_removes_continuation_without_clearing_home() {
        let mut page = YouTubeHomePage {
            sections: vec![YouTubeHomeSection {
                id: "first".to_string(),
                items: vec![item("one", "One")],
                ..YouTubeHomeSection::default()
            }],
            continuation: "page-2".to_string(),
            ..YouTubeHomePage::default()
        };

        let delta = page.append_continuation(YouTubeHomePage::default());

        assert_eq!(page.sections.len(), 1);
        assert!(page.continuation.is_empty());
        assert!(delta.sections.is_empty());
        assert!(delta.continuation.is_empty());
    }

    #[test]
    fn failed_continuation_can_leave_existing_page_retryable() {
        let page = YouTubeHomePage {
            sections: vec![YouTubeHomeSection {
                id: "first".to_string(),
                items: vec![item("one", "One")],
                ..YouTubeHomeSection::default()
            }],
            continuation: "retry-token".to_string(),
            ..YouTubeHomePage::default()
        };
        let unchanged = page.clone();

        assert_eq!(page, unchanged);
        assert_eq!(page.continuation, "retry-token");
    }

    #[test]
    fn pending_continuation_does_not_dispatch_again() {
        let page = YouTubeHomePage {
            selected_chip_params: "energy".to_string(),
            continuation: "page-2".to_string(),
            ..YouTubeHomePage::default()
        };

        assert!(page.can_request_continuation("page-2", "energy", false));
        assert!(!page.can_request_continuation("page-2", "energy", true));
    }

    #[test]
    fn stale_continuation_token_or_chip_cannot_start_request() {
        let page = YouTubeHomePage {
            selected_chip_params: "energy".to_string(),
            continuation: "page-2".to_string(),
            ..YouTubeHomePage::default()
        };

        assert!(!page.can_request_continuation("old-page", "energy", false));
        assert!(!page.can_request_continuation("page-2", "focus", false));
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

        let delta = page.update_cover_paths_delta(&incoming);
        assert_eq!(page.item_count(), 1);
        assert_eq!(page.sections[0].items[0].cover_path, "/tmp/one.jpg");
        assert_eq!(delta.sections.len(), 1);
        assert_eq!(delta.sections[0].items[0].cover_path, "/tmp/one.jpg");
    }

    #[test]
    fn thumbnail_change_clears_stale_cover_path_from_previous_thumbnail() {
        let mut page = YouTubeHomePage {
            sections: vec![YouTubeHomeSection {
                id: "quick".to_string(),
                title: "Quick picks".to_string(),
                items: vec![YouTubeItem {
                    thumbnail_url: "https://i.ytimg.com/vi/one/old.jpg".to_string(),
                    cover_path: "/tmp/old.cover".to_string(),
                    ..item("one", "One")
                }],
                ..YouTubeHomeSection::default()
            }],
            ..YouTubeHomePage::default()
        };
        let incoming = YouTubeHomePage {
            sections: vec![YouTubeHomeSection {
                id: "quick".to_string(),
                items: vec![YouTubeItem {
                    thumbnail_url: "https://i.ytimg.com/vi/one/new.jpg".to_string(),
                    cover_path: "/tmp/old.cover".to_string(),
                    ..item("one", "One")
                }],
                ..YouTubeHomeSection::default()
            }],
            ..YouTubeHomePage::default()
        };

        let delta = page.update_cover_paths_delta(&incoming);

        assert_eq!(
            page.sections[0].items[0].thumbnail_url,
            "https://i.ytimg.com/vi/one/new.jpg"
        );
        assert!(page.sections[0].items[0].cover_path.is_empty());
        assert_eq!(delta.sections.len(), 1);
        assert!(delta.sections[0].items[0].cover_path.is_empty());
    }

    #[test]
    fn thumbnail_change_can_promote_new_cover_path() {
        let mut page = YouTubeHomePage {
            sections: vec![YouTubeHomeSection {
                id: "quick".to_string(),
                title: "Quick picks".to_string(),
                items: vec![YouTubeItem {
                    thumbnail_url: "https://i.ytimg.com/vi/one/old.jpg".to_string(),
                    cover_path: "/tmp/old.cover".to_string(),
                    ..item("one", "One")
                }],
                ..YouTubeHomeSection::default()
            }],
            ..YouTubeHomePage::default()
        };
        let incoming = YouTubeHomePage {
            sections: vec![YouTubeHomeSection {
                id: "quick".to_string(),
                items: vec![YouTubeItem {
                    thumbnail_url: "https://i.ytimg.com/vi/one/new.jpg".to_string(),
                    cover_path: "/tmp/new.cover".to_string(),
                    ..item("one", "One")
                }],
                ..YouTubeHomeSection::default()
            }],
            ..YouTubeHomePage::default()
        };

        page.update_cover_paths_delta(&incoming);

        assert_eq!(
            page.sections[0].items[0].thumbnail_url,
            "https://i.ytimg.com/vi/one/new.jpg"
        );
        assert_eq!(page.sections[0].items[0].cover_path, "/tmp/new.cover");
    }

    #[test]
    fn cover_only_update_does_not_override_item_with_known_thumbnail() {
        let mut page = YouTubeHomePage {
            sections: vec![YouTubeHomeSection {
                id: "quick".to_string(),
                title: "Quick picks".to_string(),
                items: vec![YouTubeItem {
                    thumbnail_url: "https://i.ytimg.com/vi/one/current.jpg".to_string(),
                    cover_path: "/tmp/current.cover".to_string(),
                    ..item("one", "One")
                }],
                ..YouTubeHomeSection::default()
            }],
            ..YouTubeHomePage::default()
        };
        let incoming = YouTubeHomePage {
            sections: vec![YouTubeHomeSection {
                id: "quick".to_string(),
                items: vec![YouTubeItem {
                    cover_path: "/tmp/unknown-source.cover".to_string(),
                    ..item("one", "One")
                }],
                ..YouTubeHomeSection::default()
            }],
            ..YouTubeHomePage::default()
        };

        page.update_cover_paths_delta(&incoming);

        assert_eq!(page.sections[0].items[0].cover_path, "/tmp/current.cover");
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

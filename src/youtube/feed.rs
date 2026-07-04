use serde::{Deserialize, Serialize};

use super::YouTubeItem;

#[path = "artwork_trace.rs"]
mod artwork_trace;

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
                    let duplicate = existing
                        .items
                        .iter()
                        .any(|candidate| youtube_home_items_match(candidate, &item));
                    if !duplicate {
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

                artwork_trace::trace_delta_compare(
                    "delta_compare_item",
                    &section.title,
                    existing_item,
                    item,
                );

                if !item.thumbnail_url.trim().is_empty()
                    && existing_item.thumbnail_url != item.thumbnail_url
                {
                    artwork_trace::trace_delta_update(
                        "delta_before_thumbnail_url_update",
                        &section.title,
                        "thumbnail_url",
                        existing_item,
                        item,
                        &existing_item.thumbnail_url,
                        &item.thumbnail_url,
                    );
                    existing_item.thumbnail_url = item.thumbnail_url.clone();
                    section_changed = true;
                }
                if !item.cover_path.trim().is_empty() && existing_item.cover_path != item.cover_path
                {
                    artwork_trace::trace_delta_update(
                        "delta_before_cover_path_update",
                        &section.title,
                        "cover_path",
                        existing_item,
                        item,
                        &existing_item.cover_path,
                        &item.cover_path,
                    );
                    existing_item.cover_path = item.cover_path.clone();
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
    fn cover_delta_updates_existing_items_only() {
        let mut current = YouTubeHomePage {
            sections: vec![YouTubeHomeSection {
                id: "quick".to_string(),
                items: vec![item("one", "One")],
                ..YouTubeHomeSection::default()
            }],
            ..YouTubeHomePage::default()
        };
        let incoming = YouTubeHomePage {
            sections: vec![YouTubeHomeSection {
                id: "quick".to_string(),
                items: vec![YouTubeItem {
                    cover_path: "/tmp/one.jpg".to_string(),
                    ..item("one", "One")
                }],
                ..YouTubeHomeSection::default()
            }],
            ..YouTubeHomePage::default()
        };

        let delta = current.update_cover_paths_delta(&incoming);

        assert_eq!(current.sections[0].items[0].cover_path, "/tmp/one.jpg");
        assert_eq!(delta.sections.len(), 1);
    }
}

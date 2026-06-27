use crate::playback::queue::{PlaybackQueue, QueueEntry, QueueSourceKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueSection {
    Played,
    Current,
    Upcoming,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueueViewItem {
    pub entry: QueueEntry,
    pub position: usize,
    pub section: QueueSection,
    pub is_current: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueuePresentation {
    pub source: QueueSourceKind,
    pub total: usize,
    pub played_count: usize,
    pub upcoming_count: usize,
    pub current_index: Option<usize>,
    pub items: Vec<QueueViewItem>,
}

impl QueuePresentation {
    pub fn from_queue(queue: &PlaybackQueue, source: QueueSourceKind) -> Self {
        let current_index = queue.current_index();
        let items = queue
            .entries()
            .iter()
            .cloned()
            .enumerate()
            .map(|(position, entry)| {
                let section = match current_index {
                    Some(current) if position < current => QueueSection::Played,
                    Some(current) if position == current => QueueSection::Current,
                    _ => QueueSection::Upcoming,
                };

                QueueViewItem {
                    entry,
                    position,
                    section,
                    is_current: current_index == Some(position),
                }
            })
            .collect::<Vec<_>>();

        let played_count = current_index.unwrap_or(0);
        let upcoming_count = match current_index {
            Some(current) => queue.len().saturating_sub(current.saturating_add(1)),
            None => queue.len(),
        };

        Self {
            source,
            total: queue.len(),
            played_count,
            upcoming_count,
            current_index,
            items,
        }
    }

    pub const fn can_clear_upcoming(&self) -> bool {
        self.upcoming_count > 0
    }

    pub fn section_count(&self, section: QueueSection) -> usize {
        match section {
            QueueSection::Played => self.played_count,
            QueueSection::Current => usize::from(self.current_index.is_some()),
            QueueSection::Upcoming => self.upcoming_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::playback::queue::QueueMedia;
    use std::path::PathBuf;

    fn local(number: usize) -> QueueMedia {
        QueueMedia::local(
            PathBuf::from(format!("/music/{number}.flac")),
            format!("Track {number}"),
            "Artist",
            "Album",
            180,
            None,
        )
    }

    #[test]
    fn presentation_splits_played_current_and_upcoming() {
        let mut queue = PlaybackQueue::new();
        queue.replace([local(1), local(2), local(3), local(4)], Some(1));

        let view = QueuePresentation::from_queue(&queue, QueueSourceKind::Local);

        assert_eq!(view.total, 4);
        assert_eq!(view.played_count, 1);
        assert_eq!(view.upcoming_count, 2);
        assert_eq!(view.current_index, Some(1));
        assert_eq!(view.items[0].section, QueueSection::Played);
        assert_eq!(view.items[1].section, QueueSection::Current);
        assert!(view.items[1].is_current);
        assert_eq!(view.items[2].section, QueueSection::Upcoming);
        assert!(view.can_clear_upcoming());
    }

    #[test]
    fn presentation_without_current_treats_everything_as_upcoming() {
        let mut queue = PlaybackQueue::new();
        queue.append(local(1));
        queue.append(local(2));

        let view = QueuePresentation::from_queue(&queue, QueueSourceKind::Local);

        assert_eq!(view.played_count, 0);
        assert_eq!(view.upcoming_count, 2);
        assert_eq!(view.current_index, None);
        assert!(view
            .items
            .iter()
            .all(|item| item.section == QueueSection::Upcoming));
    }

    #[test]
    fn source_is_presentation_metadata_not_queue_mixing() {
        let mut queue = PlaybackQueue::new();
        queue.append(QueueMedia::youtube(
            "video-1", "Online", "Artist", "Album", 180, None,
        ));

        let view = QueuePresentation::from_queue(&queue, QueueSourceKind::YouTube);

        assert_eq!(view.source, QueueSourceKind::YouTube);
        assert_eq!(view.total, 1);
    }
}

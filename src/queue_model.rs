use serde::{Deserialize, Serialize};
use std::{collections::HashSet, error::Error, fmt, path::PathBuf};

pub const QUEUE_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct QueueEntryId(u64);

impl QueueEntryId {
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for QueueEntryId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueSourceKind {
    Local,
    YouTube,
}

impl QueueSourceKind {
    pub const fn state_file_name(self) -> &'static str {
        match self {
            Self::Local => "queue-local.json",
            Self::YouTube => "queue-youtube.json",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QueueSource {
    Local { path: PathBuf },
    YouTube { video_id: String },
}

impl QueueSource {
    pub const fn kind(&self) -> QueueSourceKind {
        match self {
            Self::Local { .. } => QueueSourceKind::Local,
            Self::YouTube { .. } => QueueSourceKind::YouTube,
        }
    }

    pub fn stable_key(&self) -> String {
        match self {
            Self::Local { path } => format!("local:{}", path.to_string_lossy()),
            Self::YouTube { video_id } => format!("youtube:{video_id}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueMedia {
    pub source: QueueSource,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_seconds: u64,
    pub cover_path: Option<PathBuf>,
}

impl QueueMedia {
    pub fn new(
        source: QueueSource,
        title: impl Into<String>,
        artist: impl Into<String>,
        album: impl Into<String>,
        duration_seconds: u64,
        cover_path: Option<PathBuf>,
    ) -> Self {
        Self {
            source,
            title: title.into(),
            artist: artist.into(),
            album: album.into(),
            duration_seconds,
            cover_path,
        }
    }

    pub fn local(
        path: PathBuf,
        title: impl Into<String>,
        artist: impl Into<String>,
        album: impl Into<String>,
        duration_seconds: u64,
        cover_path: Option<PathBuf>,
    ) -> Self {
        Self::new(
            QueueSource::Local { path },
            title,
            artist,
            album,
            duration_seconds,
            cover_path,
        )
    }

    pub fn youtube(
        video_id: impl Into<String>,
        title: impl Into<String>,
        artist: impl Into<String>,
        album: impl Into<String>,
        duration_seconds: u64,
        cover_path: Option<PathBuf>,
    ) -> Self {
        Self::new(
            QueueSource::YouTube {
                video_id: video_id.into(),
            },
            title,
            artist,
            album,
            duration_seconds,
            cover_path,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueEntry {
    pub id: QueueEntryId,
    pub media: QueueMedia,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueSnapshot {
    pub version: u32,
    pub next_id: u64,
    pub current_id: Option<QueueEntryId>,
    pub entries: Vec<QueueEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueueError {
    EntryNotFound(QueueEntryId),
    CannotRemoveCurrent(QueueEntryId),
    TargetIndexOutOfBounds {
        target: usize,
        len: usize,
    },
    UnsupportedSnapshotVersion(u32),
    DuplicateEntryId(QueueEntryId),
    InvalidCurrentEntry(QueueEntryId),
    MixedSources {
        expected: QueueSourceKind,
        found: QueueSourceKind,
    },
}

impl fmt::Display for QueueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EntryNotFound(id) => write!(formatter, "queue entry {id} was not found"),
            Self::CannotRemoveCurrent(id) => {
                write!(formatter, "queue entry {id} is currently playing")
            }
            Self::TargetIndexOutOfBounds { target, len } => {
                write!(
                    formatter,
                    "queue target index {target} is outside length {len}"
                )
            }
            Self::UnsupportedSnapshotVersion(version) => {
                write!(formatter, "unsupported queue snapshot version {version}")
            }
            Self::DuplicateEntryId(id) => write!(formatter, "duplicate queue entry ID {id}"),
            Self::InvalidCurrentEntry(id) => {
                write!(formatter, "current queue entry {id} is missing")
            }
            Self::MixedSources { expected, found } => {
                write!(
                    formatter,
                    "queue source mismatch: expected {expected:?}, found {found:?}"
                )
            }
        }
    }
}

impl Error for QueueError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaybackQueue {
    next_id: u64,
    current_id: Option<QueueEntryId>,
    entries: Vec<QueueEntry>,
}

impl Default for PlaybackQueue {
    fn default() -> Self {
        Self {
            next_id: 1,
            current_id: None,
            entries: Vec::new(),
        }
    }
}

impl PlaybackQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn replace<I>(&mut self, media: I, current_index: Option<usize>)
    where
        I: IntoIterator<Item = QueueMedia>,
    {
        self.entries.clear();
        self.current_id = None;

        for item in media {
            let _ = self.append(item);
        }

        self.current_id = current_index
            .and_then(|index| self.entries.get(index))
            .map(|entry| entry.id);
    }

    pub fn append(&mut self, media: QueueMedia) -> QueueEntryId {
        let entry = self.make_entry(media);
        let id = entry.id;
        self.entries.push(entry);
        id
    }

    pub fn append_many<I>(&mut self, media: I) -> Vec<QueueEntryId>
    where
        I: IntoIterator<Item = QueueMedia>,
    {
        media.into_iter().map(|item| self.append(item)).collect()
    }

    pub fn insert_next(&mut self, media: QueueMedia) -> QueueEntryId {
        let entry = self.make_entry(media);
        let id = entry.id;
        let target = self
            .current_index()
            .map_or(0, |current| current.saturating_add(1));
        self.entries.insert(target.min(self.entries.len()), entry);
        id
    }

    pub fn remove(&mut self, id: QueueEntryId) -> Result<QueueEntry, QueueError> {
        if self.current_id == Some(id) {
            return Err(QueueError::CannotRemoveCurrent(id));
        }

        let index = self.index_of(id).ok_or(QueueError::EntryNotFound(id))?;
        Ok(self.entries.remove(index))
    }

    pub fn move_entry(&mut self, id: QueueEntryId, target_index: usize) -> Result<(), QueueError> {
        let len = self.entries.len();
        if target_index >= len {
            return Err(QueueError::TargetIndexOutOfBounds {
                target: target_index,
                len,
            });
        }

        let source_index = self.index_of(id).ok_or(QueueError::EntryNotFound(id))?;

        if source_index == target_index {
            return Ok(());
        }

        let entry = self.entries.remove(source_index);
        self.entries.insert(target_index, entry);
        Ok(())
    }

    pub fn clear_upcoming(&mut self) {
        match self.current_index() {
            Some(index) => self.entries.truncate(index.saturating_add(1)),
            None => self.entries.clear(),
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.current_id = None;
    }

    pub fn select(&mut self, id: QueueEntryId) -> Result<(), QueueError> {
        if self.index_of(id).is_none() {
            return Err(QueueError::EntryNotFound(id));
        }
        self.current_id = Some(id);
        Ok(())
    }

    pub fn update_media(&mut self, id: QueueEntryId, media: QueueMedia) -> Result<(), QueueError> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.id == id)
            .ok_or(QueueError::EntryNotFound(id))?;
        entry.media = media;
        Ok(())
    }

    pub fn select_index(&mut self, index: usize) -> Option<QueueEntryId> {
        let id = self.entries.get(index).map(|entry| entry.id)?;
        self.current_id = Some(id);
        Some(id)
    }

    pub fn advance_next(&mut self) -> Option<&QueueEntry> {
        let next_index = self.current_index().map_or(0, |index| index + 1);
        let id = self.entries.get(next_index)?.id;
        self.current_id = Some(id);
        self.entries.get(next_index)
    }

    pub fn advance_previous(&mut self) -> Option<&QueueEntry> {
        let previous_index = self.current_index()?.checked_sub(1)?;
        let id = self.entries.get(previous_index)?.id;
        self.current_id = Some(id);
        self.entries.get(previous_index)
    }

    pub fn current(&self) -> Option<&QueueEntry> {
        self.current_id.and_then(|id| self.entry(id))
    }

    pub const fn current_id(&self) -> Option<QueueEntryId> {
        self.current_id
    }

    pub fn current_index(&self) -> Option<usize> {
        self.current_id.and_then(|id| self.index_of(id))
    }

    pub fn entry(&self, id: QueueEntryId) -> Option<&QueueEntry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    pub fn entries(&self) -> &[QueueEntry] {
        &self.entries
    }

    pub fn source_kind(&self) -> Result<Option<QueueSourceKind>, QueueError> {
        let Some(first) = self.entries.first() else {
            return Ok(None);
        };
        let expected = first.media.source.kind();
        if let Some(found) = self
            .entries
            .iter()
            .map(|entry| entry.media.source.kind())
            .find(|kind| *kind != expected)
        {
            return Err(QueueError::MixedSources { expected, found });
        }
        Ok(Some(expected))
    }

    pub fn accepts(&self, media: &QueueMedia) -> bool {
        self.source_kind()
            .is_ok_and(|kind| kind.is_none_or(|kind| kind == media.source.kind()))
    }

    pub fn validate_source(&self, expected: QueueSourceKind) -> Result<(), QueueError> {
        match self.source_kind()? {
            Some(found) if found != expected => Err(QueueError::MixedSources { expected, found }),
            _ => Ok(()),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn has_next(&self) -> bool {
        self.current_index()
            .is_some_and(|index| index + 1 < self.entries.len())
            || (self.current_id.is_none() && !self.entries.is_empty())
    }

    pub fn has_previous(&self) -> bool {
        self.current_index().is_some_and(|index| index > 0)
    }

    pub fn snapshot(&self) -> QueueSnapshot {
        QueueSnapshot {
            version: QUEUE_SCHEMA_VERSION,
            next_id: self.next_id,
            current_id: self.current_id,
            entries: self.entries.clone(),
        }
    }

    pub fn restore(snapshot: QueueSnapshot) -> Result<Self, QueueError> {
        if snapshot.version != QUEUE_SCHEMA_VERSION {
            return Err(QueueError::UnsupportedSnapshotVersion(snapshot.version));
        }

        let mut seen = HashSet::with_capacity(snapshot.entries.len());
        let mut maximum_id = 0;

        for entry in &snapshot.entries {
            if !seen.insert(entry.id) {
                return Err(QueueError::DuplicateEntryId(entry.id));
            }
            maximum_id = maximum_id.max(entry.id.get());
        }

        if let Some(current_id) = snapshot.current_id {
            if !seen.contains(&current_id) {
                return Err(QueueError::InvalidCurrentEntry(current_id));
            }
        }

        Ok(Self {
            next_id: snapshot.next_id.max(maximum_id.saturating_add(1)).max(1),
            current_id: snapshot.current_id,
            entries: snapshot.entries,
        })
    }

    fn index_of(&self, id: QueueEntryId) -> Option<usize> {
        self.entries.iter().position(|entry| entry.id == id)
    }

    fn make_entry(&mut self, media: QueueMedia) -> QueueEntry {
        let id = QueueEntryId(self.next_id.max(1));
        self.next_id = id.get().saturating_add(1).max(1);
        QueueEntry { id, media }
    }
}

// queue2_completion_core_v1
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueEndAction {
    RepeatCurrent,
    Play(QueueEntryId),
    Stop,
}

pub const fn queue_end_action(repeat_one: bool, next: Option<QueueEntryId>) -> QueueEndAction {
    if repeat_one {
        QueueEndAction::RepeatCurrent
    } else if let Some(id) = next {
        QueueEndAction::Play(id)
    } else {
        QueueEndAction::Stop
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShuffleSnapshot {
    current: Option<QueueEntryId>,
    history: Vec<QueueEntryId>,
    upcoming: Vec<QueueEntryId>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShuffleNavigator {
    current: Option<QueueEntryId>,
    history: Vec<QueueEntryId>,
    upcoming: Vec<QueueEntryId>,
}

impl ShuffleNavigator {
    pub fn clear(&mut self) {
        self.current = None;
        self.history.clear();
        self.upcoming.clear();
    }

    pub fn snapshot(&self) -> ShuffleSnapshot {
        ShuffleSnapshot {
            current: self.current,
            history: self.history.clone(),
            upcoming: self.upcoming.clone(),
        }
    }

    pub fn restore(
        &mut self,
        entries: &[QueueEntry],
        current: Option<QueueEntryId>,
        snapshot: &ShuffleSnapshot,
    ) -> bool {
        let valid = entries.iter().map(|entry| entry.id).collect::<HashSet<_>>();
        let snapshot_ids_valid = snapshot
            .current
            .into_iter()
            .chain(snapshot.history.iter().copied())
            .chain(snapshot.upcoming.iter().copied())
            .all(|id| valid.contains(&id));

        if !snapshot_ids_valid || snapshot.current != current {
            return false;
        }

        let mut unique = HashSet::new();
        let no_duplicates = snapshot
            .history
            .iter()
            .copied()
            .chain(snapshot.upcoming.iter().copied())
            .all(|id| unique.insert(id));

        if !no_duplicates {
            return false;
        }

        self.current = snapshot.current;
        self.history = snapshot.history.clone();
        self.upcoming = snapshot.upcoming.clone();
        true
    }

    pub fn reset(
        &mut self,
        entries: &[QueueEntry],
        current: Option<QueueEntryId>,
        rng_state: &mut u64,
    ) {
        let valid = entries.iter().map(|entry| entry.id).collect::<HashSet<_>>();
        let current = current.filter(|id| valid.contains(id));

        self.current = current;
        self.history.clear();
        self.upcoming = entries
            .iter()
            .map(|entry| entry.id)
            .filter(|id| Some(*id) != current)
            .collect();
        shuffle_entry_ids(&mut self.upcoming, rng_state);
    }

    pub fn next(
        &mut self,
        entries: &[QueueEntry],
        current: Option<QueueEntryId>,
        rng_state: &mut u64,
    ) -> Option<QueueEntryId> {
        self.reconcile(entries, current, rng_state);

        let next = self.upcoming.pop()?;
        if let Some(current) = self.current {
            self.history.push(current);
        }
        self.current = Some(next);
        Some(next)
    }

    pub fn previous(
        &mut self,
        entries: &[QueueEntry],
        current: Option<QueueEntryId>,
        rng_state: &mut u64,
    ) -> Option<QueueEntryId> {
        self.reconcile(entries, current, rng_state);

        let previous = self.history.pop()?;
        if let Some(current) = self.current {
            self.upcoming.push(current);
        }
        self.current = Some(previous);
        Some(previous)
    }

    fn reconcile(
        &mut self,
        entries: &[QueueEntry],
        current: Option<QueueEntryId>,
        rng_state: &mut u64,
    ) {
        let valid = entries.iter().map(|entry| entry.id).collect::<HashSet<_>>();
        let current = current.filter(|id| valid.contains(id));

        if self.current != current {
            self.reset(entries, current, rng_state);
            return;
        }

        let mut reserved = HashSet::with_capacity(entries.len());
        if let Some(current) = current {
            reserved.insert(current);
        }

        self.history
            .retain(|id| valid.contains(id) && reserved.insert(*id));
        self.upcoming
            .retain(|id| valid.contains(id) && reserved.insert(*id));

        let mut added = entries
            .iter()
            .map(|entry| entry.id)
            .filter(|id| !reserved.contains(id))
            .collect::<Vec<_>>();
        shuffle_entry_ids(&mut added, rng_state);

        if !added.is_empty() {
            added.append(&mut self.upcoming);
            self.upcoming = added;
        }
    }
}

fn shuffle_entry_ids(ids: &mut [QueueEntryId], rng_state: &mut u64) {
    for index in (1..ids.len()).rev() {
        *rng_state = next_shuffle_value(*rng_state);
        let target = (*rng_state as usize) % (index + 1);
        ids.swap(index, target);
    }
}

fn next_shuffle_value(value: u64) -> u64 {
    let mut value = if value == 0 {
        0x9e37_79b9_7f4a_7c15
    } else {
        value
    };
    value ^= value << 13;
    value ^= value >> 7;
    value ^= value << 17;
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    fn media(number: usize) -> QueueMedia {
        QueueMedia::local(
            PathBuf::from(format!("/music/{number}.flac")),
            format!("Track {number}"),
            "Artist",
            "Album",
            180,
            None,
        )
    }

    fn queue_with_three() -> PlaybackQueue {
        let mut queue = PlaybackQueue::new();
        queue.replace([media(1), media(2), media(3)], Some(1));
        queue
    }

    #[test]
    fn replacement_assigns_stable_unique_ids() {
        let queue = queue_with_three();
        let ids = queue
            .entries()
            .iter()
            .map(|entry| entry.id)
            .collect::<HashSet<_>>();

        assert_eq!(ids.len(), 3);
        assert_eq!(queue.current_index(), Some(1));
    }

    #[test]
    fn play_next_inserts_immediately_after_current() {
        let mut queue = queue_with_three();
        let inserted = queue.insert_next(media(9));

        assert_eq!(queue.entries()[2].id, inserted);
        assert_eq!(queue.entries()[2].media.title, "Track 9");
        assert_eq!(queue.current_index(), Some(1));
    }

    #[test]
    fn append_places_entry_at_the_end() {
        let mut queue = queue_with_three();
        let appended = queue.append(media(4));

        assert_eq!(queue.entries().last().map(|entry| entry.id), Some(appended));
        assert_eq!(queue.current_index(), Some(1));
    }

    #[test]
    fn current_entry_cannot_be_removed() {
        let mut queue = queue_with_three();
        let current = queue.current_id().expect("current ID");

        assert_eq!(
            queue.remove(current),
            Err(QueueError::CannotRemoveCurrent(current))
        );
    }

    #[test]
    fn moving_entries_keeps_current_identity() {
        let mut queue = queue_with_three();
        let current = queue.current_id().expect("current ID");
        let first = queue.entries()[0].id;

        queue.move_entry(first, 2).expect("move succeeds");

        assert_eq!(queue.current_id(), Some(current));
        assert_eq!(queue.current_index(), Some(0));
        assert_eq!(queue.entries()[2].id, first);
    }

    #[test]
    fn clearing_upcoming_preserves_current_and_history() {
        let mut queue = queue_with_three();
        queue.clear_upcoming();

        assert_eq!(queue.len(), 2);
        assert_eq!(queue.current_index(), Some(1));
        assert_eq!(
            queue.current().map(|entry| entry.media.title.as_str()),
            Some("Track 2")
        );
    }

    #[test]
    fn next_and_previous_follow_edited_order() {
        let mut queue = queue_with_three();
        let third = queue.entries()[2].id;
        queue.move_entry(third, 0).expect("move succeeds");

        let previous = queue.advance_previous().expect("previous entry");
        assert_eq!(previous.media.title, "Track 1");

        let next = queue.advance_next().expect("next entry");
        assert_eq!(next.media.title, "Track 2");
    }

    #[test]
    fn snapshot_round_trip_preserves_ids_order_and_current() {
        let queue = queue_with_three();
        let restored = PlaybackQueue::restore(queue.snapshot()).expect("valid snapshot");

        assert_eq!(restored, queue);
    }

    #[test]
    fn restore_rejects_duplicate_ids() {
        let queue = queue_with_three();
        let mut snapshot = queue.snapshot();
        snapshot.entries[1].id = snapshot.entries[0].id;

        assert!(matches!(
            PlaybackQueue::restore(snapshot),
            Err(QueueError::DuplicateEntryId(_))
        ));
    }

    #[test]
    fn local_and_youtube_entries_can_coexist() {
        let mut queue = PlaybackQueue::new();
        queue.append(media(1));
        queue.append(QueueMedia::youtube(
            "video-1",
            "Remote Track",
            "Remote Artist",
            "Remote Album",
            240,
            None,
        ));

        assert!(matches!(
            queue.entries()[0].media.source,
            QueueSource::Local { .. }
        ));
        assert!(matches!(
            queue.entries()[1].media.source,
            QueueSource::YouTube { .. }
        ));
    }

    #[test]
    fn updating_media_preserves_entry_identity_order_and_current() {
        let mut queue = queue_with_three();
        let ids_before = queue
            .entries()
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>();
        let current_before = queue.current_id();
        let target = ids_before[2];

        queue
            .update_media(
                target,
                QueueMedia::youtube(
                    "updated-video",
                    "Updated title",
                    "Updated artist",
                    "Updated album",
                    222,
                    Some(PathBuf::from("/covers/updated.jpg")),
                ),
            )
            .expect("update media");

        assert_eq!(
            queue
                .entries()
                .iter()
                .map(|entry| entry.id)
                .collect::<Vec<_>>(),
            ids_before
        );
        assert_eq!(queue.current_id(), current_before);
        assert_eq!(
            queue.entry(target).map(|entry| entry.media.title.as_str()),
            Some("Updated title")
        );
    }

    #[test]
    fn shuffle_visits_every_remaining_entry_once_before_stopping() {
        let queue = queue_with_three();
        let mut navigator = ShuffleNavigator::default();
        let mut rng = 7;
        let mut current = queue.current_id();
        let mut visited = HashSet::new();

        while let Some(next) = navigator.next(queue.entries(), current, &mut rng) {
            assert!(visited.insert(next), "shuffle repeated an entry");
            current = Some(next);
        }

        assert_eq!(visited.len(), queue.len() - 1);
    }

    #[test]
    fn shuffle_previous_retraces_history_and_next_returns_forward() {
        let queue = queue_with_three();
        let mut navigator = ShuffleNavigator::default();
        let mut rng = 11;
        let original = queue.current_id();

        let first = navigator
            .next(queue.entries(), original, &mut rng)
            .expect("first shuffled entry");
        let second = navigator
            .next(queue.entries(), Some(first), &mut rng)
            .expect("second shuffled entry");

        let previous = navigator
            .previous(queue.entries(), Some(second), &mut rng)
            .expect("shuffle history");
        assert_eq!(previous, first);

        let forward = navigator
            .next(queue.entries(), Some(previous), &mut rng)
            .expect("forward after previous");
        assert_eq!(forward, second);
    }

    #[test]
    fn shuffle_reconciles_entries_added_during_a_session() {
        let mut queue = queue_with_three();
        let mut navigator = ShuffleNavigator::default();
        let mut rng = 19;
        let original = queue.current_id();

        let first = navigator
            .next(queue.entries(), original, &mut rng)
            .expect("first shuffled entry");
        queue.select(first).expect("select shuffled entry");
        let added = queue.append(media(4));

        let mut visited = HashSet::from([first]);
        let mut current = Some(first);
        while let Some(next) = navigator.next(queue.entries(), current, &mut rng) {
            assert!(visited.insert(next), "shuffle repeated after queue edit");
            current = Some(next);
        }

        assert!(visited.contains(&added));
        assert_eq!(visited.len(), queue.len() - 1);
    }

    #[test]
    fn repeat_one_wins_over_an_available_next_entry() {
        let queue = queue_with_three();
        let next = queue.entries().last().map(|entry| entry.id);

        assert_eq!(queue_end_action(true, next), QueueEndAction::RepeatCurrent);
        assert_eq!(
            queue_end_action(false, next),
            QueueEndAction::Play(next.expect("next ID"))
        );
        assert_eq!(queue_end_action(false, None), QueueEndAction::Stop);
    }
    #[test]
    fn queue_source_kind_detects_and_rejects_mixing() {
        let mut queue = PlaybackQueue::new();
        queue.append(QueueMedia::local(
            PathBuf::from("/music/one.flac"),
            "One",
            "Artist",
            "Album",
            180,
            None,
        ));
        assert_eq!(queue.source_kind(), Ok(Some(QueueSourceKind::Local)));
        assert!(!queue.accepts(&QueueMedia::youtube(
            "video-id", "Online", "Artist", "Album", 200, None
        )));
        queue.append(QueueMedia::youtube(
            "video-id", "Online", "Artist", "Album", 200, None,
        ));
        assert_eq!(
            queue.source_kind(),
            Err(QueueError::MixedSources {
                expected: QueueSourceKind::Local,
                found: QueueSourceKind::YouTube
            })
        );
    }
    #[test]
    fn shuffle_snapshot_round_trip_preserves_navigation() {
        let mut queue = PlaybackQueue::new();
        let first = queue.append(QueueMedia::youtube(
            "one", "One", "Artist", "Album", 120, None,
        ));
        queue.append(QueueMedia::youtube(
            "two", "Two", "Artist", "Album", 120, None,
        ));
        queue.append(QueueMedia::youtube(
            "three", "Three", "Artist", "Album", 120, None,
        ));
        queue.select(first).expect("select first");

        let mut seed = 42;
        let mut navigator = ShuffleNavigator::default();
        navigator.reset(queue.entries(), queue.current_id(), &mut seed);
        let _ = navigator.next(queue.entries(), queue.current_id(), &mut seed);

        let snapshot = navigator.snapshot();
        let mut restored = ShuffleNavigator::default();
        assert!(restored.restore(queue.entries(), snapshot.current, &snapshot));
        assert_eq!(restored.snapshot(), snapshot);
    }

    #[test]
    fn shuffle_snapshot_rejects_unknown_entries() {
        let mut queue = PlaybackQueue::new();
        let first = queue.append(QueueMedia::youtube(
            "one", "One", "Artist", "Album", 120, None,
        ));
        queue.select(first).expect("select first");

        let snapshot = ShuffleSnapshot {
            current: Some(first),
            history: Vec::new(),
            upcoming: vec![QueueEntryId(999)],
        };
        let mut restored = ShuffleNavigator::default();
        assert!(!restored.restore(queue.entries(), Some(first), &snapshot));
    }
}

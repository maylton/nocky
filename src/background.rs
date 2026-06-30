use crate::{
    lyrics::LyricLine,
    model::TrackData,
    youtube::{
        YouTubeArtistOverview, YouTubeHomePage, YouTubeItem, YouTubeLibrarySnapshot,
        YouTubePlaylistCreation, YouTubeSearchResults, YouTubeStatus, YouTubeStream,
    },
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
};

pub(crate) enum BackgroundMessage {
    LibraryScanned {
        root: PathBuf,
        result: Result<Vec<TrackData>, String>,
    },
    LyricsDownloaded {
        path: PathBuf,
        result: Result<(), String>,
        notify: bool,
    },
    YouTubeLyricsDownloaded {
        video_id: String,
        notify: bool,
        result: Result<Vec<LyricLine>, String>,
    },
    YouTubeStatus(Result<YouTubeStatus, String>),
    YouTubeConnected(Result<YouTubeStatus, String>),
    YouTubeDisconnected(Result<YouTubeStatus, String>),
    YouTubeLibrarySynced {
        notify: bool,
        result: Result<YouTubeLibrarySnapshot, String>,
    },
    YouTubeRatingChanged {
        request_id: u64,
        item: YouTubeItem,
        liked: bool,
        result: Result<bool, String>,
    },
    YouTubePlaylistCreated {
        result: Result<YouTubePlaylistCreation, String>,
    },
    YouTubeLikeReconciled {
        request_id: u64,
        video_id: String,
        optimistic_liked: bool,
        result: Result<YouTubeLibrarySnapshot, String>,
    },
    YouTubeCollectionQueueLoaded {
        request_id: u64,
        item: YouTubeItem,
        playlist: bool,
        play_next: bool,
        result: Result<Vec<YouTubeItem>, String>,
    },
    YouTubeCollectionPlaybackLoaded {
        request_id: u64,
        item: YouTubeItem,
        playlist: bool,
        result: Result<Vec<YouTubeItem>, String>,
    },
    YouTubeBrowserPlaylist {
        request_id: u64,
        playlist: YouTubeItem,
        result: Result<Vec<YouTubeItem>, String>,
    },
    YouTubeBrowserPlaylistCoversCached {
        request_id: u64,
        playlist: YouTubeItem,
        items: Vec<YouTubeItem>,
    },
    YouTubeBrowserCollection {
        item: YouTubeItem,
        key: String,
        result: Result<Vec<YouTubeItem>, String>,
    },
    YouTubeArtistOverview {
        key: String,
        result: Result<YouTubeArtistOverview, String>,
    },
    YouTubePlaylistsCached(Result<HashMap<String, Vec<YouTubeItem>>, String>),
    YouTubeCollectionsCached(Result<HashMap<String, Vec<YouTubeItem>>, String>),
    YouTubeItems {
        title: String,
        result: Result<Vec<YouTubeItem>, String>,
    },
    YouTubeStructuredPage {
        request_id: u64,
        title: String,
        home: bool,
        append: bool,
        result: Result<YouTubeHomePage, String>,
    },
    YouTubeStructuredPageCoversCached {
        request_id: u64,
        title: String,
        home: bool,
        append: bool,
        page: YouTubeHomePage,
    },
    YouTubeGlobalSearch {
        request_id: u64,
        query: String,
        result: Result<YouTubeSearchResults, String>,
    },
    OfflineCollectionProgress {
        collection_id: String,
        completed: usize,
        total: usize,
        item: Box<YouTubeItem>,
        result: Result<PathBuf, String>,
    },
    OfflineCollectionFinished {
        collection_id: String,
        collection_title: String,
        completed: usize,
        failed: usize,
        automatic: bool,
    },
    YouTubeResolved {
        request_id: u64,
        queue: Vec<YouTubeItem>,
        index: usize,
        item: Box<YouTubeItem>,
        result: Result<(YouTubeStream, Option<PathBuf>), String>,
    },
    YouTubeRecoveryRetry {
        generation: u64,
        queue: Vec<YouTubeItem>,
        index: usize,
        item: Box<YouTubeItem>,
    },
}

pub(crate) fn youtube_home_response_is_current(
    home: bool,
    request_id: u64,
    current_request_id: u64,
) -> bool {
    !home || request_id == current_request_id
}

pub(crate) fn youtube_home_sections_changed(
    current: &YouTubeHomePage,
    incoming: &YouTubeHomePage,
) -> bool {
    current.sections != incoming.sections
}

pub(crate) struct BackgroundChannel {
    sender: Sender<BackgroundMessage>,
    receiver: Receiver<BackgroundMessage>,
}

impl BackgroundChannel {
    pub(crate) fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self { sender, receiver }
    }

    pub(crate) fn sender(&self) -> Sender<BackgroundMessage> {
        self.sender.clone()
    }

    pub(crate) fn try_recv(&self) -> Result<BackgroundMessage, TryRecvError> {
        self.receiver.try_recv()
    }
}

#[cfg(test)]
mod tests {
    use super::{youtube_home_response_is_current, youtube_home_sections_changed};
    use crate::youtube::{YouTubeHomePage, YouTubeHomeSection};

    #[test]
    fn rejects_stale_home_responses_but_accepts_non_home_pages() {
        assert!(youtube_home_response_is_current(true, 7, 7));
        assert!(!youtube_home_response_is_current(true, 6, 7));
        assert!(youtube_home_response_is_current(false, 0, 7));
    }

    #[test]
    fn detects_identical_and_changed_home_sections() {
        let current = YouTubeHomePage {
            sections: vec![YouTubeHomeSection {
                id: "quick".to_string(),
                title: "Quick picks".to_string(),
                ..YouTubeHomeSection::default()
            }],
            selected_chip_params: "first".to_string(),
            ..YouTubeHomePage::default()
        };
        let same_sections = YouTubeHomePage {
            sections: current.sections.clone(),
            selected_chip_params: "second".to_string(),
            ..YouTubeHomePage::default()
        };
        let changed = YouTubeHomePage {
            sections: vec![YouTubeHomeSection {
                id: "energy".to_string(),
                title: "Energy".to_string(),
                ..YouTubeHomeSection::default()
            }],
            ..YouTubeHomePage::default()
        };

        assert!(!youtube_home_sections_changed(&current, &same_sections));
        assert!(youtube_home_sections_changed(&current, &changed));
    }
}

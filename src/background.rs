// youtube_collection_background_playback_v1
// youtube_collection_queue_background_load_v1
// youtube_playlist_background_autoplay_v1
use crate::{
    lyrics::LyricLine,
    model::TrackData,
    youtube::{
        YouTubeArtistOverview, YouTubeItem, YouTubeLibrarySnapshot, YouTubeSearchResults,
        YouTubeStatus, YouTubeStream,
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

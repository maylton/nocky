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
    YouTubeResolved {
        request_id: u64,
        queue: Vec<YouTubeItem>,
        index: usize,
        item: Box<YouTubeItem>,
        result: Result<(YouTubeStream, Option<PathBuf>), String>,
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

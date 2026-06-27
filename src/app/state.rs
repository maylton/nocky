//! Application state shared by the controller and feature modules.

use crate::{lyrics::LyricLine, model::Track, youtube::YouTubeItem};
use std::path::PathBuf;

#[derive(Default)]
pub(crate) struct AppState {
    pub(crate) tracks: Vec<Track>,
    pub(crate) current: Option<usize>,
    pub(crate) playback_queue: Vec<usize>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum PlaybackSource {
    #[default]
    None,
    Local,
    YouTube,
}

#[derive(Clone, Debug)]
pub(crate) struct YouTubePlaybackState {
    pub(crate) queue: Vec<YouTubeItem>,
    pub(crate) current: usize,
    pub(crate) item: YouTubeItem,
    pub(crate) cover_path: Option<PathBuf>,
    pub(crate) lyrics: Vec<LyricLine>,
}

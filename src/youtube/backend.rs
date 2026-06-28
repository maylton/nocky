#![allow(dead_code)]

use super::{YouTubeBridge, YouTubeHomePage, YouTubeItem, YouTubeLibrarySnapshot, YouTubeStatus};

/// Stable boundary between the GTK application and the current YouTube Music
/// transport. The first implementation remains ytmusicapi-backed, while this
/// trait allows a future native InnerTube backend without changing page logic.
pub trait YouTubeMusicBackend {
    fn status(&self) -> Result<YouTubeStatus, String>;
    fn home_page(&self, continuation: Option<&str>) -> Result<YouTubeHomePage, String>;
    fn library_overview(&self) -> Result<YouTubeHomePage, String>;
    fn sync_library(&self) -> Result<YouTubeLibrarySnapshot, String>;
    fn search(&self, query: &str, filter: &str) -> Result<Vec<YouTubeItem>, String>;
    fn playlist(&self, playlist: &YouTubeItem) -> Result<Vec<YouTubeItem>, String>;
    fn rate(&self, video_id: &str, liked: bool) -> Result<bool, String>;
}

impl YouTubeMusicBackend for YouTubeBridge {
    fn status(&self) -> Result<YouTubeStatus, String> {
        YouTubeBridge::status(self)
    }

    fn home_page(&self, continuation: Option<&str>) -> Result<YouTubeHomePage, String> {
        YouTubeBridge::home_page(self, continuation)
    }

    fn library_overview(&self) -> Result<YouTubeHomePage, String> {
        YouTubeBridge::library_overview(self)
    }

    fn sync_library(&self) -> Result<YouTubeLibrarySnapshot, String> {
        YouTubeBridge::sync_library(self)
    }

    fn search(&self, query: &str, filter: &str) -> Result<Vec<YouTubeItem>, String> {
        YouTubeBridge::search(self, query, filter)
    }

    fn playlist(&self, playlist: &YouTubeItem) -> Result<Vec<YouTubeItem>, String> {
        YouTubeBridge::playlist(self, playlist)
    }

    fn rate(&self, video_id: &str, liked: bool) -> Result<bool, String> {
        YouTubeBridge::rate(self, video_id, liked)
    }
}

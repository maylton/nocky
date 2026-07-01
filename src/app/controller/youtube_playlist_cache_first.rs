//! Durable cache-first snapshots for YouTube Music playlists.

use super::AppController;

impl AppController {
    pub(crate) fn restore_playlist_first_paint_snapshot(&self) {}

    pub(crate) fn poll_playlist_snapshot_revalidation(&self) {}

    pub(crate) fn checkpoint_playlist_first_paint_snapshot(&self) {}
}

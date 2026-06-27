//! Helpers for reconciling scanned local library state.

use crate::model::{Track, TrackData};

pub(crate) fn scanned_library_matches(tracks: &[Track], data: &[TrackData]) -> bool {
    tracks.len() == data.len()
        && tracks.iter().zip(data).all(|(track, incoming)| {
            track.path == incoming.path
                && track.title == incoming.title
                && track.artist == incoming.artist
                && track.album == incoming.album
                && track.duration_seconds == incoming.duration_seconds
                && track.disc_number == incoming.disc_number
                && track.track_number == incoming.track_number
                && track.cover_path == incoming.cover_path
        })
}

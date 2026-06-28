//! Media formatting and playback helper utilities.

use crate::{config, youtube::error};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    path::Path,
};

pub(crate) fn is_refreshable_stream_error(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    let network_source = message.contains("gstsouphttpsrc")
        || message.contains("souphttpsrc")
        || message.contains("googlevideo.com");
    let rejected = message.contains("forbidden")
        || message.contains("(403)")
        || message.contains("http 403")
        || message.contains("unauthorized")
        || message.contains("(401)")
        || message.contains("gone")
        || message.contains("(410)");
    let transient_network = message.contains("connection reset")
        || message.contains("connection timed out")
        || message.contains("timed out")
        || message.contains("temporary failure")
        || message.contains("network is unreachable")
        || message.contains("host is unreachable")
        || message.contains("could not connect")
        || message.contains("internal data stream error")
        || message.contains("resource not found");

    network_source && (rejected || transient_network)
}

pub(crate) fn playback_error_message(message: &str) -> &'static str {
    error::classify_youtube_playback_error(message).message(config::AppConfig::load().language)
}

pub(crate) fn redact_stream_url(message: &str) -> String {
    let Some(url_marker) = message.find("URL: http") else {
        return message.to_string();
    };
    let url_start = url_marker + "URL: ".len();
    let tail = message.get(url_start..).unwrap_or_default();
    let url_end = tail
        .find(", Redirect")
        .or_else(|| tail.find(char::is_whitespace))
        .unwrap_or(tail.len());

    let mut redacted = String::with_capacity(message.len().min(512));
    redacted.push_str(message.get(..url_start).unwrap_or_default());
    redacted.push_str("<redacted>");
    redacted.push_str(tail.get(url_end..).unwrap_or_default());
    redacted
}

pub(crate) fn mpris_track_id(path: &Path) -> String {
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    format!("/io/github/maylton/Nocky/track_{:016x}", hasher.finish())
}

pub(crate) fn mpris_youtube_track_id(video_id: &str) -> String {
    let mut hasher = DefaultHasher::new();
    video_id.hash(&mut hasher);
    format!("/io/github/maylton/Nocky/youtube_{:016x}", hasher.finish())
}

pub(crate) fn format_time(microseconds: i64) -> String {
    let total_seconds = (microseconds / 1_000_000).max(0);
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{minutes}:{seconds:02}")
}

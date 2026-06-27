// local_artist_index_foundation_v3
// stable_artist_directory_refresh_v1
// stable_collection_identity_and_deferred_cache_v2
// clickable_player_artist_album_navigation_v1
// artist_profile_revalidation_v5
// youtube_like_reconciliation_and_request_guard_v1
// youtube_like_button_and_track_menu_v2
// youtube_real_like_sync_v5
// source_aware_liked_songs_page_v1
// clickable_lyrics_seek_v3
// fix_resume_seek_oscillation_v1
// fix_shutdown_save_order_v1
// youtube_resume_seek_convergence_v1
// lyrics_2_v2
// playback_resume_preferences_fix_v1
// playback_persistence_resume_2_v1
// queue_collection_cover_fallback_v1
// preserve_home_carousel_scroll_v1
// collection_card_inline_loading_fix_v2
// youtube_collection_background_playback_v1
// collection_card_loading_spinner_v3\n// youtube_collection_queue_background_load_v1
// collection_card_overflow_and_play_state_v2
// youtube_playlist_background_autoplay_v1
// contextual_collection_controls_v5
// recent_activity_exact_fix_v1
// personalized_home_resume_v2
#[path = "ui/widgets/animated_page_switcher.rs"]
mod animated_page_switcher;
mod app;
mod artist_index;
mod background;
mod background_handler;
mod browser;
#[path = "ui/widgets/compact_volume_motion.rs"]
mod compact_volume_motion;
mod config;
#[path = "ui/widgets/cover.rs"]
mod cover_view;
mod dialogs;
#[path = "ui/widgets/expressive_transport.rs"]
mod expressive_transport;
#[path = "ui/footer/layout.rs"]
mod footer_layout;
#[path = "ui/footer/now_playing.rs"]
mod footer_now_playing;
#[path = "ui/footer/progress.rs"]
mod footer_progress;
#[path = "ui/footer/transport.rs"]
mod footer_transport;
#[path = "ui/footer/utilities.rs"]
mod footer_utilities;
#[path = "ui/footer/view.rs"]
mod footer_view;
mod i18n;
mod integrations;
mod library;
mod listening_history;
mod local_mix_cover;
mod search_text;
// material_dynamic_palette_v1
mod lyrics;
#[path = "lyrics/provider.rs"]
mod lyrics_provider;
#[path = "lyrics/view.rs"]
mod lyrics_view;
mod material_palette;
mod md3_volume;
mod mode_toggle;
mod model;
mod offline_store;
mod onboarding;
pub mod playback;
#[path = "ui/player/view.rs"]
mod player_view;
mod reveal_bounce;
#[path = "ui/settings/page.rs"]
mod settings_page;
mod theme;
mod theme_css;
mod visual_theme;
mod visualizer;
#[path = "ui/widgets/wave_progress.rs"]
mod wave_progress;
mod youtube;
#[path = "youtube/diagnostics.rs"]
mod youtube_diagnostics;
#[path = "youtube/error.rs"]
mod youtube_error;
#[path = "youtube/playback.rs"]
mod youtube_playback;

use gtk::glib;
use model::{Track, TrackData};
use std::{
    collections::{hash_map::DefaultHasher, HashSet},
    hash::{Hash, Hasher},
    path::Path,
};
use youtube::{YouTubeBridge, YouTubeItem, YouTubeLibraryCache};

const APP_ID: &str = "io.github.maylton.Nocky";
const HOME_PLAYER_WIDTH: i32 = 454;

fn resolve_youtube_collection_item(
    bridge: &YouTubeBridge,
    item: &YouTubeItem,
    filter: &str,
) -> Result<YouTubeItem, String> {
    if !item.browse_id.trim().is_empty() {
        return Ok(item.clone());
    }

    let query = item.title.trim();
    if query.is_empty() {
        return Err("The YouTube Music collection has no title".to_string());
    }

    let mut candidates = bridge.search(query, filter)?;
    candidates.retain(|candidate| {
        candidate
            .result_type
            .eq_ignore_ascii_case(item.result_type.as_str())
            || candidate
                .result_type
                .eq_ignore_ascii_case(filter.trim_end_matches('s'))
    });

    candidates
        .iter()
        .position(|candidate| {
            candidate.title.eq_ignore_ascii_case(query)
                && (item.artist.trim().is_empty()
                    || candidate.artist.eq_ignore_ascii_case(item.artist.trim()))
        })
        .or_else(|| {
            candidates
                .iter()
                .position(|candidate| candidate.title.eq_ignore_ascii_case(query))
        })
        .map(|index| candidates.remove(index))
        .or_else(|| candidates.into_iter().next())
        .ok_or_else(|| {
            format!(
                "No YouTube Music {} could be resolved for '{}'",
                item.result_type, item.title
            )
        })
}

fn scanned_library_matches(tracks: &[Track], data: &[TrackData]) -> bool {
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

fn main() -> glib::ExitCode {
    app::run()
}

fn is_refreshable_stream_error(message: &str) -> bool {
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

fn youtube_home_prefetch_candidates(library: &YouTubeLibraryCache) -> Vec<YouTubeItem> {
    let mut seen = HashSet::new();
    let mut candidates = Vec::new();
    for playlist in library
        .playlists
        .iter()
        .filter(|playlist| youtube_playlist_is_mix(playlist))
        .chain(
            library
                .playlists
                .iter()
                .filter(|playlist| !youtube_playlist_is_mix(playlist)),
        )
        .filter(|playlist| !playlist.browse_id.is_empty())
        .filter(|playlist| {
            library
                .playlist_tracks
                .get(&playlist.browse_id)
                .map(|items| items.is_empty())
                .unwrap_or(true)
        })
    {
        if seen.insert(playlist.browse_id.clone()) {
            candidates.push(playlist.clone());
        }
        if candidates.len() >= 24 {
            break;
        }
    }
    candidates
}

fn youtube_playlist_is_mix(playlist: &YouTubeItem) -> bool {
    if playlist.playlist_kind == "mix" {
        return true;
    }
    let title = playlist.title.to_lowercase();
    title.contains("mix") || title.contains("radio") || title.contains("supermix")
}

fn playback_error_message(message: &str) -> &'static str {
    youtube_error::classify_youtube_playback_error(message)
        .message(config::AppConfig::load().language)
}

fn redact_stream_url(message: &str) -> String {
    let Some(url_marker) = message.find("URL: http") else {
        return message.to_string();
    };
    let url_start = url_marker + "URL: ".len();
    let tail = &message[url_start..];
    let url_end = tail
        .find(", Redirect")
        .or_else(|| tail.find(char::is_whitespace))
        .unwrap_or(tail.len());

    let mut redacted = String::with_capacity(message.len().min(512));
    redacted.push_str(&message[..url_start]);
    redacted.push_str("<redacted>");
    redacted.push_str(&tail[url_end..]);
    redacted
}

fn mpris_track_id(path: &Path) -> String {
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    format!("/io/github/maylton/Nocky/track_{:016x}", hasher.finish())
}

fn mpris_youtube_track_id(video_id: &str) -> String {
    let mut hasher = DefaultHasher::new();
    video_id.hash(&mut hasher);
    format!("/io/github/maylton/Nocky/youtube_{:016x}", hasher.finish())
}

fn format_time(microseconds: i64) -> String {
    let total_seconds = (microseconds / 1_000_000).max(0);
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{minutes}:{seconds:02}")
}

//! Helpers for resolving and prefetching YouTube Music collections.

use crate::youtube::{YouTubeBridge, YouTubeItem, YouTubeLibraryCache};
use std::collections::HashSet;

pub(crate) fn resolve_youtube_collection_item(
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

pub(crate) fn youtube_home_prefetch_candidates(library: &YouTubeLibraryCache) -> Vec<YouTubeItem> {
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

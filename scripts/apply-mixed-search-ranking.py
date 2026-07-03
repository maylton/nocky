#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path.cwd()
MAIN = ROOT / "src/main.rs"
BROWSER = ROOT / "src/browser.rs"
RANKING = ROOT / "src/search_ranking.rs"
CONSTRUCTION = ROOT / "src/app/controller/construction.rs"
SEARCH_STYLE = ROOT / "assets/themes/material-expressive/102-search-history.css"
ROADMAP = ROOT / "ROADMAP.md"
DOC = ROOT / "docs/SEARCH_RANKING.md"


class PatchError(RuntimeError):
    pass


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count == 0 and new in text:
        print(f"[already applied] {label}")
        return text
    if count != 1:
        raise PatchError(f"{label}: expected one match, found {count}")
    print(f"[changed] {label}")
    return text.replace(old, new, 1)


RANKING_SOURCE = r'''use crate::search_text::normalize_search_text;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SearchSource {
    Local,
    YouTube,
}

impl SearchSource {
    fn penalty(self) -> u8 {
        match self {
            Self::Local => 0,
            Self::YouTube => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct SearchRank {
    tier: u8,
    source_penalty: u8,
    title_distance: usize,
    metadata_length: usize,
}

pub(crate) fn rank_search_document(
    raw_query: &str,
    source: SearchSource,
    title: &str,
    artist: &str,
    album: &str,
    extra: &str,
) -> Option<SearchRank> {
    let query = normalize_search_text(raw_query);
    if query.is_empty() {
        return Some(SearchRank {
            tier: 0,
            source_penalty: source.penalty(),
            title_distance: 0,
            metadata_length: 0,
        });
    }

    let title = normalize_search_text(title);
    let artist = normalize_search_text(artist);
    let album = normalize_search_text(album);
    let extra = normalize_search_text(extra);
    let combined = [title.as_str(), artist.as_str(), album.as_str(), extra.as_str()]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let terms = query.split_whitespace().collect::<Vec<_>>();

    if !contains_all_terms(&combined, &terms) {
        return None;
    }

    let tier = if title == query {
        0
    } else if title.starts_with(&query) {
        1
    } else if title.contains(&query) {
        2
    } else if contains_all_terms(&title, &terms) {
        3
    } else if artist == query {
        4
    } else if artist.starts_with(&query) {
        5
    } else if artist.contains(&query) {
        6
    } else if album == query {
        7
    } else if album.starts_with(&query) {
        8
    } else if album.contains(&query) {
        9
    } else if contains_all_terms(&format!("{title} {artist}"), &terms) {
        10
    } else {
        11
    };

    Some(SearchRank {
        tier,
        source_penalty: source.penalty(),
        title_distance: title.len().abs_diff(query.len()),
        metadata_length: combined.len(),
    })
}

fn contains_all_terms(text: &str, terms: &[&str]) -> bool {
    terms.iter().all(|term| text.contains(term))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_wins_when_relevance_is_equal() {
        let local = rank_search_document(
            "muse",
            SearchSource::Local,
            "Muse",
            "",
            "",
            "",
        )
        .expect("local result should match");
        let remote = rank_search_document(
            "muse",
            SearchSource::YouTube,
            "Muse",
            "",
            "",
            "",
        )
        .expect("remote result should match");

        assert!(local < remote);
    }

    #[test]
    fn stronger_remote_title_match_beats_weaker_local_metadata_match() {
        let local = rank_search_document(
            "hysteria",
            SearchSource::Local,
            "Live at Wembley",
            "Muse",
            "Hysteria Collection",
            "",
        )
        .expect("local result should match");
        let remote = rank_search_document(
            "hysteria",
            SearchSource::YouTube,
            "Hysteria",
            "Muse",
            "Absolution",
            "",
        )
        .expect("remote result should match");

        assert!(remote < local);
    }

    #[test]
    fn title_terms_rank_ahead_of_terms_scattered_across_metadata() {
        let title_match = rank_search_document(
            "dark side",
            SearchSource::YouTube,
            "The Dark Side of the Moon",
            "Pink Floyd",
            "",
            "",
        )
        .expect("title should match");
        let scattered = rank_search_document(
            "dark side",
            SearchSource::Local,
            "Darkness",
            "Side Project",
            "Collection",
            "",
        )
        .expect("metadata should match");

        assert!(title_match < scattered);
    }

    #[test]
    fn normalization_keeps_accented_metadata_searchable() {
        let rank = rank_search_document(
            "beyonce",
            SearchSource::Local,
            "Beyoncé",
            "",
            "",
            "",
        );
        assert!(rank.is_some());
    }

    #[test]
    fn unrelated_documents_are_excluded() {
        assert!(rank_search_document(
            "radiohead",
            SearchSource::YouTube,
            "Hysteria",
            "Muse",
            "Absolution",
            "",
        )
        .is_none());
    }
}
'''

DOC_SOURCE = r'''# Mixed local and remote search ranking

## Scope

When YouTube Music is the active source, global search now combines matching
local-library items with the current remote result pages. Local-only mode remains
strictly local and never starts or displays YouTube queries.

## Ranking contract

The same pure relevance contract is used for tracks, albums, artists and
playlists:

1. exact title;
2. title prefix;
3. title phrase and title-token matches;
4. exact or prefix artist matches;
5. exact or prefix album matches;
6. terms distributed across the remaining metadata.

Source is only a tie-breaker. A local item wins when relevance is otherwise
equal, while a stronger remote title match still outranks a weak local metadata
match. Accents, punctuation and repeated whitespace use the existing normalized
search text contract.

## Mixed-source behavior

- local matches render immediately while the remote debounce or refresh runs;
- remote pages continue to paginate independently by category;
- local and YouTube rows retain their existing source-specific actions;
- activating a YouTube track still builds a queue from the visible YouTube
  results only;
- local-only mode keeps all remote results hidden;
- stable source identities prevent accidental same-source duplicates without
  collapsing a local file and a remote catalog item into one action target.

## Tests

The ranking module covers equal-relevance source preference, stronger remote
matches, distributed metadata terms, accent normalization and unrelated-result
exclusion.

## Deferred

Route-aware cancellation of unnecessary remote requests and result-update
announcements remain the next search checkpoints.
'''

TRACK_RANKING_BLOCK = r'''        let mut ranked_tracks = Vec::new();
        for (index, track) in tracks.iter().enumerate() {
            let haystack = format!("{} {} {}", track.title, track.artist, track.album);
            let Some(rank) = rank_search_document(
                &query,
                SearchSource::Local,
                &track.title,
                &track.artist,
                &track.album,
                "",
            ) else {
                continue;
            };
            ranked_tracks.push((
                rank,
                search_score(&haystack, &query),
                normalize_search_text(&track.title),
                track.path.to_string_lossy().into_owned(),
                VisibleTrack::Local(index),
            ));
        }

        if online_state_matches {
            for item in youtube.search.songs.iter().filter(|item| item.playable()) {
                let haystack = format!("{} {} {}", item.title, item.artist, item.album);
                let Some(rank) = rank_search_document(
                    &query,
                    SearchSource::YouTube,
                    &item.title,
                    &item.artist,
                    &item.album,
                    &item.subtitle,
                ) else {
                    continue;
                };
                ranked_tracks.push((
                    rank,
                    search_score(&haystack, &query),
                    normalize_search_text(&item.title),
                    item.video_id.clone(),
                    VisibleTrack::YouTube(Box::new(item.clone())),
                ));
            }
        }

        ranked_tracks.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| left.1.cmp(&right.1))
                .then_with(|| left.2.cmp(&right.2))
                .then_with(|| left.3.cmp(&right.3))
        });
        let track_matches = ranked_tracks
            .into_iter()
            .map(|(_, _, _, _, entry)| entry)
            .collect::<Vec<_>>();

'''

CARD_RANKING_BLOCK = r'''fn search_card_rank(card: &HomeCard, query: &str) -> Option<SearchRank> {
    match card {
        HomeCard::LocalAlbum {
            title,
            subtitle,
            detail,
            ..
        } => rank_search_document(
            query,
            SearchSource::Local,
            title,
            subtitle,
            "",
            detail,
        ),
        HomeCard::YouTubeAlbum {
            item,
            subtitle,
            detail,
            ..
        } => rank_search_document(
            query,
            SearchSource::YouTube,
            &item.title,
            if item.artist.is_empty() {
                subtitle
            } else {
                &item.artist
            },
            &item.album,
            detail,
        ),
        HomeCard::LocalArtist {
            title,
            subtitle,
            detail,
            ..
        } => rank_search_document(
            query,
            SearchSource::Local,
            title,
            subtitle,
            "",
            detail,
        ),
        HomeCard::YouTubeArtist {
            item,
            subtitle,
            detail,
            ..
        } => rank_search_document(
            query,
            SearchSource::YouTube,
            &item.title,
            &item.artist,
            "",
            &format!("{subtitle} {detail}"),
        ),
        HomeCard::LocalPlaylist { title, subtitle } => rank_search_document(
            query,
            SearchSource::Local,
            title,
            "",
            "",
            subtitle,
        ),
        HomeCard::LocalMix {
            title,
            subtitle,
            detail,
            ..
        } => rank_search_document(
            query,
            SearchSource::Local,
            title,
            subtitle,
            "",
            detail,
        ),
        HomeCard::YouTubeTrack { item, .. } => rank_search_document(
            query,
            SearchSource::YouTube,
            &item.title,
            &item.artist,
            &item.album,
            &item.subtitle,
        ),
        HomeCard::YouTubePlaylist(item) => rank_search_document(
            query,
            SearchSource::YouTube,
            &item.title,
            &item.artist,
            &item.album,
            &format!("{} {}", item.subtitle, item.playlist_kind),
        ),
    }
}

fn rank_search_cards(cards: Vec<HomeCard>, query: &str) -> Vec<HomeCard> {
    let mut seen = HashSet::new();
    let mut ranked = cards
        .into_iter()
        .filter(|card| seen.insert(home_card_identity(card)))
        .filter_map(|card| {
            let text = search_card_text(&card);
            let rank = search_card_rank(&card, query)?;
            Some((
                rank,
                search_score(&text, query),
                normalize_search_text(&text),
                card,
            ))
        })
        .collect::<Vec<_>>();

    ranked.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1))
            .then_with(|| left.2.cmp(&right.2))
            .then_with(|| home_card_identity(&left.3).cmp(&home_card_identity(&right.3)))
    });

    ranked
        .into_iter()
        .map(|(_, _, _, card)| card)
        .collect()
}

'''

ALBUM_CARDS_BLOCK = r'''fn search_album_cards(
    tracks: &[Track],
    youtube: &YouTubeLibraryCache,
    query: &str,
    online_state_matches: bool,
) -> Vec<HomeCard> {
    let mut cards = Vec::new();
    let mut groups: BTreeMap<String, Vec<&Track>> = BTreeMap::new();
    for track in tracks {
        groups.entry(track.album.clone()).or_default().push(track);
    }
    for (album, album_tracks) in groups {
        let artists = album_tracks
            .iter()
            .flat_map(|track| credited_artists(&track.artist))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join(", ");
        if !search_matches(&format!("{album} {artists}"), query) {
            continue;
        }
        cards.push(HomeCard::LocalAlbum {
            title: album,
            subtitle: artists,
            detail: format!("Local • {} faixas", album_tracks.len()),
            cover_path: album_tracks
                .iter()
                .find_map(|track| track.cover_path.clone()),
        });
    }

    if online_state_matches {
        cards.extend(
            youtube
                .search
                .albums
                .iter()
                .cloned()
                .map(|item| HomeCard::YouTubeAlbum {
                    subtitle: if item.artist.is_empty() {
                        item.subtitle.clone()
                    } else {
                        item.artist.clone()
                    },
                    detail: "Álbum • YouTube Music".to_string(),
                    cover_path: item.cached_cover().map(Path::to_path_buf),
                    item,
                }),
        );
    }
    rank_search_cards(cards, query)
}

'''

ARTIST_CARDS_BLOCK = r'''fn search_artist_cards(
    tracks: &[Track],
    youtube: &YouTubeLibraryCache,
    query: &str,
    online_state_matches: bool,
) -> Vec<HomeCard> {
    let mut cards = Vec::new();
    let mut groups: BTreeMap<String, Vec<&Track>> = BTreeMap::new();
    for track in tracks {
        groups.entry(track.artist.clone()).or_default().push(track);
    }
    for (artist, artist_tracks) in groups {
        if !search_matches(&artist, query) {
            continue;
        }
        cards.push(HomeCard::LocalArtist {
            title: artist,
            subtitle: String::new(),
            detail: format!("Local • {} faixas", artist_tracks.len()),
            cover_path: artist_tracks
                .iter()
                .find_map(|track| track.cover_path.clone()),
        });
    }

    if online_state_matches {
        cards.extend(
            youtube
                .search
                .artists
                .iter()
                .cloned()
                .map(|item| HomeCard::YouTubeArtist {
                    subtitle: if item.subtitle.is_empty() {
                        "Artista".to_string()
                    } else {
                        item.subtitle.clone()
                    },
                    detail: "Artista • YouTube Music".to_string(),
                    cover_path: item.cached_cover().map(Path::to_path_buf),
                    item,
                }),
        );
    }
    rank_search_cards(cards, query)
}

'''

PLAYLIST_CARDS_BLOCK = r'''fn search_playlist_cards(
    _tracks: &[Track],
    config: &AppConfig,
    youtube: &YouTubeLibraryCache,
    query: &str,
    online_state_matches: bool,
) -> Vec<HomeCard> {
    let mut cards = Vec::new();
    for playlist in &config.playlists {
        if search_matches(&playlist.name, query) {
            cards.push(HomeCard::LocalPlaylist {
                title: playlist.name.clone(),
                subtitle: format!("{} faixas locais", playlist.tracks.len()),
            });
        }
    }

    if online_state_matches {
        cards.extend(
            youtube
                .search
                .playlists
                .iter()
                .cloned()
                .map(HomeCard::YouTubePlaylist),
        );
    }
    rank_search_cards(cards, query)
}

'''


def patch_main(text: str) -> str:
    return replace_once(
        text,
        "mod search_text;\n",
        "mod search_ranking;\nmod search_text;\n",
        "Register mixed search ranking module",
    )


def patch_browser(text: str) -> str:
    text = replace_once(
        text,
        "    offline_store::OfflineStore,\n    search_text::{normalize_search_text, search_matches, search_score},\n",
        "    offline_store::OfflineStore,\n    search_ranking::{rank_search_document, SearchRank, SearchSource},\n    search_text::{normalize_search_text, search_matches, search_score},\n",
        "Import mixed search ranking contract",
    )

    old_tracks = '''        let mut track_matches = Vec::new();
        if local_mode {
            let mut indices = (0..tracks.len()).collect::<Vec<_>>();
            indices.sort_by(|left, right| compare_library_tracks(&tracks[*left], &tracks[*right]));
            let mut ranked_matches = Vec::new();
            for index in indices {
                let track = &tracks[index];
                let haystack = format!("{} {} {}", track.title, track.artist, track.album);
                if search_matches(&haystack, &query) {
                    ranked_matches.push((search_score(&haystack, &query), index));
                }
            }
            ranked_matches.sort_by_key(|(score, _)| *score);
            track_matches.extend(
                ranked_matches
                    .into_iter()
                    .map(|(_, index)| VisibleTrack::Local(index)),
            );
        } else if online_state_matches {
            track_matches.extend(
                youtube
                    .search
                    .songs
                    .iter()
                    .filter(|item| item.playable())
                    .cloned()
                    .map(|item| VisibleTrack::YouTube(Box::new(item))),
            );
        }

'''
    text = replace_once(text, old_tracks, TRACK_RANKING_BLOCK, "Rank mixed track results")

    old_card_ranking = '''fn rank_search_cards(cards: Vec<HomeCard>, query: &str) -> Vec<HomeCard> {
    let mut seen = HashSet::new();
    let mut ranked = cards
        .into_iter()
        .filter(|card| seen.insert(home_card_identity(card)))
        .map(|card| {
            let text = search_card_text(&card);
            let score = search_score(&text, query);
            (score, normalize_search_text(&text), card)
        })
        .collect::<Vec<_>>();

    ranked.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1))
            .then_with(|| home_card_identity(&left.2).cmp(&home_card_identity(&right.2)))
    });

    ranked.into_iter().map(|(_, _, card)| card).collect()
}

'''
    text = replace_once(
        text,
        old_card_ranking,
        CARD_RANKING_BLOCK,
        "Rank mixed collection results",
    )

    album_start = text.index("fn search_album_cards(\n")
    artist_start = text.index("fn search_artist_cards(\n", album_start)
    text = text[:album_start] + ALBUM_CARDS_BLOCK + text[artist_start:]
    print("[changed] Mix local and remote album results")

    artist_start = text.index("fn search_artist_cards(\n")
    playlist_start = text.index("fn search_playlist_cards(\n", artist_start)
    text = text[:artist_start] + ARTIST_CARDS_BLOCK + text[playlist_start:]
    print("[changed] Mix local and remote artist results")

    playlist_start = text.index("fn search_playlist_cards(\n")
    home_album_start = text.index("fn home_album_cards(\n", playlist_start)
    text = text[:playlist_start] + PLAYLIST_CARDS_BLOCK + text[home_album_start:]
    print("[changed] Mix local and remote playlist results")
    return text


def patch_search_style(text: str) -> str:
    return replace_once(
        text,
        '''  background-color: alpha(@m3_surface_container_high, 0.94);
  border: 1px solid alpha(@m3_outline_variant, 0.62);
  box-shadow: none;
''',
        '''  background-color: @m3_surface_container_high;
  border: 1px solid alpha(@m3_outline, 0.34);
  box-shadow:
    0 8px 22px alpha(black, 0.18),
    inset 0 0 0 1px alpha(@m3_primary, 0.04);
''',
        "Restore recent-search card surface",
    )


def patch_roadmap(text: str) -> str:
    text = replace_once(
        text,
        "- 🟡 Better ranking across mixed local and remote results.\n",
        "- 🟡 Cancellation of unnecessary remote requests after route changes.\n",
        "Advance active search checkpoint",
    )
    anchor = "- ✅ Local recent-query history with MRU ordering, individual removal and clear-all controls.\n"
    text = replace_once(
        text,
        anchor,
        anchor + "- ✅ Relevance-ranked mixed local and remote results while YouTube Music is active.\n",
        "Document completed mixed search ranking",
    )
    text = replace_once(
        text,
        "- Better ranking across mixed local and remote results.\n",
        "",
        "Remove completed mixed ranking item",
    )
    return replace_once(
        text,
        "8. Improve mixed-source ranking and route-aware cancellation.\n",
        "8. Add route-aware remote search cancellation.\n",
        "Advance recommended search order",
    )


def main() -> int:
    required = [MAIN, BROWSER, CONSTRUCTION, SEARCH_STYLE, ROADMAP]
    missing = [path for path in required if not path.is_file()]
    if missing:
        print("Run this script from the Nocky repository root.", file=sys.stderr)
        for path in missing:
            print(f"missing: {path}", file=sys.stderr)
        return 1

    original = {path: path.read_text(encoding="utf-8") for path in required}
    if "search_history_revealer" not in original[CONSTRUCTION]:
        print("ERROR: apply and validate the recent-search dropdown checkpoint first.", file=sys.stderr)
        return 1
    if "YouTubeSearchCategory" not in original[BROWSER]:
        print("ERROR: apply and validate real remote search pagination first.", file=sys.stderr)
        return 1

    creations = [(RANKING, RANKING_SOURCE), (DOC, DOC_SOURCE)]
    for path, expected in creations:
        if path.exists() and path.read_text(encoding="utf-8") != expected:
            print(f"ERROR: {path} already exists with different content.", file=sys.stderr)
            print("No files were written.", file=sys.stderr)
            return 1

    updated = dict(original)
    try:
        updated[MAIN] = patch_main(updated[MAIN])
        updated[BROWSER] = patch_browser(updated[BROWSER])
        updated[SEARCH_STYLE] = patch_search_style(updated[SEARCH_STYLE])
        updated[ROADMAP] = patch_roadmap(updated[ROADMAP])
    except (PatchError, ValueError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        print("No files were written.", file=sys.stderr)
        return 1

    changed: list[Path] = []
    for path in required:
        if updated[path] != original[path]:
            path.write_text(updated[path], encoding="utf-8")
            changed.append(path.relative_to(ROOT))

    for path, content in creations:
        if not path.exists():
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8")
            changed.append(path.relative_to(ROOT))

    print("Mixed local and remote search ranking patch applied successfully.")
    for path in changed:
        print(f"  {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path.cwd()
BROWSER = ROOT / "src/browser.rs"
ROADMAP = ROOT / "ROADMAP.md"
DOC = ROOT / "docs/SEARCH_ACCESSIBILITY.md"


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


DOC_SOURCE = r'''# Search result accessibility announcements

## Scope

The categorized search page now updates its accessible label every time the
search result tree is rebuilt. This gives assistive technologies one compact
summary of the current query, loading state and category counts without adding
extra visible chrome to the interface.

## Announcement contract

For each rebuild, Nocky summarizes:

- the active query;
- whether YouTube Music is still updating results;
- total result count;
- visible category totals for tracks, albums, artists and playlists.

The message is localized in Portuguese, English and Spanish and is exposed
through GTK's accessible property API on the search results container.

## Behavior

- Local-only search announces local counts only.
- YouTube Music search announces synchronized/remote counts for the active
  source without mixing in the local library.
- Loading state changes update the announcement even when the visible result
  counts are unchanged.
- Existing keyboard row activation and source-specific actions are preserved.

## Deferred

Future polish can use toolkit-level live-region APIs when the project moves to a
GTK/gtk-rs version that exposes reliable cross-screen-reader announcement
priorities. This checkpoint keeps the implementation dependency-free and stable
with the current GTK bindings.
'''

ANNOUNCEMENT_HELPERS = r'''fn search_results_announcement(
    language: AppLanguage,
    query: &str,
    total: usize,
    tracks: usize,
    albums: usize,
    artists: usize,
    playlists: usize,
    loading: bool,
) -> String {
    let query = query.trim();
    match language {
        AppLanguage::Portuguese => {
            let state = if loading {
                "Atualizando resultados"
            } else {
                "Resultados atualizados"
            };
            format!(
                "{state} para ‘{query}’: {total} no total, {tracks} faixas, {albums} álbuns, {artists} artistas e {playlists} playlists."
            )
        }
        AppLanguage::English => {
            let state = if loading {
                "Updating results"
            } else {
                "Results updated"
            };
            format!(
                "{state} for ‘{query}’: {total} total, {tracks} tracks, {albums} albums, {artists} artists and {playlists} playlists."
            )
        }
        AppLanguage::Spanish => {
            let state = if loading {
                "Actualizando resultados"
            } else {
                "Resultados actualizados"
            };
            format!(
                "{state} para ‘{query}’: {total} en total, {tracks} canciones, {albums} álbumes, {artists} artistas y {playlists} playlists."
            )
        }
    }
}

fn update_search_results_accessible_summary(
    widget: &impl IsA<gtk::Widget>,
    language: AppLanguage,
    query: &str,
    total: usize,
    tracks: usize,
    albums: usize,
    artists: usize,
    playlists: usize,
    loading: bool,
) {
    let message = search_results_announcement(
        language, query, total, tracks, albums, artists, playlists, loading,
    );
    widget.update_property(&[gtk::accessible::Property::Label(&message)]);
}

'''

ANNOUNCEMENT_TESTS = r'''
#[cfg(test)]
mod search_accessibility_tests {
    use super::*;

    #[test]
    fn portuguese_announcement_mentions_loading_query_and_counts() {
        let message = search_results_announcement(
            AppLanguage::Portuguese,
            "Muse",
            10,
            4,
            2,
            3,
            1,
            true,
        );
        assert!(message.contains("Atualizando resultados"));
        assert!(message.contains("Muse"));
        assert!(message.contains("10 no total"));
        assert!(message.contains("4 faixas"));
    }

    #[test]
    fn english_announcement_switches_to_updated_state() {
        let message = search_results_announcement(
            AppLanguage::English,
            "Radiohead",
            3,
            1,
            1,
            1,
            0,
            false,
        );
        assert!(message.starts_with("Results updated"));
        assert!(message.contains("3 total"));
        assert!(message.contains("0 playlists"));
    }

    #[test]
    fn spanish_announcement_keeps_category_totals() {
        let message = search_results_announcement(
            AppLanguage::Spanish,
            "Soda Stereo",
            7,
            2,
            2,
            2,
            1,
            false,
        );
        assert!(message.contains("7 en total"));
        assert!(message.contains("2 canciones"));
        assert!(message.contains("1 playlists"));
    }
}
'''


def patch_browser(text: str) -> str:
    text = replace_once(
        text,
        "fn search_section_heading(\n",
        ANNOUNCEMENT_HELPERS + "fn search_section_heading(\n",
        "Add localized search result announcement helpers",
    )

    old = '''        self.search_content.append(&track_section);

        self.search_content.append(&search_list_section(
            copy.albums,
            copy.no_albums,
            search_album_cards(tracks, youtube, &query, online_state_matches),
            self.search_album_limit.clone(),
            &self.event_tx,
            loading,
            copy,
            config,
            playback,
            YouTubeSearchCategory::Albums,
            if online_state_matches {
                youtube.search.continuation(YouTubeSearchCategory::Albums)
            } else {
                ""
            },
            online_state_matches && youtube.search.loading_more(YouTubeSearchCategory::Albums),
        ));
        self.search_content.append(&search_list_section(
            copy.artists,
            copy.no_artists,
            search_artist_cards(tracks, youtube, &query, online_state_matches),
            self.search_artist_limit.clone(),
            &self.event_tx,
            loading,
            copy,
            config,
            playback,
            YouTubeSearchCategory::Artists,
            if online_state_matches {
                youtube.search.continuation(YouTubeSearchCategory::Artists)
            } else {
                ""
            },
            online_state_matches && youtube.search.loading_more(YouTubeSearchCategory::Artists),
        ));
        self.search_content.append(&search_list_section(
            copy.playlists,
            copy.no_playlists,
            search_playlist_cards(tracks, config, youtube, &query, online_state_matches),
            self.search_playlist_limit.clone(),
            &self.event_tx,
            loading,
            copy,
            config,
            playback,
            YouTubeSearchCategory::Playlists,
            if online_state_matches {
                youtube
                    .search
                    .continuation(YouTubeSearchCategory::Playlists)
            } else {
                ""
            },
            online_state_matches
                && youtube
                    .search
                    .loading_more(YouTubeSearchCategory::Playlists),
        ));
'''
    new = '''        self.search_content.append(&track_section);

        let album_matches = search_album_cards(tracks, youtube, &query, online_state_matches);
        let artist_matches = search_artist_cards(tracks, youtube, &query, online_state_matches);
        let playlist_matches = search_playlist_cards(
            tracks,
            config,
            youtube,
            &query,
            online_state_matches,
        );
        update_search_results_accessible_summary(
            &self.search_content,
            config.language,
            raw_query,
            track_matches.len() + album_matches.len() + artist_matches.len() + playlist_matches.len(),
            track_matches.len(),
            album_matches.len(),
            artist_matches.len(),
            playlist_matches.len(),
            loading,
        );

        self.search_content.append(&search_list_section(
            copy.albums,
            copy.no_albums,
            album_matches,
            self.search_album_limit.clone(),
            &self.event_tx,
            loading,
            copy,
            config,
            playback,
            YouTubeSearchCategory::Albums,
            if online_state_matches {
                youtube.search.continuation(YouTubeSearchCategory::Albums)
            } else {
                ""
            },
            online_state_matches && youtube.search.loading_more(YouTubeSearchCategory::Albums),
        ));
        self.search_content.append(&search_list_section(
            copy.artists,
            copy.no_artists,
            artist_matches,
            self.search_artist_limit.clone(),
            &self.event_tx,
            loading,
            copy,
            config,
            playback,
            YouTubeSearchCategory::Artists,
            if online_state_matches {
                youtube.search.continuation(YouTubeSearchCategory::Artists)
            } else {
                ""
            },
            online_state_matches && youtube.search.loading_more(YouTubeSearchCategory::Artists),
        ));
        self.search_content.append(&search_list_section(
            copy.playlists,
            copy.no_playlists,
            playlist_matches,
            self.search_playlist_limit.clone(),
            &self.event_tx,
            loading,
            copy,
            config,
            playback,
            YouTubeSearchCategory::Playlists,
            if online_state_matches {
                youtube
                    .search
                    .continuation(YouTubeSearchCategory::Playlists)
            } else {
                ""
            },
            online_state_matches
                && youtube
                    .search
                    .loading_more(YouTubeSearchCategory::Playlists),
        ));
'''
    text = replace_once(
        text,
        old,
        new,
        "Update search result accessible summary before rendering categorized sections",
    )

    if "mod search_accessibility_tests" not in text:
        text += ANNOUNCEMENT_TESTS
        print("[changed] Add search accessibility announcement tests")
    else:
        print("[already applied] Add search accessibility announcement tests")
    return text


def patch_roadmap(text: str) -> str:
    active_candidates = [
        "- 🟡 Search result update announcements and accessibility polish.\n",
        "- 🟡 Accessibility announcements when results update.\n",
        "- 🟡 Search history and recent queries.\n",
    ]
    active_matches = [candidate for candidate in active_candidates if candidate in text]
    if active_matches:
        text = text.replace(
            active_matches[0],
            "- 🟡 Final search release polish and keyboard/a11y audit.\n",
            1,
        )
        print("[changed] Advance active search accessibility checkpoint")
    else:
        print("[already applied] Advance active search accessibility checkpoint")

    anchors = [
        "- ✅ Route-aware cancellation for stale YouTube Music search responses.\n",
        "- ✅ Local recent-query history with MRU ordering, individual removal and clear-all controls.\n",
        "- ✅ Collection-result rows support arrow navigation and Enter/Space activation.\n",
    ]
    anchor = next((candidate for candidate in anchors if candidate in text), None)
    if anchor is None:
        raise PatchError("Document completed accessibility announcements: anchor not found")
    completed = "- ✅ Accessible search-result summaries update after each categorized result rebuild.\n"
    if completed not in text:
        text = text.replace(anchor, anchor + completed, 1)
        print("[changed] Document completed search accessibility announcements")
    else:
        print("[already applied] Document completed search accessibility announcements")

    remaining = "- Accessibility announcements when results update.\n"
    if remaining in text:
        text = text.replace(remaining, "", 1)
        print("[changed] Remove completed accessibility announcement item")

    order_candidates = [
        "8. Add search result announcements and release polish.\n",
        "8. Add route-aware remote search cancellation.\n",
        "8. Improve mixed-source ranking and route-aware cancellation.\n",
    ]
    order_matches = [candidate for candidate in order_candidates if candidate in text]
    if order_matches:
        text = text.replace(
            order_matches[0],
            "8. Complete final search release polish and accessibility audit.\n",
            1,
        )
        print("[changed] Advance recommended search order")
    else:
        print("[already applied] Advance recommended search order")
    return text


def main() -> int:
    required = [BROWSER, ROADMAP]
    missing = [path for path in required if not path.is_file()]
    if missing:
        print("Run this script from the Nocky repository root.", file=sys.stderr)
        for path in missing:
            print(f"missing: {path}", file=sys.stderr)
        return 1

    original = {path: path.read_text(encoding="utf-8") for path in required}
    if "LoadMoreSearch(YouTubeSearchCategory)" not in original[BROWSER]:
        print("ERROR: apply and validate the categorized remote search pagination checkpoint first.", file=sys.stderr)
        return 1
    if DOC.exists() and DOC.read_text(encoding="utf-8") != DOC_SOURCE:
        print(f"ERROR: {DOC} already exists with different content. No files were written.", file=sys.stderr)
        return 1

    updated = dict(original)
    try:
        updated[BROWSER] = patch_browser(updated[BROWSER])
        updated[ROADMAP] = patch_roadmap(updated[ROADMAP])
    except PatchError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        print("No files were written.", file=sys.stderr)
        return 1

    changed: list[Path] = []
    for path in required:
        if updated[path] != original[path]:
            path.write_text(updated[path], encoding="utf-8")
            changed.append(path.relative_to(ROOT))

    if not DOC.exists():
        DOC.write_text(DOC_SOURCE, encoding="utf-8")
        changed.append(DOC.relative_to(ROOT))

    print("Search result accessibility announcements patch applied successfully.")
    for path in changed:
        print(f"  {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

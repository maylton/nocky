#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path

ROOT = Path.cwd()
BROWSER = ROOT / "src/browser.rs"

if not BROWSER.is_file():
    raise SystemExit("Run this script from the Nocky repository root.")

text = BROWSER.read_text(encoding="utf-8")

old_helpers = '''fn search_results_announcement(
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
    widget: &gtk::Box,
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

new_helpers = '''#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct SearchResultCounts {
    tracks: usize,
    albums: usize,
    artists: usize,
    playlists: usize,
}

impl SearchResultCounts {
    fn total(self) -> usize {
        self.tracks + self.albums + self.artists + self.playlists
    }
}

fn search_results_announcement(
    language: AppLanguage,
    query: &str,
    counts: SearchResultCounts,
    loading: bool,
) -> String {
    let query = query.trim();
    let total = counts.total();
    let tracks = counts.tracks;
    let albums = counts.albums;
    let artists = counts.artists;
    let playlists = counts.playlists;
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
    widget: &gtk::Box,
    language: AppLanguage,
    query: &str,
    counts: SearchResultCounts,
    loading: bool,
) {
    let message = search_results_announcement(language, query, counts, loading);
    widget.update_property(&[gtk::accessible::Property::Label(&message)]);
}

'''

if old_helpers in text:
    text = text.replace(old_helpers, new_helpers, 1)
elif "struct SearchResultCounts" in text:
    print("Search announcement helper signatures are already below the clippy argument limit.")
else:
    raise SystemExit("Could not find the search announcement helper block. No files were written.")

old_call = '''        update_search_results_accessible_summary(
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
'''
new_call = '''        let result_counts = SearchResultCounts {
            tracks: track_matches.len(),
            albums: album_matches.len(),
            artists: artist_matches.len(),
            playlists: playlist_matches.len(),
        };
        update_search_results_accessible_summary(
            &self.search_content,
            config.language,
            raw_query,
            result_counts,
            loading,
        );
'''

if old_call in text:
    text = text.replace(old_call, new_call, 1)
elif new_call in text:
    pass
else:
    raise SystemExit("Could not find the search accessible summary call. No files were written.")

replacements = {
'''        let message = search_results_announcement(
            AppLanguage::Portuguese,
            "Muse",
            10,
            4,
            2,
            3,
            1,
            true,
        );
''': '''        let message = search_results_announcement(
            AppLanguage::Portuguese,
            "Muse",
            SearchResultCounts {
                tracks: 4,
                albums: 2,
                artists: 3,
                playlists: 1,
            },
            true,
        );
''',
'''        let message = search_results_announcement(
            AppLanguage::English,
            "Radiohead",
            3,
            1,
            1,
            1,
            0,
            false,
        );
''': '''        let message = search_results_announcement(
            AppLanguage::English,
            "Radiohead",
            SearchResultCounts {
                tracks: 1,
                albums: 1,
                artists: 1,
                playlists: 0,
            },
            false,
        );
''',
'''        let message = search_results_announcement(
            AppLanguage::Spanish,
            "Soda Stereo",
            7,
            2,
            2,
            2,
            1,
            false,
        );
''': '''        let message = search_results_announcement(
            AppLanguage::Spanish,
            "Soda Stereo",
            SearchResultCounts {
                tracks: 2,
                albums: 2,
                artists: 2,
                playlists: 1,
            },
            false,
        );
''',
}

for old, new in replacements.items():
    if old in text:
        text = text.replace(old, new, 1)
    elif new in text:
        pass
    else:
        raise SystemExit("Could not update one search announcement test call. No files were written.")

BROWSER.write_text(text, encoding="utf-8")
print("Search announcement helpers now satisfy clippy's argument limit.")

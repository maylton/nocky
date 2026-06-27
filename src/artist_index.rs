use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use crate::{model::Track, youtube::credited_artists};

#[derive(Clone, Debug, Default)]
pub struct LocalArtistIndex {
    first_solo_covers: BTreeMap<String, Option<PathBuf>>,
}

impl LocalArtistIndex {
    pub fn build(tracks: &[Track]) -> Self {
        #[derive(Default)]
        struct CoverCandidate {
            first_solo_track_seen: bool,
            first_solo_cover: Option<PathBuf>,
        }

        let mut candidates = BTreeMap::<String, CoverCandidate>::new();

        for track in tracks {
            let credits = credited_artists(&track.artist);

            for artist in &credits {
                let normalized = normalize_artist_name(artist);
                if normalized.is_empty() {
                    continue;
                }

                let candidate = candidates.entry(normalized).or_default();

                // Collaboration tracks establish artist identity, but card
                // artwork comes from the first solo track only.
                if credits.len() == 1 && !candidate.first_solo_track_seen {
                    candidate.first_solo_track_seen = true;
                    candidate.first_solo_cover = track.cover_path.clone();
                }
            }
        }

        let first_solo_covers = candidates
            .into_iter()
            .map(|(artist, candidate)| (artist, candidate.first_solo_cover))
            .collect();

        Self { first_solo_covers }
    }

    pub fn first_solo_cover(&self, artist: &str) -> Option<&Path> {
        self.first_solo_covers
            .get(&normalize_artist_name(artist))
            .and_then(|cover| cover.as_deref())
    }
}

fn normalize_artist_name(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gtk::gio;

    fn track(title: &str, artist: &str, cover: Option<&str>) -> Track {
        let path = PathBuf::from(format!("/tmp/{title}.mp3"));
        Track {
            file: gio::File::for_path(&path),
            path,
            title: title.to_string(),
            artist: artist.to_string(),
            album: "Album".to_string(),
            duration_seconds: 180,
            disc_number: None,
            track_number: None,
            cover_path: cover.map(PathBuf::from),
            lyrics: Vec::new(),
        }
    }

    #[test]
    fn collaboration_tracks_do_not_become_ranked_solo_artwork() {
        let tracks = vec![track(
            "Collab",
            "Artist A feat. Artist B",
            Some("/tmp/shared.jpg"),
        )];

        let index = LocalArtistIndex::build(&tracks);
        assert!(index.first_solo_covers.contains_key("artist a"));
        assert!(index.first_solo_covers.contains_key("artist b"));
        assert_eq!(index.first_solo_cover("Artist A"), None);
        assert_eq!(index.first_solo_cover("Artist B"), None);
    }

    #[test]
    fn keeps_bare_ampersand_band_names_intact() {
        let tracks = vec![track("Song", "Simon & Garfunkel", Some("/tmp/duo.jpg"))];

        let index = LocalArtistIndex::build(&tracks);
        assert_eq!(index.first_solo_covers.len(), 1);
        assert_eq!(
            index.first_solo_cover("Simon & Garfunkel"),
            Some(Path::new("/tmp/duo.jpg"))
        );
    }

    #[test]
    fn first_solo_track_without_cover_blocks_later_solo_artwork() {
        let tracks = vec![
            track("First", "Artist A", None),
            track("Second", "Artist A", Some("/tmp/second.jpg")),
        ];

        let index = LocalArtistIndex::build(&tracks);
        assert_eq!(index.first_solo_cover("Artist A"), None);
    }

    #[test]
    fn normalizes_case_and_repeated_whitespace() {
        let tracks = vec![track("Song", "  Artist   A  ", Some("/tmp/artist.jpg"))];

        let index = LocalArtistIndex::build(&tracks);
        assert_eq!(
            index.first_solo_cover("artist a"),
            Some(Path::new("/tmp/artist.jpg"))
        );
        assert_eq!(
            index.first_solo_cover("ARTIST   A"),
            Some(Path::new("/tmp/artist.jpg"))
        );
    }
}

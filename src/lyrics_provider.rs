use reqwest::blocking::Client;
use serde::Deserialize;
use std::{fs, path::Path, time::Duration};

#[derive(Clone, Debug)]
pub struct LyricsLookup {
    pub title: String,
    pub artist: String,
    pub album: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LyricsRecord {
    track_name: String,
    artist_name: String,
    album_name: String,
    synced_lyrics: Option<String>,
}

pub fn download_to_sidecar(audio_path: &Path, lookup: &LyricsLookup) -> Result<(), String> {
    let lyrics = fetch_synced_lyrics(lookup)?;
    let sidecar = audio_path.with_extension("lrc");
    fs::write(&sidecar, lyrics)
        .map_err(|error| format!("Could not save {}: {error}", sidecar.display()))
}

fn fetch_synced_lyrics(lookup: &LyricsLookup) -> Result<String, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(concat!("Nocky/", env!("CARGO_PKG_VERSION"), " (Linux music player)"))
        .build()
        .map_err(|error| format!("Could not create the lyrics client: {error}"))?;

    let mut request = client
        .get("https://lrclib.net/api/search")
        .query(&[("track_name", lookup.title.as_str())]);

    if !is_unknown(&lookup.artist) {
        request = request.query(&[("artist_name", lookup.artist.as_str())]);
    }
    if !is_unknown(&lookup.album) {
        request = request.query(&[("album_name", lookup.album.as_str())]);
    }

    let response = request
        .send()
        .map_err(|error| format!("Lyrics request failed: {error}"))?;

    if !response.status().is_success() {
        return Err(format!("Lyrics service returned HTTP {}", response.status()));
    }

    let records = response
        .json::<Vec<LyricsRecord>>()
        .map_err(|error| format!("Invalid response from lyrics service: {error}"))?;

    records
        .into_iter()
        .filter_map(|record| {
            let score = match_score(lookup, &record);
            let lyrics = record.synced_lyrics?.trim().to_string();
            if lyrics.is_empty() {
                return None;
            }
            Some((score, lyrics))
        })
        .max_by_key(|(score, _)| *score)
        .map(|(_, lyrics)| lyrics)
        .ok_or_else(|| "No synchronized lyrics were found".to_string())
}

fn match_score(lookup: &LyricsLookup, record: &LyricsRecord) -> u8 {
    let mut score = 0;
    if normalize(&lookup.title) == normalize(&record.track_name) {
        score += 5;
    }
    if normalize(&lookup.artist) == normalize(&record.artist_name) {
        score += 3;
    }
    if normalize(&lookup.album) == normalize(&record.album_name) {
        score += 1;
    }
    score
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn is_unknown(value: &str) -> bool {
    value.trim().is_empty() || value.starts_with("Unknown") || value == "Local music"
}

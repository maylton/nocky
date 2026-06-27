use crate::lyrics::plain_to_lrc;
use gtk::glib;
use reqwest::blocking::{Client, Response};
use serde::Deserialize;
use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Clone, Debug)]
pub struct LyricsLookup {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_seconds: u64,
}

#[derive(Clone, Debug)]
pub struct LyricsDocument {
    pub contents: String,
    pub synchronized: bool,
    pub provider: &'static str,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LyricsRecord {
    track_name: String,
    artist_name: String,
    album_name: String,
    duration: Option<f64>,
    synced_lyrics: Option<String>,
    plain_lyrics: Option<String>,
}

pub fn download_to_sidecar(
    audio_path: &Path,
    lookup: &LyricsLookup,
    force: bool,
) -> Result<LyricsDocument, String> {
    let document = fetch_lyrics(lookup, force)?;
    let sidecar = audio_path.with_extension("lrc");
    fs::write(&sidecar, &document.contents)
        .map_err(|error| format!("Could not save {}: {error}", sidecar.display()))?;
    Ok(document)
}

pub fn fetch_lyrics(lookup: &LyricsLookup, force: bool) -> Result<LyricsDocument, String> {
    if !force {
        if let Some(contents) = read_cache(lookup) {
            return Ok(LyricsDocument {
                contents,
                synchronized: true,
                provider: "cache",
            });
        }
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(concat!(
            "Nocky/",
            env!("CARGO_PKG_VERSION"),
            " (Linux music player)"
        ))
        .build()
        .map_err(|error| format!("Could not create the lyrics client: {error}"))?;

    let mut candidates = Vec::new();

    if let Some(record) = fetch_exact(&client, lookup)? {
        candidates.push(record);
    }

    candidates.extend(fetch_search(&client, lookup, true)?);

    if candidates.is_empty() {
        candidates.extend(fetch_search(&client, lookup, false)?);
    }

    let record = candidates
        .into_iter()
        .max_by_key(|record| match_score(lookup, record))
        .ok_or_else(|| "No lyrics were found".to_string())?;

    let document = record_to_document(lookup, record)?;
    write_cache(lookup, &document.contents);
    Ok(document)
}

fn fetch_exact(client: &Client, lookup: &LyricsLookup) -> Result<Option<LyricsRecord>, String> {
    if is_unknown(&lookup.artist) {
        return Ok(None);
    }

    let mut request = client.get("https://lrclib.net/api/get").query(&[
        ("track_name", lookup.title.as_str()),
        ("artist_name", lookup.artist.as_str()),
    ]);

    if !is_unknown(&lookup.album) {
        request = request.query(&[("album_name", lookup.album.as_str())]);
    }

    if lookup.duration_seconds > 0 {
        request = request.query(&[("duration", lookup.duration_seconds.to_string())]);
    }

    let response = request
        .send()
        .map_err(|error| format!("Exact lyrics request failed: {error}"))?;

    if response.status().as_u16() == 404 {
        return Ok(None);
    }

    parse_single_response(response).map(Some)
}

fn fetch_search(
    client: &Client,
    lookup: &LyricsLookup,
    strict: bool,
) -> Result<Vec<LyricsRecord>, String> {
    let mut request = client
        .get("https://lrclib.net/api/search")
        .query(&[("track_name", lookup.title.as_str())]);

    if strict && !is_unknown(&lookup.artist) {
        request = request.query(&[("artist_name", lookup.artist.as_str())]);
    }
    if strict && !is_unknown(&lookup.album) {
        request = request.query(&[("album_name", lookup.album.as_str())]);
    }

    let response = request
        .send()
        .map_err(|error| format!("Lyrics search failed: {error}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "Lyrics service returned HTTP {}",
            response.status()
        ));
    }

    response
        .json::<Vec<LyricsRecord>>()
        .map_err(|error| format!("Invalid response from lyrics service: {error}"))
}

fn parse_single_response(response: Response) -> Result<LyricsRecord, String> {
    if !response.status().is_success() {
        return Err(format!(
            "Lyrics service returned HTTP {}",
            response.status()
        ));
    }

    response
        .json::<LyricsRecord>()
        .map_err(|error| format!("Invalid exact lyrics response: {error}"))
}

fn record_to_document(
    lookup: &LyricsLookup,
    record: LyricsRecord,
) -> Result<LyricsDocument, String> {
    if let Some(contents) = record
        .synced_lyrics
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        return Ok(LyricsDocument {
            contents,
            synchronized: true,
            provider: "LRCLIB synced",
        });
    }

    let plain = record
        .plain_lyrics
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "The selected lyrics result was empty".to_string())?;

    let contents = plain_to_lrc(&plain, lookup.duration_seconds);
    if contents.is_empty() {
        return Err("The plain lyrics could not be converted".to_string());
    }

    Ok(LyricsDocument {
        contents,
        synchronized: false,
        provider: "LRCLIB plain",
    })
}

fn match_score(lookup: &LyricsLookup, record: &LyricsRecord) -> i32 {
    let mut score = 0_i32;

    if normalize(&lookup.title) == normalize(&record.track_name) {
        score += 100;
    } else if normalize(&record.track_name).contains(&normalize(&lookup.title)) {
        score += 30;
    }

    if normalize(&lookup.artist) == normalize(&record.artist_name) {
        score += 60;
    }

    if normalize(&lookup.album) == normalize(&record.album_name) {
        score += 15;
    }

    if record
        .synced_lyrics
        .as_ref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        score += 40;
    } else if record
        .plain_lyrics
        .as_ref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        score += 10;
    }

    if lookup.duration_seconds > 0 {
        if let Some(duration) = record.duration {
            let difference = (duration.round() as i64 - lookup.duration_seconds as i64).abs();
            score += (20_i64.saturating_sub(difference)).max(0) as i32;
        }
    }

    score
}

fn cache_path(lookup: &LyricsLookup) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    normalize(&lookup.title).hash(&mut hasher);
    normalize(&lookup.artist).hash(&mut hasher);
    normalize(&lookup.album).hash(&mut hasher);
    lookup.duration_seconds.hash(&mut hasher);

    glib::user_cache_dir()
        .join("nocky")
        .join("lyrics")
        .join(format!("{:016x}.lrc", hasher.finish()))
}

fn read_cache(lookup: &LyricsLookup) -> Option<String> {
    let path = cache_path(lookup);
    let contents = fs::read_to_string(path).ok()?;
    (!contents.trim().is_empty()).then_some(contents)
}

fn write_cache(lookup: &LyricsLookup, contents: &str) {
    let path = cache_path(lookup);
    let Some(parent) = path.parent() else {
        return;
    };

    if fs::create_dir_all(parent).is_ok() {
        if let Err(error) = fs::write(&path, contents) {
            eprintln!("Could not cache lyrics at {}: {error}", path.display());
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_metadata_scores_above_partial_metadata() {
        let lookup = LyricsLookup {
            title: "Example Song".to_string(),
            artist: "Example Artist".to_string(),
            album: "Example Album".to_string(),
            duration_seconds: 180,
        };

        let exact = LyricsRecord {
            track_name: lookup.title.clone(),
            artist_name: lookup.artist.clone(),
            album_name: lookup.album.clone(),
            duration: Some(180.0),
            synced_lyrics: Some("[00:00]Hello".to_string()),
            plain_lyrics: None,
        };

        let partial = LyricsRecord {
            track_name: lookup.title.clone(),
            artist_name: "Other Artist".to_string(),
            album_name: String::new(),
            duration: Some(240.0),
            synced_lyrics: None,
            plain_lyrics: Some("Hello".to_string()),
        };

        assert!(match_score(&lookup, &exact) > match_score(&lookup, &partial));
    }
}

use gtk::gio;
use lofty::{
    file::{AudioFile, TaggedFileExt},
    tag::Accessor,
};
use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use crate::lyrics::{load_sidecar, LyricLine};

#[derive(Clone, Debug)]
pub struct TrackData {
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_seconds: u64,
    pub disc_number: Option<u32>,
    pub track_number: Option<u32>,
    pub cover_path: Option<PathBuf>,
    pub lyrics: Vec<LyricLine>,
}

impl TrackData {
    pub fn from_path(path: PathBuf) -> Option<Self> {
        let stem = path.file_stem()?.to_string_lossy().trim().to_string();
        let (fallback_artist, fallback_title) = parse_filename(&stem);
        let fallback_album = path
            .parent()
            .and_then(Path::file_name)
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "Local music".to_string());

        let mut title = fallback_title;
        let mut artist = fallback_artist;
        let mut album = fallback_album;
        let mut duration_seconds = 0;
        let mut disc_number = None;
        let mut track_number = None;
        let mut cover_path = find_cover_path(&path);

        if let Ok(tagged_file) = lofty::read_from_path(&path) {
            duration_seconds = tagged_file.properties().duration().as_secs();
            if let Some(tag) = tagged_file
                .primary_tag()
                .or_else(|| tagged_file.first_tag())
            {
                title = tag
                    .title()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty())
                    .unwrap_or(title);
                artist = tag
                    .artist()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty())
                    .unwrap_or(artist);
                album = tag
                    .album()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty())
                    .unwrap_or(album);
                disc_number = tag.disk();
                track_number = tag.track();
                if cover_path.is_none() {
                    cover_path = extract_embedded_cover(&path, tag);
                }
            }
        }

        let lyrics = load_sidecar(&path);
        Some(Self {
            path,
            title,
            artist,
            album,
            duration_seconds,
            disc_number,
            track_number,
            cover_path,
            lyrics,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Track {
    pub file: gio::File,
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_seconds: u64,
    pub disc_number: Option<u32>,
    pub track_number: Option<u32>,
    pub cover_path: Option<PathBuf>,
    pub lyrics: Vec<LyricLine>,
}

impl From<TrackData> for Track {
    fn from(data: TrackData) -> Self {
        let file = gio::File::for_path(&data.path);
        Self {
            file,
            path: data.path,
            title: data.title,
            artist: data.artist,
            album: data.album,
            duration_seconds: data.duration_seconds,
            disc_number: data.disc_number,
            track_number: data.track_number,
            cover_path: data.cover_path,
            lyrics: data.lyrics,
        }
    }
}

impl Track {
    pub fn reload_lyrics(&mut self) {
        self.lyrics = load_sidecar(&self.path);
    }
}

fn parse_filename(stem: &str) -> (String, String) {
    match stem.split_once(" - ") {
        Some((artist, title)) => (artist.trim().to_string(), title.trim().to_string()),
        None => ("Unknown artist".to_string(), stem.to_string()),
    }
}

fn find_cover_path(audio_path: &Path) -> Option<PathBuf> {
    let parent = audio_path.parent()?;
    let stem = audio_path.file_stem()?.to_string_lossy();

    let candidates = [
        format!("{stem}.jpg"),
        format!("{stem}.jpeg"),
        format!("{stem}.png"),
        format!("{stem}.webp"),
        "cover.jpg".to_string(),
        "cover.jpeg".to_string(),
        "cover.png".to_string(),
        "cover.webp".to_string(),
        "folder.jpg".to_string(),
        "folder.jpeg".to_string(),
        "folder.png".to_string(),
        "folder.webp".to_string(),
        "front.jpg".to_string(),
        "front.png".to_string(),
    ];

    candidates
        .into_iter()
        .map(|name| parent.join(name))
        .find(|candidate| candidate.is_file())
}

fn extract_embedded_cover(audio_path: &Path, tag: &lofty::tag::Tag) -> Option<PathBuf> {
    let picture = tag.pictures().first()?;
    let data = picture.data();
    if data.is_empty() {
        return None;
    }

    let cache_dir = cover_cache_dir();
    fs::create_dir_all(&cache_dir).ok()?;

    let mut hasher = DefaultHasher::new();
    audio_path.hash(&mut hasher);
    if let Ok(metadata) = fs::metadata(audio_path) {
        if let Ok(modified) = metadata.modified() {
            if let Ok(since_epoch) = modified.duration_since(UNIX_EPOCH) {
                since_epoch.as_nanos().hash(&mut hasher);
            }
        }
    }

    let cover_path = cache_dir.join(format!("{:016x}.cover", hasher.finish()));
    if !cover_path.is_file() || fs::metadata(&cover_path).ok()?.len() == 0 {
        fs::write(&cover_path, data).ok()?;
    }
    Some(cover_path)
}

fn cover_cache_dir() -> PathBuf {
    if let Some(xdg_cache) = std::env::var_os("XDG_CACHE_HOME") {
        return PathBuf::from(xdg_cache).join("nocky").join("covers");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home)
            .join(".cache")
            .join("nocky")
            .join("covers");
    }
    std::env::temp_dir().join("nocky-covers")
}

use crate::model::TrackData;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const AUDIO_EXTENSIONS: &[&str] = &["mp3", "flac", "ogg", "opus", "m4a", "mp4", "wav", "aac"];

pub fn scan_music_directory(root: &Path) -> Result<Vec<TrackData>, String> {
    if !root.is_dir() {
        return Err(format!("{} is not a readable directory", root.display()));
    }

    let mut paths = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| is_supported_audio(path))
        .collect::<Vec<PathBuf>>();

    paths.sort_by(|left, right| {
        left.to_string_lossy()
            .to_lowercase()
            .cmp(&right.to_string_lossy().to_lowercase())
    });
    paths.dedup();

    Ok(paths.into_iter().filter_map(TrackData::from_path).collect())
}

pub fn is_supported_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            AUDIO_EXTENSIONS
                .iter()
                .any(|supported| extension.eq_ignore_ascii_case(supported))
        })
}

//! Persistent metadata foundation for YouTube Music offline collections.
//!
//! Audio downloads are added by the download-manager phase. This store already
//! owns the stable paths, manifest format, integrity checks and cleanup rules.

#![allow(dead_code)]

use crate::youtube::{YouTubeItem, YouTubeStream};
use gtk::glib;
use reqwest::blocking::Client;
use reqwest::header::{
    HeaderName, HeaderValue, ACCEPT_ENCODING, CONTENT_LENGTH, CONTENT_RANGE, RANGE, USER_AGENT,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const MANIFEST_VERSION: u32 = 1;
const MIN_VALID_AUDIO_BYTES: u64 = 4 * 1024;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct OfflineTrack {
    pub video_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub relative_path: String,
    pub size_bytes: u64,
    pub downloaded_at: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct OfflineCollection {
    pub collection_id: String,
    pub item: YouTubeItem,
    pub playlist: bool,
    pub followed_at: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct OfflineManifest {
    version: u32,
    tracks: HashMap<String, OfflineTrack>,
    collections: HashMap<String, OfflineCollection>,
}

#[derive(Debug)]
pub struct OfflineStore {
    root: PathBuf,
    manifest_path: PathBuf,
    manifest: OfflineManifest,
}

impl OfflineStore {
    pub fn load_default() -> Self {
        let root = glib::user_data_dir().join("nocky").join("offline");
        Self::load(root)
    }

    pub fn load(root: PathBuf) -> Self {
        let manifest_path = root.join("manifest.json");
        let mut manifest = fs::read(&manifest_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<OfflineManifest>(&bytes).ok())
            .filter(|manifest| manifest.version == MANIFEST_VERSION)
            .unwrap_or_else(|| OfflineManifest {
                version: MANIFEST_VERSION,
                tracks: HashMap::new(),
                collections: HashMap::new(),
            });

        manifest.tracks.retain(|video_id, track| {
            !video_id.trim().is_empty()
                && video_id == &track.video_id
                && valid_audio_file(&root.join(&track.relative_path))
        });

        Self {
            root,
            manifest_path,
            manifest,
        }
    }

    pub fn root_dir(&self) -> PathBuf {
        self.root.clone()
    }

    pub fn audio_dir(&self) -> PathBuf {
        self.root.join("audio")
    }

    pub fn partial_dir(&self) -> PathBuf {
        self.root.join("partial")
    }

    pub fn final_path(&self, video_id: &str, extension: &str) -> PathBuf {
        let extension = sanitize_extension(extension);
        self.audio_dir()
            .join(format!("{}.{}", sanitize_video_id(video_id), extension))
    }

    pub fn partial_path(&self, video_id: &str) -> PathBuf {
        self.partial_dir()
            .join(format!("{}.part", sanitize_video_id(video_id)))
    }

    pub fn resolve(&self, video_id: &str) -> Option<PathBuf> {
        let track = self.manifest.tracks.get(video_id)?;
        let path = self.root.join(&track.relative_path);
        valid_audio_file(&path).then_some(path)
    }

    pub fn contains(&self, video_id: &str) -> bool {
        self.resolve(video_id).is_some()
    }

    pub fn video_ids(&self) -> HashSet<String> {
        self.manifest.tracks.keys().cloned().collect()
    }

    pub fn follow_collection(
        &mut self,
        collection_id: &str,
        item: &YouTubeItem,
        playlist: bool,
    ) -> Result<(), String> {
        if collection_id.trim().is_empty() {
            return Err("A coleção offline não possui uma identidade estável".to_string());
        }

        self.manifest.collections.insert(
            collection_id.to_string(),
            OfflineCollection {
                collection_id: collection_id.to_string(),
                item: item.clone(),
                playlist,
                followed_at: unix_timestamp(),
            },
        );
        self.save()
    }

    pub fn followed_collections(&self) -> Vec<OfflineCollection> {
        self.manifest.collections.values().cloned().collect()
    }

    pub fn register(
        &mut self,
        video_id: &str,
        title: &str,
        artist: &str,
        album: &str,
        file_path: &Path,
    ) -> Result<(), String> {
        if video_id.trim().is_empty() {
            return Err("A faixa offline não possui video_id".to_string());
        }
        if !valid_audio_file(file_path) {
            return Err("O arquivo de áudio offline está vazio ou incompleto".to_string());
        }

        let relative_path = file_path
            .strip_prefix(&self.root)
            .map_err(|_| "O arquivo offline está fora do diretório do Nocky".to_string())?
            .to_string_lossy()
            .into_owned();
        let size_bytes = fs::metadata(file_path)
            .map_err(|error| format!("Não foi possível verificar o áudio offline: {error}"))?
            .len();

        self.manifest.tracks.insert(
            video_id.to_string(),
            OfflineTrack {
                video_id: video_id.to_string(),
                title: title.to_string(),
                artist: artist.to_string(),
                album: album.to_string(),
                relative_path,
                size_bytes,
                downloaded_at: unix_timestamp(),
            },
        );
        self.save()
    }

    pub fn remove(&mut self, video_id: &str) -> Result<bool, String> {
        let Some(track) = self.manifest.tracks.remove(video_id) else {
            return Ok(false);
        };

        let path = self.root.join(track.relative_path);
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "Não foi possível remover o arquivo offline '{}': {error}",
                    path.display()
                ));
            }
        }

        self.save()?;
        Ok(true)
    }

    pub fn track_count(&self) -> usize {
        self.manifest.tracks.len()
    }

    pub fn total_size_bytes(&self) -> u64 {
        self.manifest
            .tracks
            .values()
            .map(|track| track.size_bytes)
            .sum()
    }

    pub fn partial_stats(&self) -> (usize, u64) {
        directory_file_stats(&self.partial_dir())
    }

    pub fn clear_partials(&self) -> Result<usize, String> {
        let (count, _) = self.partial_stats();
        let partial_dir = self.partial_dir();

        match fs::remove_dir_all(&partial_dir) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "Não foi possível limpar os downloads incompletos: {error}"
                ));
            }
        }

        fs::create_dir_all(&partial_dir).map_err(|error| {
            format!("Não foi possível recriar a pasta temporária offline: {error}")
        })?;
        Ok(count)
    }

    pub fn clear_all(&mut self) -> Result<(usize, u64), String> {
        let removed_tracks = self.track_count();
        let removed_bytes = self.total_size_bytes();

        for directory in [self.audio_dir(), self.partial_dir()] {
            match fs::remove_dir_all(&directory) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(format!(
                        "Não foi possível remover '{}': {error}",
                        directory.display()
                    ));
                }
            }
        }

        self.manifest.tracks.clear();
        self.manifest.collections.clear();
        self.save()?;

        fs::create_dir_all(self.audio_dir())
            .map_err(|error| format!("Não foi possível recriar a pasta de áudio: {error}"))?;
        fs::create_dir_all(self.partial_dir())
            .map_err(|error| format!("Não foi possível recriar a pasta temporária: {error}"))?;

        Ok((removed_tracks, removed_bytes))
    }

    pub fn save(&self) -> Result<(), String> {
        fs::create_dir_all(&self.root)
            .map_err(|error| format!("Não foi possível criar o diretório offline: {error}"))?;

        let temporary = self.manifest_path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(&self.manifest)
            .map_err(|error| format!("Não foi possível serializar o manifesto offline: {error}"))?;
        fs::write(&temporary, bytes)
            .map_err(|error| format!("Não foi possível salvar o manifesto offline: {error}"))?;
        fs::rename(&temporary, &self.manifest_path)
            .map_err(|error| format!("Não foi possível finalizar o manifesto offline: {error}"))
    }
}

pub fn download_youtube_track(
    item: &YouTubeItem,
    stream: &YouTubeStream,
) -> Result<PathBuf, String> {
    const MAX_CHUNK_ATTEMPTS: usize = 3;
    const RANGE_CHUNK_BYTES: u64 = 4 * 1024 * 1024;
    const REQUEST_TIMEOUT_SECS: u64 = 45;

    let store = OfflineStore::load_default();
    fs::create_dir_all(store.audio_dir())
        .map_err(|error| format!("Não foi possível criar o diretório de áudio offline: {error}"))?;
    fs::create_dir_all(store.partial_dir()).map_err(|error| {
        format!("Não foi possível criar o diretório temporário offline: {error}")
    })?;

    let partial = store.partial_path(&item.video_id);
    let client = Client::builder()
        .connect_timeout(std::time::Duration::from_secs(12))
        .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .build()
        .map_err(|error| format!("Não foi possível preparar o download offline: {error}"))?;

    let mut downloaded = fs::metadata(&partial)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let mut total_size = None;
    let mut extension = "webm";

    loop {
        if total_size.is_some_and(|total| downloaded >= total) {
            break;
        }

        let range_end = downloaded
            .saturating_add(RANGE_CHUNK_BYTES)
            .saturating_sub(1);
        let mut chunk_complete = false;
        let mut last_error = None;

        for attempt in 1..=MAX_CHUNK_ATTEMPTS {
            let mut request = client
                .get(&stream.stream_url)
                .header(ACCEPT_ENCODING, "identity")
                .header(USER_AGENT, "Mozilla/5.0 Nocky/0.4.0")
                .header(RANGE, format!("bytes={downloaded}-{range_end}"));

            for (name, value) in &stream.http_headers {
                let Ok(name) = HeaderName::from_bytes(name.as_bytes()) else {
                    continue;
                };
                let Ok(value) = HeaderValue::from_str(value) else {
                    continue;
                };
                request = request.header(name, value);
            }

            let mut response = match request.send() {
                Ok(response) => response,
                Err(error) => {
                    last_error = Some(format!("falha ao iniciar o bloco: {error}"));
                    if attempt < MAX_CHUNK_ATTEMPTS {
                        std::thread::sleep(std::time::Duration::from_millis(250 * attempt as u64));
                        continue;
                    }
                    break;
                }
            };

            if response.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE
                && valid_audio_file(&partial)
            {
                total_size = Some(downloaded);
                chunk_complete = true;
                break;
            }

            if !response.status().is_success() {
                last_error = Some(format!("o servidor respondeu com {}", response.status()));
                if attempt < MAX_CHUNK_ATTEMPTS {
                    std::thread::sleep(std::time::Duration::from_millis(250 * attempt as u64));
                    continue;
                }
                break;
            }

            let partial_content = response.status() == reqwest::StatusCode::PARTIAL_CONTENT;

            if downloaded > 0 && !partial_content {
                fs::remove_file(&partial).map_err(|error| {
                    format!(
                        "O servidor não permitiu retomar '{}' e o arquivo parcial não pôde ser reiniciado: {error}",
                        item.title
                    )
                })?;
                downloaded = 0;
            }

            extension = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .map(|value| {
                    if value.contains("mp4") || value.contains("m4a") {
                        "m4a"
                    } else {
                        "webm"
                    }
                })
                .unwrap_or("webm");

            total_size = response
                .headers()
                .get(CONTENT_RANGE)
                .and_then(|value| value.to_str().ok())
                .and_then(content_range_total)
                .or_else(|| {
                    response
                        .headers()
                        .get(CONTENT_LENGTH)
                        .and_then(|value| value.to_str().ok())
                        .and_then(|value| value.parse::<u64>().ok())
                        .map(|length| {
                            if partial_content {
                                downloaded.saturating_add(length)
                            } else {
                                length
                            }
                        })
                })
                .or(total_size);

            let before = downloaded;
            let mut file = fs::OpenOptions::new()
                .create(true)
                .write(true)
                .append(downloaded > 0)
                .truncate(downloaded == 0)
                .open(&partial)
                .map_err(|error| format!("Não foi possível abrir o arquivo temporário: {error}"))?;

            match response.copy_to(&mut file) {
                Ok(written) => {
                    use std::io::Write;
                    file.flush().map_err(|error| {
                        format!("Não foi possível finalizar o bloco temporário: {error}")
                    })?;
                    downloaded = before.saturating_add(written);

                    if written == 0 {
                        last_error = Some("o servidor retornou um bloco vazio".to_string());
                        if attempt < MAX_CHUNK_ATTEMPTS {
                            std::thread::sleep(std::time::Duration::from_millis(
                                250 * attempt as u64,
                            ));
                            continue;
                        }
                        break;
                    }

                    chunk_complete = true;
                    break;
                }
                Err(error) => {
                    use std::io::Write;
                    let _ = file.flush();
                    downloaded = fs::metadata(&partial)
                        .map(|metadata| metadata.len())
                        .unwrap_or(before);
                    last_error = Some(format!(
                        "a conexão foi interrompida após {downloaded} bytes: {error}"
                    ));

                    if attempt < MAX_CHUNK_ATTEMPTS {
                        std::thread::sleep(std::time::Duration::from_millis(250 * attempt as u64));
                        continue;
                    }
                }
            }
        }

        if !chunk_complete {
            return Err(format!(
                "O download de '{}' falhou: {}",
                item.title,
                last_error.unwrap_or_else(|| "erro desconhecido".to_string())
            ));
        }

        if total_size.is_none() {
            let current_size = fs::metadata(&partial)
                .map(|metadata| metadata.len())
                .unwrap_or(downloaded);
            if current_size == downloaded && current_size < RANGE_CHUNK_BYTES {
                total_size = Some(current_size);
            }
        }
    }

    if !valid_audio_file(&partial) {
        return Err(format!(
            "O download de '{}' não produziu um arquivo de áudio válido",
            item.title
        ));
    }

    let final_path = store.final_path(&item.video_id, extension);
    if final_path.exists() {
        fs::remove_file(&final_path).map_err(|error| {
            format!(
                "Não foi possível substituir o áudio offline existente de '{}': {error}",
                item.title
            )
        })?;
    }
    fs::rename(&partial, &final_path)
        .map_err(|error| format!("Não foi possível finalizar o download offline: {error}"))?;

    Ok(final_path)
}

fn content_range_total(value: &str) -> Option<u64> {
    value
        .rsplit_once('/')
        .and_then(|(_, total)| (total != "*").then_some(total))
        .and_then(|total| total.parse::<u64>().ok())
}

fn directory_file_stats(path: &Path) -> (usize, u64) {
    let Ok(entries) = fs::read_dir(path) else {
        return (0, 0);
    };

    entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.metadata().ok())
        .filter(|metadata| metadata.is_file())
        .fold((0_usize, 0_u64), |(count, bytes), metadata| {
            (
                count.saturating_add(1),
                bytes.saturating_add(metadata.len()),
            )
        })
}

fn valid_audio_file(path: &Path) -> bool {
    path.is_file()
        && fs::metadata(path)
            .map(|metadata| metadata.len() >= MIN_VALID_AUDIO_BYTES)
            .unwrap_or(false)
}

fn sanitize_video_id(video_id: &str) -> String {
    let sanitized = video_id
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
        .collect::<String>();
    if sanitized.is_empty() {
        "unknown".to_string()
    } else {
        sanitized
    }
}

fn sanitize_extension(extension: &str) -> String {
    let sanitized = extension
        .trim_start_matches('.')
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_ascii_lowercase();
    if sanitized.is_empty() {
        "audio".to_string()
    } else {
        sanitized
    }
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "nocky-offline-store-{name}-{}-{}",
            std::process::id(),
            unix_timestamp()
        ))
    }

    #[test]
    fn register_persist_and_resolve_track() {
        let root = temporary_root("persist");
        let mut store = OfflineStore::load(root.clone());
        let file = store.final_path("video_1", "webm");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, vec![1_u8; MIN_VALID_AUDIO_BYTES as usize]).unwrap();

        store
            .register("video_1", "Title", "Artist", "Album", &file)
            .unwrap();

        let restored = OfflineStore::load(root.clone());
        assert_eq!(restored.resolve("video_1"), Some(file));
        assert_eq!(restored.track_count(), 1);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn incomplete_files_are_not_restored() {
        let root = temporary_root("incomplete");
        let mut store = OfflineStore::load(root.clone());
        let file = store.final_path("video_2", "m4a");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, vec![1_u8; MIN_VALID_AUDIO_BYTES as usize]).unwrap();
        store
            .register("video_2", "Title", "Artist", "Album", &file)
            .unwrap();

        fs::write(&file, [1_u8; 8]).unwrap();
        let restored = OfflineStore::load(root.clone());
        assert!(!restored.contains("video_2"));

        let _ = fs::remove_dir_all(root);
    }
}

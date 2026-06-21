use gtk::glib;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Playlist {
    pub name: String,
    pub tracks: Vec<PathBuf>,
}

impl Default for Playlist {
    fn default() -> Self {
        Self {
            name: String::new(),
            tracks: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub music_directory: Option<PathBuf>,
    pub auto_download_lyrics: bool,
    pub volume: f64,
    pub liked_tracks: Vec<PathBuf>,
    pub playlists: Vec<Playlist>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            music_directory: None,
            auto_download_lyrics: true,
            volume: 0.75,
            liked_tracks: Vec::new(),
            playlists: Vec::new(),
        }
    }
}

impl AppConfig {
    pub fn load() -> Self {
        let path = config_path();
        let source = if path.is_file() {
            path.clone()
        } else {
            legacy_config_path()
        };

        let Ok(contents) = fs::read_to_string(&source) else {
            return Self::default();
        };
        let config: Self = serde_json::from_str(&contents).unwrap_or_default();

        // Transparently migrate settings from the old project name.
        if source != path {
            let _ = config.save();
        }
        config
    }

    pub fn save(&self) -> io::Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let temporary = path.with_extension("json.tmp");
        let contents = serde_json::to_vec_pretty(self)
            .map_err(|error| io::Error::other(error.to_string()))?;
        fs::write(&temporary, contents)?;
        fs::rename(temporary, path)
    }

    pub fn is_liked(&self, path: &Path) -> bool {
        self.liked_tracks.iter().any(|liked| liked == path)
    }

    pub fn toggle_liked(&mut self, path: &Path) -> bool {
        if let Some(index) = self.liked_tracks.iter().position(|liked| liked == path) {
            self.liked_tracks.remove(index);
            false
        } else {
            self.liked_tracks.push(path.to_path_buf());
            true
        }
    }

    pub fn playlist(&self, name: &str) -> Option<&Playlist> {
        self.playlists.iter().find(|playlist| playlist.name == name)
    }

    pub fn create_playlist(&mut self, name: &str) -> bool {
        let name = name.trim();
        if name.is_empty()
            || self
                .playlists
                .iter()
                .any(|playlist| playlist.name.eq_ignore_ascii_case(name))
        {
            return false;
        }
        self.playlists.push(Playlist {
            name: name.to_string(),
            tracks: Vec::new(),
        });
        self.playlists
            .sort_by_key(|playlist| playlist.name.to_lowercase());
        true
    }

    pub fn delete_playlist(&mut self, name: &str) -> bool {
        let before = self.playlists.len();
        self.playlists.retain(|playlist| playlist.name != name);
        self.playlists.len() != before
    }

    pub fn add_to_playlist(&mut self, name: &str, path: &Path) -> bool {
        let Some(playlist) = self
            .playlists
            .iter_mut()
            .find(|playlist| playlist.name == name)
        else {
            return false;
        };
        if playlist.tracks.iter().any(|track| track == path) {
            return false;
        }
        playlist.tracks.push(path.to_path_buf());
        true
    }

    pub fn remove_from_playlist(&mut self, name: &str, path: &Path) -> bool {
        let Some(playlist) = self
            .playlists
            .iter_mut()
            .find(|playlist| playlist.name == name)
        else {
            return false;
        };
        let before = playlist.tracks.len();
        playlist.tracks.retain(|track| track != path);
        playlist.tracks.len() != before
    }
}

pub fn config_path() -> PathBuf {
    glib::user_config_dir().join("nocky").join("config.json")
}

fn legacy_config_path() -> PathBuf {
    glib::user_config_dir()
        .join("noctalia-music")
        .join("config.json")
}

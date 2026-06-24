use gtk::glib;
use serde::{Deserialize, Serialize};
use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Playlist {
    pub name: String,
    pub tracks: Vec<PathBuf>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StartupSource {
    Local,
    YouTube,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VisualTheme {
    #[default]
    Noctalia,
    MaterialExpressive,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BlurMode {
    Custom,
    #[default]
    Noctalia,
    Off,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FooterMode {
    #[default]
    Automatic,
    Full,
    Compact,
    Hidden,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AppLanguage {
    Portuguese,
    English,
    Spanish,
}

impl AppLanguage {
    pub fn detect_system() -> Self {
        let locale = ["LC_ALL", "LC_MESSAGES", "LANGUAGE", "LANG"]
            .into_iter()
            .find_map(|name| env::var(name).ok())
            .unwrap_or_default()
            .to_ascii_lowercase();

        if locale.starts_with("pt") || locale.contains(":pt") {
            Self::Portuguese
        } else if locale.starts_with("es") || locale.contains(":es") {
            Self::Spanish
        } else {
            Self::English
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Portuguese => "Português",
            Self::English => "English",
            Self::Spanish => "Español",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub music_directory: Option<PathBuf>,
    pub auto_download_lyrics: bool,
    pub show_home_visualizer: bool,
    pub show_home_lyrics: bool,
    // home_player_collapse_and_dialog_fix_v2
    pub home_player_collapsed: bool,
    pub visual_theme: VisualTheme,
    pub footer_mode: FooterMode,
    // pixel_player_expressive_transport_v1
    pub expressive_transport_effects: bool,
    pub noctalia_theme_sync: bool,
    pub youtube_auto_sync: bool,
    pub language: AppLanguage,
    pub volume: f64,
    pub liked_tracks: Vec<PathBuf>,
    pub playlists: Vec<Playlist>,
    pub startup_source: Option<StartupSource>,
    pub blur_mode: BlurMode,
    pub blur_opacity: f64,
    pub onboarding_completed: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            music_directory: None,
            auto_download_lyrics: true,
            show_home_visualizer: true,
            show_home_lyrics: true,
            home_player_collapsed: false,
            visual_theme: VisualTheme::Noctalia,
            footer_mode: FooterMode::Automatic,
            expressive_transport_effects: true,
            noctalia_theme_sync: true,
            youtube_auto_sync: true,
            language: AppLanguage::detect_system(),
            volume: 0.75,
            liked_tracks: Vec::new(),
            playlists: Vec::new(),
            startup_source: None,
            blur_mode: BlurMode::Noctalia,
            blur_opacity: 0.74,
            onboarding_completed: false,
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

        let parsed = serde_json::from_str::<serde_json::Value>(&contents).ok();
        let onboarding_was_stored = parsed
            .as_ref()
            .and_then(|value| value.get("onboarding_completed"))
            .is_some();
        let visual_theme_was_stored = parsed
            .as_ref()
            .and_then(|value| value.get("visual_theme"))
            .is_some();
        let legacy_m3_progress = parsed
            .as_ref()
            .and_then(|value| value.get("use_m3_progress"))
            .and_then(serde_json::Value::as_bool);

        let mut config: Self = parsed
            .clone()
            .and_then(|value| serde_json::from_value(value).ok())
            .unwrap_or_default();

        if !visual_theme_was_stored {
            config.visual_theme = match legacy_m3_progress {
                Some(true) => VisualTheme::MaterialExpressive,
                _ => VisualTheme::Noctalia,
            };
        }

        // Existing installations must not be interrupted by the new
        // first-run wizard. Only genuinely new configurations start with
        // onboarding_completed = false.
        if !onboarding_was_stored {
            config.onboarding_completed = true;
        }

        // Transparently migrate settings from the old project name and
        // persist the onboarding migration marker.
        if source != path || !onboarding_was_stored || !visual_theme_was_stored {
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
        let contents =
            serde_json::to_vec_pretty(self).map_err(|error| io::Error::other(error.to_string()))?;
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

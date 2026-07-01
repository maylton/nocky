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
    FrostedGlass,
}

impl VisualTheme {
    pub fn is_expressive(self) -> bool {
        matches!(self, Self::MaterialExpressive | Self::FrostedGlass)
    }

    pub fn uses_dynamic_palette(self) -> bool {
        self.is_expressive()
    }
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
        for locale in ["LC_ALL", "LC_MESSAGES", "LANGUAGE", "LANG"]
            .into_iter()
            .filter_map(|name| env::var(name).ok())
            .flat_map(|value| {
                value
                    .split([':', ';', ','])
                    .map(str::trim)
                    .map(str::to_ascii_lowercase)
                    .collect::<Vec<_>>()
            })
        {
            let locale_name = locale.split(['.', '@']).next().unwrap_or_default().trim();
            if matches!(locale_name, "c" | "posix") {
                continue;
            }
            if locale_name.starts_with("pt") {
                return Self::Portuguese;
            }
            if locale_name.starts_with("es") {
                return Self::Spanish;
            }
        }

        Self::English
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Portuguese => "Português",
            Self::English => "English",
            Self::Spanish => "Español",
        }
    }
}

pub const YOUTUBE_STREAM_SOURCE_KEYS: [&str; 6] =
    ["web_music", "web_creator", "tv", "android_vr", "web", "ios"];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct YouTubeStreamSources {
    pub order: Vec<String>,
    pub disabled: Vec<String>,
}

impl Default for YouTubeStreamSources {
    fn default() -> Self {
        Self {
            order: YOUTUBE_STREAM_SOURCE_KEYS
                .into_iter()
                .map(str::to_string)
                .collect(),
            disabled: vec!["ios".to_string()],
        }
    }
}

impl YouTubeStreamSources {
    fn known(key: &str) -> bool {
        YOUTUBE_STREAM_SOURCE_KEYS.contains(&key)
    }

    fn normalized_keys(values: &[String]) -> Vec<String> {
        let mut normalized = Vec::new();
        for value in values {
            let key = value.trim().to_ascii_lowercase();
            if Self::known(&key) && !normalized.contains(&key) {
                normalized.push(key);
            }
        }
        normalized
    }

    pub fn normalize(&mut self) -> bool {
        let previous = self.clone();

        if self.order.is_empty() && self.disabled.is_empty() {
            *self = Self::default();
            return *self != previous;
        }

        self.order = Self::normalized_keys(&self.order);
        for key in YOUTUBE_STREAM_SOURCE_KEYS {
            if !self.order.iter().any(|value| value == key) {
                self.order.push(key.to_string());
            }
        }

        self.disabled = Self::normalized_keys(&self.disabled);
        if self.effective_order().is_empty() {
            self.disabled.retain(|key| key != "android_vr");
        }

        *self != previous
    }

    pub fn effective_order(&self) -> Vec<String> {
        self.order
            .iter()
            .filter(|key| !self.disabled.contains(key))
            .cloned()
            .collect()
    }

    pub fn is_enabled(&self, key: &str) -> bool {
        Self::known(key) && !self.disabled.iter().any(|disabled| disabled == key)
    }

    pub fn set_enabled(&mut self, key: &str, enabled: bool) -> bool {
        if !Self::known(key) || self.is_enabled(key) == enabled {
            return false;
        }

        if enabled {
            self.disabled.retain(|disabled| disabled != key);
            return true;
        }

        if self.effective_order().len() <= 1 {
            return false;
        }

        self.disabled.push(key.to_string());
        true
    }

    pub fn move_source(&mut self, key: &str, offset: isize) -> bool {
        let Some(index) = self.order.iter().position(|source| source == key) else {
            return false;
        };
        let target = index as isize + offset;
        if target < 0 || target >= self.order.len() as isize {
            return false;
        }
        self.order.swap(index, target as usize);
        true
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub music_directory: Option<PathBuf>,
    pub auto_download_lyrics: bool,
    pub resume_playback_on_startup: bool,
    pub show_home_visualizer: bool,
    pub show_home_lyrics: bool,
    pub show_personalized_home_history: bool,
    pub collect_listening_history: bool,
    pub home_player_collapsed: bool,
    pub visual_theme: VisualTheme,
    pub footer_mode: FooterMode,
    pub expressive_transport_effects: bool,
    pub expressive_home_card_effects: bool,
    pub noctalia_theme_sync: bool,
    pub youtube_auto_sync: bool,
    #[serde(default)]
    pub offline_collection_auto_sync: bool,
    #[serde(default)]
    pub youtube_stream_sources: YouTubeStreamSources,
    pub language: AppLanguage,
    pub volume: f64,
    pub liked_tracks: Vec<PathBuf>,
    #[serde(default)]
    pub favorite_collections: Vec<String>,
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
            resume_playback_on_startup: false,
            show_home_visualizer: true,
            show_home_lyrics: true,
            show_personalized_home_history: true,
            collect_listening_history: true,
            home_player_collapsed: false,
            visual_theme: VisualTheme::Noctalia,
            footer_mode: FooterMode::Automatic,
            expressive_transport_effects: true,
            expressive_home_card_effects: true,
            noctalia_theme_sync: true,
            youtube_auto_sync: true,
            offline_collection_auto_sync: false,
            youtube_stream_sources: YouTubeStreamSources::default(),
            language: AppLanguage::detect_system(),
            volume: 0.75,
            liked_tracks: Vec::new(),
            favorite_collections: Vec::new(),
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

        let stream_sources_normalized = config.youtube_stream_sources.normalize();

        // Existing installations must not be interrupted by the new
        // first-run wizard. Only genuinely new configurations start with
        // onboarding_completed = false.
        if !onboarding_was_stored {
            config.onboarding_completed = true;
        }

        // Transparently migrate settings from the old project name and
        // persist compatibility and normalization markers.
        if source != path
            || !onboarding_was_stored
            || !visual_theme_was_stored
            || stream_sources_normalized
        {
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

    pub fn is_collection_favorite(&self, key: &str) -> bool {
        self.favorite_collections
            .iter()
            .any(|favorite| favorite.eq_ignore_ascii_case(key))
    }

    pub fn toggle_collection_favorite(&mut self, key: &str) -> bool {
        if let Some(index) = self
            .favorite_collections
            .iter()
            .position(|favorite| favorite.eq_ignore_ascii_case(key))
        {
            self.favorite_collections.remove(index);
            false
        } else {
            self.favorite_collections.push(key.to_string());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_sources_default_to_current_policy() {
        let sources = YouTubeStreamSources::default();
        assert_eq!(
            sources.effective_order(),
            ["web_music", "web_creator", "tv", "android_vr", "web"]
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>()
        );
        assert!(!sources.is_enabled("ios"));
    }

    #[test]
    fn stream_sources_normalize_unknown_duplicate_and_missing_keys() {
        let mut sources = YouTubeStreamSources {
            order: vec![
                "TV".to_string(),
                "unknown".to_string(),
                "tv".to_string(),
                "ios".to_string(),
            ],
            disabled: vec!["unknown".to_string(), "web".to_string(), "web".to_string()],
        };

        assert!(sources.normalize());
        assert_eq!(sources.order[0], "tv");
        assert_eq!(sources.order[1], "ios");
        assert_eq!(sources.order.len(), YOUTUBE_STREAM_SOURCE_KEYS.len());
        assert_eq!(sources.disabled, vec!["web"]);
    }

    #[test]
    fn stream_sources_never_leave_policy_empty() {
        let mut sources = YouTubeStreamSources {
            order: YOUTUBE_STREAM_SOURCE_KEYS
                .into_iter()
                .map(str::to_string)
                .collect(),
            disabled: YOUTUBE_STREAM_SOURCE_KEYS
                .into_iter()
                .map(str::to_string)
                .collect(),
        };

        assert!(sources.normalize());
        assert!(sources.is_enabled("android_vr"));
        assert_eq!(sources.effective_order(), vec!["android_vr"]);
        assert!(!sources.set_enabled("android_vr", false));
    }

    #[test]
    fn legacy_config_without_stream_sources_uses_defaults() {
        let config: AppConfig = serde_json::from_value(serde_json::json!({
            "onboarding_completed": true
        }))
        .expect("legacy config should deserialize");

        assert_eq!(
            config.youtube_stream_sources.effective_order(),
            ["web_music", "web_creator", "tv", "android_vr", "web"]
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>()
        );
    }
}

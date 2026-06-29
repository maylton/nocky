#![allow(dead_code)]

use super::{
    HelperResponse, YouTubeBridge, YouTubeHomePage, YouTubeItem, YouTubeLibrarySnapshot,
    YouTubeStatus,
};
use serde::{Deserialize, Serialize};
use std::process::{Command, Stdio};

/// Stable boundary between the GTK application and the current YouTube Music
/// transport. The first implementation remains ytmusicapi-backed, while this
/// trait allows a future native InnerTube backend without changing page logic.
pub trait YouTubeMusicBackend {
    fn status(&self) -> Result<YouTubeStatus, String>;
    fn home_page(
        &self,
        continuation: Option<&str>,
        params: Option<&str>,
    ) -> Result<YouTubeHomePage, String>;
    fn library_overview(&self) -> Result<YouTubeHomePage, String>;
    fn sync_library(&self) -> Result<YouTubeLibrarySnapshot, String>;
    fn search(&self, query: &str, filter: &str) -> Result<Vec<YouTubeItem>, String>;
    fn playlist(&self, playlist: &YouTubeItem) -> Result<Vec<YouTubeItem>, String>;
    fn rate(&self, video_id: &str, liked: bool) -> Result<bool, String>;
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct YouTubeAccountProfile {
    pub name: String,
    pub channel_handle: String,
    pub photo_url: String,
}

impl YouTubeAccountProfile {
    pub fn display_label(&self, fallback_name: &str) -> String {
        let name = if self.name.trim().is_empty() {
            fallback_name.trim()
        } else {
            self.name.trim()
        };
        let handle = self.channel_handle.trim();

        match (name.is_empty(), handle.is_empty()) {
            (false, false) => format!("{name} · {handle}"),
            (false, true) => name.to_string(),
            (true, false) => handle.to_string(),
            (true, true) => String::new(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct YouTubeProfileDiscoverySummary {
    pub state: String,
    pub deterministic: bool,
    pub profile_count: usize,
}

impl YouTubeProfileDiscoverySummary {
    pub fn diagnostic_suffix(&self) -> Option<String> {
        match self.state.as_str() {
            "single" if self.profile_count == 1 && self.deterministic => {
                Some("1 perfil detectado".to_string())
            }
            "multiple" if self.profile_count > 1 && self.deterministic => Some(format!(
                "{} perfis detectados; troca desativada",
                self.profile_count
            )),
            "ambiguous" if self.profile_count > 0 => {
                Some("perfis detectados; seleção indisponível".to_string())
            }
            _ => None,
        }
    }
}

impl YouTubeBridge {
    pub fn account_profile(&self) -> Result<YouTubeAccountProfile, String> {
        let helper = self
            .helper
            .parent()
            .map(|directory| directory.join("nocky_youtube_profile.py"))
            .filter(|path| path.is_file())
            .ok_or_else(|| {
                "The Nocky YouTube account-profile helper was not found. Reinstall Nocky."
                    .to_string()
            })?;

        let output = Command::new(&self.python)
            .arg(helper)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|error| format!("Could not start the YouTube profile helper: {error}"))?;

        let response: HelperResponse<YouTubeAccountProfile> =
            serde_json::from_slice(&output.stdout).map_err(|error| {
                let stderr = String::from_utf8_lossy(&output.stderr);
                format!("Invalid response from the YouTube profile helper: {error}. {stderr}")
            })?;

        if !response.ok {
            return Err(response.error.unwrap_or_else(|| {
                "The YouTube profile helper reported an unknown error".to_string()
            }));
        }

        response
            .result
            .ok_or_else(|| "The YouTube profile helper returned no result".to_string())
    }

    pub fn account_discovery_summary(&self) -> Result<YouTubeProfileDiscoverySummary, String> {
        let helper = self
            .helper
            .parent()
            .map(|directory| directory.join("nocky_youtube_profiles.py"))
            .filter(|path| path.is_file())
            .ok_or_else(|| {
                "The Nocky YouTube profile-discovery helper was not found. Reinstall Nocky."
                    .to_string()
            })?;

        let output = Command::new(&self.python)
            .arg(helper)
            .arg("--summary")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|error| {
                format!("Could not start the YouTube profile-discovery helper: {error}")
            })?;

        let response: HelperResponse<YouTubeProfileDiscoverySummary> =
            serde_json::from_slice(&output.stdout).map_err(|error| {
                let stderr = String::from_utf8_lossy(&output.stderr);
                format!(
                    "Invalid response from the YouTube profile-discovery helper: {error}. {stderr}"
                )
            })?;

        if !response.ok {
            return Err(response.error.unwrap_or_else(|| {
                "The YouTube profile-discovery helper reported an unknown error".to_string()
            }));
        }

        response
            .result
            .ok_or_else(|| "The YouTube profile-discovery helper returned no result".to_string())
    }

    pub fn status_with_profile(&self) -> Result<YouTubeStatus, String> {
        let mut status = YouTubeBridge::status(self)?;
        if status.connected {
            match self.account_profile() {
                Ok(profile) => {
                    let label = profile.display_label(&status.account);
                    if !label.is_empty() {
                        status.account = label;
                    }
                }
                Err(error) => {
                    eprintln!("Could not refresh YouTube Music account profile: {error}");
                }
            }

            match self.account_discovery_summary() {
                Ok(discovery) => {
                    if let Some(suffix) = discovery.diagnostic_suffix() {
                        if status.account.trim().is_empty() {
                            status.account = format!("Perfis: {suffix}");
                        } else {
                            status.account = format!("{}\nPerfis: {suffix}", status.account.trim());
                        }
                    }
                }
                Err(error) => {
                    eprintln!("Could not inspect YouTube Music account profiles: {error}");
                }
            }
        }
        Ok(status)
    }

    pub fn connect_with_profile(&self, raw: &str) -> Result<YouTubeStatus, String> {
        YouTubeBridge::connect(self, raw)?;
        self.status_with_profile()
    }

    pub fn disconnect_with_profile(&self) -> Result<YouTubeStatus, String> {
        YouTubeBridge::disconnect(self)
    }
}

impl YouTubeMusicBackend for YouTubeBridge {
    fn status(&self) -> Result<YouTubeStatus, String> {
        self.status_with_profile()
    }

    fn home_page(
        &self,
        continuation: Option<&str>,
        params: Option<&str>,
    ) -> Result<YouTubeHomePage, String> {
        YouTubeBridge::home_page(self, continuation, params)
    }

    fn library_overview(&self) -> Result<YouTubeHomePage, String> {
        YouTubeBridge::library_overview(self)
    }

    fn sync_library(&self) -> Result<YouTubeLibrarySnapshot, String> {
        YouTubeBridge::sync_library(self)
    }

    fn search(&self, query: &str, filter: &str) -> Result<Vec<YouTubeItem>, String> {
        YouTubeBridge::search(self, query, filter)
    }

    fn playlist(&self, playlist: &YouTubeItem) -> Result<Vec<YouTubeItem>, String> {
        YouTubeBridge::playlist(self, playlist)
    }

    fn rate(&self, video_id: &str, liked: bool) -> Result<bool, String> {
        YouTubeBridge::rate(self, video_id, liked)
    }
}

#[cfg(test)]
mod tests {
    use super::{YouTubeAccountProfile, YouTubeProfileDiscoverySummary};

    #[test]
    fn profile_display_includes_name_and_handle() {
        let profile = YouTubeAccountProfile {
            name: "Sample profile".to_string(),
            channel_handle: "@sample".to_string(),
            photo_url: "https://example.invalid/avatar.jpg".to_string(),
        };

        assert_eq!(
            profile.display_label("Fallback"),
            "Sample profile · @sample"
        );
    }

    #[test]
    fn profile_display_falls_back_to_legacy_account_name() {
        assert_eq!(
            YouTubeAccountProfile::default().display_label("Existing account"),
            "Existing account"
        );
    }

    #[test]
    fn profile_deserialization_accepts_missing_fields() {
        let profile: YouTubeAccountProfile =
            serde_json::from_value(serde_json::json!({ "name": "Profile" })).unwrap();

        assert_eq!(profile.name, "Profile");
        assert!(profile.channel_handle.is_empty());
        assert!(profile.photo_url.is_empty());
    }

    #[test]
    fn discovery_deserialization_accepts_minimal_contract() {
        let discovery: YouTubeProfileDiscoverySummary = serde_json::from_value(serde_json::json!({
            "state": "multiple",
            "deterministic": true,
            "profile_count": 2
        }))
        .unwrap();

        assert_eq!(discovery.state, "multiple");
        assert!(discovery.deterministic);
        assert_eq!(discovery.profile_count, 2);
    }

    #[test]
    fn single_profile_discovery_has_read_only_status() {
        let discovery = YouTubeProfileDiscoverySummary {
            state: "single".to_string(),
            deterministic: true,
            profile_count: 1,
        };

        assert_eq!(
            discovery.diagnostic_suffix().as_deref(),
            Some("1 perfil detectado")
        );
    }

    #[test]
    fn multiple_profile_discovery_keeps_switching_disabled() {
        let discovery = YouTubeProfileDiscoverySummary {
            state: "multiple".to_string(),
            deterministic: true,
            profile_count: 3,
        };

        assert_eq!(
            discovery.diagnostic_suffix().as_deref(),
            Some("3 perfis detectados; troca desativada")
        );
    }

    #[test]
    fn ambiguous_discovery_never_offers_selection() {
        let discovery = YouTubeProfileDiscoverySummary {
            state: "ambiguous".to_string(),
            deterministic: false,
            profile_count: 2,
        };

        assert_eq!(
            discovery.diagnostic_suffix().as_deref(),
            Some("perfis detectados; seleção indisponível")
        );
    }

    #[test]
    fn unavailable_discovery_stays_silent() {
        assert_eq!(
            YouTubeProfileDiscoverySummary::default().diagnostic_suffix(),
            None
        );
    }
}

use crate::mpris::{MprisPlayback, MprisTrack, MprisUpdate};
use serde_json::{json, Value};
use std::{
    env, fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const BRIDGE_VERSION: u64 = 1;
const BRIDGE_SCHEME: &str = "m3-content";

pub struct AlbumAuraBridge {
    path: Option<PathBuf>,
    track: Option<MprisTrack>,
    playback: MprisPlayback,
    revision: u64,
}

impl AlbumAuraBridge {
    pub fn discover() -> Self {
        let path = env::var_os("XDG_RUNTIME_DIR")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .map(|root| root.join("nocky").join("album-aura.json"));

        Self {
            path,
            track: None,
            playback: MprisPlayback::Stopped,
            revision: 0,
        }
    }

    pub fn apply_mpris_update(&mut self, update: &MprisUpdate) {
        match update {
            MprisUpdate::Metadata(track) => {
                self.track = Some(track.clone());
                self.publish();
            }
            MprisUpdate::ClearMetadata => {
                self.track = None;
                self.publish_inactive();
            }
            MprisUpdate::Playback(playback) => {
                self.playback = *playback;
                if matches!(playback, MprisPlayback::Stopped) {
                    self.publish_inactive();
                } else {
                    self.publish();
                }
            }
            MprisUpdate::Shutdown => self.shutdown(),
            _ => {}
        }
    }

    pub fn shutdown(&mut self) {
        self.playback = MprisPlayback::Stopped;
        self.publish_inactive();
    }

    fn publish(&mut self) {
        let Some(track) = self.track.as_ref() else {
            self.publish_inactive();
            return;
        };

        if matches!(self.playback, MprisPlayback::Stopped) {
            self.publish_inactive();
            return;
        }

        self.revision = self.revision.saturating_add(1);
        let mut payload = json!({
            "version": BRIDGE_VERSION,
            "active": true,
            "player": "Nocky",
            "mpris_track_id": track.track_id,
            "track_id": track.track_id,
            "title": track.title,
            "artist": track.artist,
            "album": track.album,
            "source": source_name(track),
            "playback_status": playback_name(self.playback),
            "scheme": BRIDGE_SCHEME,
            "revision": self.revision,
            "updated_at": unix_timestamp(),
        });

        if let Some(art_url) = track.art_url.as_deref().filter(|value| !value.is_empty()) {
            if let Some(path) = file_uri_to_path(art_url) {
                payload["artwork_path"] = Value::String(path.to_string_lossy().into_owned());
            } else {
                payload["artwork_url"] = Value::String(art_url.to_string());
            }
        }

        self.write_payload(&payload);
    }

    fn publish_inactive(&mut self) {
        self.revision = self.revision.saturating_add(1);
        let payload = json!({
            "version": BRIDGE_VERSION,
            "active": false,
            "player": "Nocky",
            "playback_status": "Stopped",
            "revision": self.revision,
            "updated_at": unix_timestamp(),
        });
        self.write_payload(&payload);
    }

    fn write_payload(&self, payload: &Value) {
        let Some(path) = self.path.as_ref() else {
            return;
        };

        if let Err(error) = write_json_atomically(path, payload) {
            eprintln!("Nocky Album Aura bridge error: {error}");
        }
    }
}

fn source_name(track: &MprisTrack) -> &'static str {
    match track.url.as_deref() {
        Some(url) if url.starts_with("http://") || url.starts_with("https://") => "youtube",
        _ => "local",
    }
}

const fn playback_name(playback: MprisPlayback) -> &'static str {
    match playback {
        MprisPlayback::Playing => "Playing",
        MprisPlayback::Paused => "Paused",
        MprisPlayback::Stopped => "Stopped",
    }
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn file_uri_to_path(uri: &str) -> Option<PathBuf> {
    let encoded = uri.strip_prefix("file://")?;
    let decoded = percent_decode(encoded)?;
    Some(PathBuf::from(decoded))
}

fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = *bytes.get(index + 1)?;
            let low = *bytes.get(index + 2)?;
            output.push((hex_value(high)? << 4) | hex_value(low)?);
            index += 3;
        } else {
            output.push(bytes[index]);
            index += 1;
        }
    }

    String::from_utf8(output).ok()
}

const fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn write_json_atomically(path: &Path, payload: &Value) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "bridge path has no parent directory".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("could not create {}: {error}", parent.display()))?;

    let temporary = parent.join(format!(".album-aura.json.{}.tmp", std::process::id()));
    let serialized = serde_json::to_vec_pretty(payload)
        .map_err(|error| format!("could not serialize bridge payload: {error}"))?;

    fs::write(&temporary, serialized)
        .map_err(|error| format!("could not write {}: {error}", temporary.display()))?;
    fs::rename(&temporary, path)
        .map_err(|error| format!("could not replace {}: {error}", path.display()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_file_uri_for_album_aura() {
        assert_eq!(
            file_uri_to_path("file:///home/user/Music/Album%20Cover.webp"),
            Some(PathBuf::from("/home/user/Music/Album Cover.webp"))
        );
    }

    #[test]
    fn identifies_online_tracks_as_youtube_source() {
        let track = MprisTrack {
            track_id: "/io/github/maylton/Nocky/track/test".into(),
            title: "Title".into(),
            artist: "Artist".into(),
            album: "Album".into(),
            length_us: 1,
            art_url: None,
            url: Some("https://music.youtube.com/watch?v=test".into()),
        };
        assert_eq!(source_name(&track), "youtube");
    }

    #[test]
    fn writes_bridge_atomically() {
        let root = env::temp_dir().join(format!(
            "nocky-album-aura-test-{}-{}",
            std::process::id(),
            unix_timestamp()
        ));
        let path = root.join("nocky").join("album-aura.json");
        let payload = json!({"version": 1, "active": true});

        write_json_atomically(&path, &payload).expect("bridge write must succeed");
        let stored: Value =
            serde_json::from_slice(&fs::read(&path).expect("bridge file must exist"))
                .expect("bridge JSON must be valid");

        assert_eq!(stored["active"], true);
        let _ = fs::remove_dir_all(root);
    }
}

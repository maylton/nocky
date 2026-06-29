#[path = "playlist_metadata.rs"]
mod playlist_metadata_model;

use super::{HelperResponse, YouTubeBridge, YouTubePage, YouTubePageEvent};
use adw::prelude::*;
use playlist_metadata_model::{YouTubePlaylistMetadata, YouTubePlaylistPrivacy};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use std::{
    path::PathBuf,
    process::{Command, Stdio},
};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct YouTubePlaylistCreation {
    pub playlist_id: String,
    pub title: String,
    pub privacy: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct YouTubePlaylistAddition {
    pub playlist_id: String,
    pub video_id: String,
    pub added_count: usize,
    pub reconciliation_required: bool,
}

impl YouTubeBridge {
    fn playlist_helper_path(&self) -> Result<PathBuf, String> {
        self.helper
            .parent()
            .map(|directory| directory.join("nocky_youtube_playlist_create.py"))
            .filter(|path| path.is_file())
            .ok_or_else(|| {
                "The Nocky YouTube playlist helper was not found. Reinstall Nocky.".to_string()
            })
    }

    fn run_playlist_helper<T: DeserializeOwned>(
        &self,
        payload: serde_json::Value,
        operation: &str,
    ) -> Result<T, String> {
        let helper = self.playlist_helper_path()?;
        let mut child = Command::new(&self.python)
            .arg(helper)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("Could not start the YouTube playlist helper: {error}"))?;

        if let Some(mut stdin) = child.stdin.take() {
            serde_json::to_writer(&mut stdin, &payload).map_err(|error| {
                format!("Could not send the playlist {operation} request: {error}")
            })?;
        }

        let output = child
            .wait_with_output()
            .map_err(|error| format!("The YouTube playlist helper did not finish: {error}"))?;
        let response: HelperResponse<T> =
            serde_json::from_slice(&output.stdout).map_err(|error| {
                let stderr = String::from_utf8_lossy(&output.stderr);
                format!("Invalid response from the YouTube playlist helper: {error}. {stderr}")
            })?;

        if !response.ok {
            return Err(response.error.unwrap_or_else(|| {
                format!("The YouTube playlist helper reported an unknown {operation} error")
            }));
        }

        response
            .result
            .ok_or_else(|| format!("The YouTube playlist helper returned no {operation} result"))
    }

    pub fn create_empty_playlist(
        &self,
        title: &str,
        description: &str,
        privacy: &str,
    ) -> Result<YouTubePlaylistCreation, String> {
        self.run_playlist_helper(
            json!({
                "operation": "create",
                "title": title,
                "description": description,
                "privacy": privacy,
            }),
            "creation",
        )
    }

    pub fn playlist_metadata_access(&self, playlist_id: &str) -> Result<(String, bool), String> {
        let metadata: YouTubePlaylistMetadata = self.run_playlist_helper(
            json!({
                "operation": "metadata",
                "playlist_id": playlist_id,
                "limit": 500,
            }),
            "metadata",
        )?;

        if metadata.playlist_id.trim() != playlist_id.trim().trim_start_matches("VL") {
            return Err("The YouTube playlist helper returned mismatched metadata".to_string());
        }

        let editable = metadata.can_edit();
        Ok((format_playlist_metadata_diagnostic(&metadata), editable))
    }

    pub fn playlist_metadata_diagnostic(&self, playlist_id: &str) -> Result<String, String> {
        self.playlist_metadata_access(playlist_id)
            .map(|(diagnostic, _)| diagnostic)
    }

    pub fn add_playlist_item(
        &self,
        playlist_id: &str,
        video_id: &str,
    ) -> Result<YouTubePlaylistAddition, String> {
        self.run_playlist_helper(
            json!({
                "operation": "add",
                "playlist_id": playlist_id,
                "video_id": video_id,
                "owned": true,
                "editable": true,
                "duplicates": false,
            }),
            "item addition",
        )
    }
}

fn format_playlist_metadata_diagnostic(metadata: &YouTubePlaylistMetadata) -> String {
    let privacy = match metadata.privacy_kind() {
        YouTubePlaylistPrivacy::Private => "privada",
        YouTubePlaylistPrivacy::Unlisted => "não listada",
        YouTubePlaylistPrivacy::Public => "pública",
        YouTubePlaylistPrivacy::Unknown => "privacidade desconhecida",
    };

    if !metadata.owned {
        return format!("Playlist compartilhada • {privacy} • somente leitura");
    }
    if !metadata.can_edit() {
        return format!("Playlist própria • {privacy} • edição indisponível");
    }

    let identified = metadata.removable_track_count();
    let total = metadata.tracks.len();
    let identity = if identified == total && metadata.has_unique_removal_identities() {
        match identified {
            1 => "1 ocorrência identificada".to_string(),
            count => format!("{count} ocorrências identificadas"),
        }
    } else {
        format!("{identified} de {total} ocorrências identificadas")
    };
    format!("Playlist própria • {privacy} • {identity}")
}

pub fn playlist_creation_error_message(error: &str) -> &'static str {
    let normalized = error.to_lowercase();
    if normalized.contains("session")
        || normalized.contains("authentication")
        || normalized.contains("unauthorized")
        || normalized.contains("401")
    {
        "A sessão do YouTube Music expirou. Reconecte sua conta para criar playlists."
    } else if normalized.contains("permission")
        || normalized.contains("forbidden")
        || normalized.contains("403")
    {
        "O YouTube Music recusou a criação da playlist. Verifique as permissões da conta."
    } else if normalized.contains("network")
        || normalized.contains("offline")
        || normalized.contains("timed out")
        || normalized.contains("timeout")
        || normalized.contains("connect")
    {
        "Sem conexão com o YouTube Music. A playlist não foi criada."
    } else if normalized.contains("title") {
        "Escolha um título válido para a playlist."
    } else {
        "Não foi possível criar a playlist no YouTube Music."
    }
}

pub fn playlist_add_error_message(error: &str) -> &'static str {
    let normalized = error.to_lowercase();
    if normalized.contains("session")
        || normalized.contains("authentication")
        || normalized.contains("unauthorized")
        || normalized.contains("401")
    {
        "A sessão do YouTube Music expirou. Reconecte sua conta para editar playlists."
    } else if normalized.contains("ownership")
        || normalized.contains("editability")
        || normalized.contains("permission")
        || normalized.contains("forbidden")
        || normalized.contains("403")
    {
        "Esta playlist não está disponível para edição nesta conta."
    } else if normalized.contains("duplicate") || normalized.contains("already") {
        "Esta música já está na playlist."
    } else if normalized.contains("network")
        || normalized.contains("offline")
        || normalized.contains("timed out")
        || normalized.contains("timeout")
        || normalized.contains("connect")
    {
        "Não foi possível confirmar a adição. A playlist não foi alterada no Nocky."
    } else if normalized.contains("video id") || normalized.contains("playlist id") {
        "A música ou a playlist não possui um identificador válido."
    } else {
        "Não foi possível adicionar a música à playlist."
    }
}

fn privacy_code(selected: u32) -> &'static str {
    match selected {
        1 => "UNLISTED",
        2 => "PUBLIC",
        _ => "PRIVATE",
    }
}

impl YouTubePage {
    pub(super) fn present_playlist_create_dialog(&self) {
        let dialog = adw::Dialog::builder()
            .title("Criar playlist")
            .content_width(480)
            .build();
        dialog.add_css_class("playlist-create-dialog");

        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&adw::HeaderBar::new());

        let content = gtk::Box::new(gtk::Orientation::Vertical, 14);
        content.set_margin_top(20);
        content.set_margin_bottom(20);
        content.set_margin_start(20);
        content.set_margin_end(20);

        let intro = gtk::Label::new(Some(
            "Crie uma playlist vazia na conta conectada. Ela será privada por padrão.",
        ));
        intro.set_wrap(true);
        intro.set_xalign(0.0);
        intro.add_css_class("dim-label");

        let title_label = gtk::Label::new(Some("Título"));
        title_label.set_xalign(0.0);
        title_label.add_css_class("heading");
        let title_entry = gtk::Entry::new();
        title_entry.set_placeholder_text(Some("Nome da playlist"));
        title_entry.set_max_length(150);

        let description_label = gtk::Label::new(Some("Descrição opcional"));
        description_label.set_xalign(0.0);
        description_label.add_css_class("heading");
        let description_entry = gtk::Entry::new();
        description_entry.set_placeholder_text(Some("Uma breve descrição"));
        description_entry.set_max_length(500);

        let privacy_label = gtk::Label::new(Some("Privacidade"));
        privacy_label.set_xalign(0.0);
        privacy_label.add_css_class("heading");
        let privacy = gtk::DropDown::from_strings(&["Privada", "Não listada", "Pública"]);
        privacy.set_selected(0);
        privacy.set_hexpand(true);

        let cancel = gtk::Button::with_label("Cancelar");
        cancel.add_css_class("flat");
        let create = gtk::Button::with_label("Criar playlist");
        create.add_css_class("suggested-action");
        create.set_sensitive(false);

        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        actions.set_halign(gtk::Align::End);
        actions.append(&cancel);
        actions.append(&create);

        content.append(&intro);
        content.append(&title_label);
        content.append(&title_entry);
        content.append(&description_label);
        content.append(&description_entry);
        content.append(&privacy_label);
        content.append(&privacy);
        content.append(&actions);

        toolbar.set_content(Some(&content));
        dialog.set_child(Some(&toolbar));

        {
            let create = create.clone();
            title_entry.connect_changed(move |entry| {
                create.set_sensitive(!entry.text().trim().is_empty());
            });
        }
        {
            let dialog = dialog.clone();
            cancel.connect_clicked(move |_| {
                dialog.close();
            });
        }
        {
            let sender = self.event_tx.clone();
            let dialog = dialog.clone();
            let title_entry = title_entry.clone();
            let description_entry = description_entry.clone();
            let privacy = privacy.clone();
            create.connect_clicked(move |_| {
                let title = title_entry.text().trim().to_string();
                if title.is_empty() {
                    return;
                }
                let description = description_entry.text().trim().to_string();
                let privacy = privacy_code(privacy.selected()).to_string();
                let _ = sender.send(YouTubePageEvent::CreatePlaylist {
                    title,
                    description,
                    privacy,
                });
                dialog.close();
            });
        }

        dialog.present(Some(&self.root));
        title_entry.grab_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::playlist_metadata_model::{YouTubePlaylistMetadata, YouTubePlaylistTrackMetadata};
    use super::{
        format_playlist_metadata_diagnostic, playlist_add_error_message,
        playlist_creation_error_message, privacy_code, YouTubePlaylistAddition,
        YouTubePlaylistCreation,
    };

    #[test]
    fn privacy_selection_defaults_to_private() {
        assert_eq!(privacy_code(0), "PRIVATE");
        assert_eq!(privacy_code(1), "UNLISTED");
        assert_eq!(privacy_code(2), "PUBLIC");
        assert_eq!(privacy_code(99), "PRIVATE");
    }

    #[test]
    fn creation_result_accepts_the_minimal_allowlist() {
        let result: YouTubePlaylistCreation = serde_json::from_value(serde_json::json!({
            "playlist_id": "PL_created",
            "title": "Focus",
            "privacy": "PRIVATE"
        }))
        .unwrap();

        assert_eq!(result.playlist_id, "PL_created");
        assert_eq!(result.title, "Focus");
        assert_eq!(result.privacy, "PRIVATE");
    }

    #[test]
    fn addition_result_accepts_only_the_sanitized_contract() {
        let result: YouTubePlaylistAddition = serde_json::from_value(serde_json::json!({
            "playlist_id": "PL_owned",
            "video_id": "abcdefghijk",
            "added_count": 1,
            "reconciliation_required": true
        }))
        .unwrap();

        assert_eq!(result.playlist_id, "PL_owned");
        assert_eq!(result.video_id, "abcdefghijk");
        assert_eq!(result.added_count, 1);
        assert!(result.reconciliation_required);
    }

    #[test]
    fn creation_errors_are_actionable_without_raw_details() {
        assert!(playlist_creation_error_message("401 unauthorized").contains("expirou"));
        assert!(playlist_creation_error_message("network timeout").contains("Sem conexão"));
        assert!(playlist_creation_error_message("unknown failure").contains("Não foi possível"));
    }

    #[test]
    fn addition_errors_are_actionable_without_raw_details() {
        assert!(playlist_add_error_message("401 unauthorized").contains("expirou"));
        assert!(playlist_add_error_message("ownership missing").contains("não está disponível"));
        assert!(playlist_add_error_message("network timeout").contains("não foi alterada"));
    }

    #[test]
    fn metadata_diagnostic_marks_owned_private_playlist() {
        let metadata = YouTubePlaylistMetadata {
            playlist_id: "PL-owned".to_string(),
            owned: true,
            editable: true,
            privacy: "PRIVATE".to_string(),
            tracks: vec![YouTubePlaylistTrackMetadata {
                video_id: "abcdefghijk".to_string(),
                set_video_id: "set-occurrence-1".to_string(),
                title: "Track".to_string(),
            }],
            ..YouTubePlaylistMetadata::default()
        };

        assert_eq!(
            format_playlist_metadata_diagnostic(&metadata),
            "Playlist própria • privada • 1 ocorrência identificada"
        );
    }

    #[test]
    fn metadata_diagnostic_keeps_shared_playlist_read_only() {
        let metadata = YouTubePlaylistMetadata {
            playlist_id: "PL-shared".to_string(),
            owned: false,
            editable: false,
            privacy: "UNLISTED".to_string(),
            ..YouTubePlaylistMetadata::default()
        };

        assert_eq!(
            format_playlist_metadata_diagnostic(&metadata),
            "Playlist compartilhada • não listada • somente leitura"
        );
    }
}

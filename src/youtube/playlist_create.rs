use super::{HelperResponse, YouTubeBridge, YouTubePage, YouTubePageEvent};
use adw::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::process::{Command, Stdio};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct YouTubePlaylistCreation {
    pub playlist_id: String,
    pub title: String,
    pub privacy: String,
}

impl YouTubeBridge {
    pub fn create_empty_playlist(
        &self,
        title: &str,
        description: &str,
        privacy: &str,
    ) -> Result<YouTubePlaylistCreation, String> {
        let helper = self
            .helper
            .parent()
            .map(|directory| directory.join("nocky_youtube_playlist_create.py"))
            .filter(|path| path.is_file())
            .ok_or_else(|| {
                "The Nocky YouTube playlist-creation helper was not found. Reinstall Nocky."
                    .to_string()
            })?;

        let mut child = Command::new(&self.python)
            .arg(helper)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                format!("Could not start the YouTube playlist-creation helper: {error}")
            })?;

        if let Some(mut stdin) = child.stdin.take() {
            serde_json::to_writer(
                &mut stdin,
                &json!({
                    "title": title,
                    "description": description,
                    "privacy": privacy,
                }),
            )
            .map_err(|error| {
                format!("Could not send the playlist request to the YouTube helper: {error}")
            })?;
        }

        let output = child
            .wait_with_output()
            .map_err(|error| format!("The YouTube playlist helper did not finish: {error}"))?;
        let response: HelperResponse<YouTubePlaylistCreation> =
            serde_json::from_slice(&output.stdout).map_err(|error| {
                let stderr = String::from_utf8_lossy(&output.stderr);
                format!(
                    "Invalid response from the YouTube playlist-creation helper: {error}. {stderr}"
                )
            })?;

        if !response.ok {
            return Err(response.error.unwrap_or_else(|| {
                "The YouTube playlist-creation helper reported an unknown error".to_string()
            }));
        }

        response
            .result
            .ok_or_else(|| "The YouTube playlist-creation helper returned no result".to_string())
    }
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
    use super::{playlist_creation_error_message, privacy_code, YouTubePlaylistCreation};

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
    fn creation_errors_are_actionable_without_raw_details() {
        assert!(playlist_creation_error_message("401 unauthorized").contains("expirou"));
        assert!(playlist_creation_error_message("network timeout").contains("Sem conexão"));
        assert!(playlist_creation_error_message("unknown failure").contains("Não foi possível"));
    }
}

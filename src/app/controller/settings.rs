//! Settings and onboarding controller methods for `AppController`.

use super::AppController;
use crate::{
    config::{BlurMode, StartupSource},
    dialogs::{self, SettingsEvent},
    onboarding,
};
use adw::prelude::*;
use gtk::{gio, glib};
use std::rc::Rc;

impl AppController {
    pub(crate) fn open_settings_page(&self) {
        let initial = self.config.borrow().clone();
        self.settings_page
            .rebuild(&initial, self._theme.noctalia_shell_detected());
        self.search_button.set_active(false);
        self.content_stack.set_visible_child_name("settings");
        if !self.settings_button.is_active() {
            self.settings_button.set_active(true);
        }
        self.apply_footer_mode();
    }

    pub(crate) fn close_settings_page(&self) {
        if self.content_stack.visible_child_name().as_deref() != Some("settings") {
            return;
        }
        self.content_stack.set_visible_child_name("main");
        if self.settings_button.is_active() {
            self.settings_button.set_active(false);
        }
        self.apply_footer_mode();
    }

    pub(crate) fn handle_settings_events(self: &Rc<Self>) {
        while let Some(event) = self.settings_page.try_recv() {
            self.apply_settings_event(event);
        }
    }

    pub(crate) fn apply_settings_event(self: &Rc<Self>, event: SettingsEvent) {
        match event {
            SettingsEvent::Language(language) => {
                self.config.borrow_mut().language = language;
                self.save_config();
                self.apply_translations();
                let initial = self.config.borrow().clone();
                self.settings_page
                    .rebuild(&initial, self._theme.noctalia_shell_detected());
            }
            SettingsEvent::StartupSource(source) => self.set_startup_source(source),
            SettingsEvent::BlurMode(mode) => {
                self.config.borrow_mut().blur_mode = mode;
                self.save_config();
                self.apply_home_preferences();
            }
            SettingsEvent::BlurOpacityPreview(value) => {
                let custom = {
                    let mut config = self.config.borrow_mut();
                    config.blur_opacity = value;
                    config.blur_mode == BlurMode::Custom
                };
                if custom {
                    self.apply_home_preferences();
                }
            }
            SettingsEvent::BlurOpacityCommit(value) => {
                self.config.borrow_mut().blur_opacity = value;
                self.save_config();
            }
            SettingsEvent::ShowHomeVisualizer(active) => {
                self.config.borrow_mut().show_home_visualizer = active;
                self.save_config();
                self.apply_home_preferences();
            }
            SettingsEvent::ShowHomeLyrics(active) => {
                self.config.borrow_mut().show_home_lyrics = active;
                self.save_config();
                self.apply_home_preferences();
            }
            SettingsEvent::ShowPersonalizedHomeHistory(active) => {
                self.config.borrow_mut().show_personalized_home_history = active;
                self.save_config();
                self.refresh_browser();
            }
            SettingsEvent::CollectListeningHistory(active) => {
                self.config.borrow_mut().collect_listening_history = active;
                self.listening_history
                    .borrow_mut()
                    .set_recording_enabled(active);
                self.save_config();
                self.show_toast(if active {
                    "O Nocky voltou a aprender com sua atividade"
                } else {
                    "O registro de novas reproduções foi desativado"
                });
            }
            SettingsEvent::ClearListeningHistory => {
                let cleared = self.listening_history.borrow_mut().clear();
                self.refresh_browser();
                self.show_toast(if cleared {
                    "Histórico de reprodução apagado"
                } else {
                    "O histórico já está vazio"
                });
            }
            SettingsEvent::VisualTheme(theme) => {
                self.config.borrow_mut().visual_theme = theme;
                self.save_config();
                self.apply_visual_theme();
                self.refresh_browser();
            }
            SettingsEvent::FooterMode(mode) => {
                self.config.borrow_mut().footer_mode = mode;
                self.save_config();
                self.apply_footer_mode();
            }
            SettingsEvent::ExpressiveTransportEffects(active) => {
                self.config.borrow_mut().expressive_transport_effects = active;
                self.save_config();
                self.apply_expressive_transport_effects();
            }
            SettingsEvent::ExpressiveHomeCardEffects(active) => {
                self.config.borrow_mut().expressive_home_card_effects = active;
                self.save_config();
                self.refresh_browser();
            }
            SettingsEvent::AutoDownloadLyrics(active) => {
                self.config.borrow_mut().auto_download_lyrics = active;
                self.save_config();
                self.apply_home_preferences();
            }
            SettingsEvent::ResumePlaybackOnStartup(active) => {
                self.config.borrow_mut().resume_playback_on_startup = active;
                self.save_config();
            }
            SettingsEvent::YouTubeAutoSync(active) => {
                self.config.borrow_mut().youtube_auto_sync = active;
                self.save_config();
                self.apply_home_preferences();
            }
            SettingsEvent::OfflineCollectionAutoSync(active) => {
                self.config.borrow_mut().offline_collection_auto_sync = active;
                self.save_config();
                if active {
                    self.sync_followed_offline_collections();
                }
            }
            SettingsEvent::NoctaliaThemeSync(active) => {
                self.config.borrow_mut().noctalia_theme_sync = active;
                self.save_config();
                self.apply_home_preferences();
            }
            SettingsEvent::ManageYouTube => self.show_youtube_settings_dialog(),
            SettingsEvent::OpenOfflineFolder => {
                let path = self.offline_store.borrow().root_dir();
                if let Err(error) = std::fs::create_dir_all(&path) {
                    self.show_toast(&format!("Não foi possível abrir a pasta offline: {error}"));
                    return;
                }

                let file = gio::File::for_path(path);
                if let Err(error) = gio::AppInfo::launch_default_for_uri(
                    &file.uri(),
                    None::<&gio::AppLaunchContext>,
                ) {
                    self.show_toast(&format!("Não foi possível abrir a pasta offline: {error}"));
                }
            }
            SettingsEvent::CleanOfflinePartials => {
                let result = self.offline_store.borrow().clear_partials();
                match result {
                    Ok(0) => self.show_toast("Não há downloads incompletos para remover"),
                    Ok(count) => {
                        self.show_toast(&format!("{count} arquivos incompletos foram removidos"))
                    }
                    Err(error) => self.show_toast(&error),
                }

                let initial = self.config.borrow().clone();
                self.settings_page
                    .rebuild(&initial, self._theme.noctalia_shell_detected());
            }
            SettingsEvent::ClearOfflineDownloads => {
                if !self.offline_download_pending.borrow().is_empty() {
                    self.show_toast(
                        "Aguarde os downloads atuais terminarem antes de limpar os arquivos",
                    );
                    return;
                }

                let result = self.offline_store.borrow_mut().clear_all();
                match result {
                    Ok((0, _)) => self.show_toast("O armazenamento offline já está vazio"),
                    Ok((count, _)) => self.show_toast(&format!(
                        "{count} faixas offline foram removidas deste dispositivo"
                    )),
                    Err(error) => self.show_toast(&error),
                }

                self.refresh_browser();
                let initial = self.config.borrow().clone();
                self.settings_page
                    .rebuild(&initial, self._theme.noctalia_shell_detected());
            }
        }
    }

    pub(crate) fn show_youtube_settings_dialog(self: &Rc<Self>) {
        dialogs::present_youtube_settings(&self.window, self.youtube_page.root());
    }

    pub(crate) fn show_onboarding_wizard(self: &Rc<Self>) {
        let initial = self.config.borrow().clone();
        let language = initial.language;
        let noctalia_available = self._theme.noctalia_shell_detected();
        let weak = Rc::downgrade(self);

        onboarding::present(
            &self.window,
            language,
            &initial,
            noctalia_available,
            move |choices| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };

                let choose_local_folder = {
                    let mut config = controller.config.borrow_mut();
                    config.startup_source = Some(choices.startup_source);
                    config.show_personalized_home_history = choices.show_personalized_home_history;
                    config.blur_mode = choices.blur_mode;
                    config.blur_opacity = choices.blur_opacity;
                    config.footer_mode = choices.footer_mode;
                    config.visual_theme = choices.visual_theme;
                    config.noctalia_theme_sync = noctalia_available && choices.noctalia_theme_sync;
                    config.onboarding_completed = true;

                    choices.startup_source == StartupSource::Local
                        && config.music_directory.is_none()
                };

                controller.save_config();
                controller.apply_home_preferences();
                controller.apply_footer_mode();
                controller.apply_startup_source();

                if choose_local_folder {
                    let controller = controller.clone();
                    glib::idle_add_local_once(move || {
                        controller.choose_library_folder();
                    });
                }
            },
        );
    }

    pub(crate) fn show_startup_source_dialog(self: &Rc<Self>, first_run: bool) {
        let language = self.config.borrow().language;
        let weak = Rc::downgrade(self);

        dialogs::present_startup_source(&self.window, language, first_run, move |source| {
            if let Some(controller) = weak.upgrade() {
                controller.set_startup_source(source);
            }
        });
    }
}

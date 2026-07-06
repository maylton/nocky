//! Application action wiring for `AppController`.

use super::AppController;
use crate::i18n::Message;
use adw::prelude::*;
use gtk::gio;
use std::rc::Rc;

impl AppController {
    pub(crate) fn install_actions(self: &Rc<Self>, app: &adw::Application) {
        let choose = gio::SimpleAction::new("choose-library", None);
        {
            let weak = Rc::downgrade(self);
            choose.connect_activate(move |_, _| {
                if let Some(controller) = weak.upgrade() {
                    controller.choose_library_folder();
                }
            });
        }
        app.add_action(&choose);

        let rescan = gio::SimpleAction::new("rescan", None);
        {
            let weak = Rc::downgrade(self);
            rescan.connect_activate(move |_, _| {
                if let Some(controller) = weak.upgrade() {
                    controller.scan_library();
                }
            });
        }
        app.add_action(&rescan);

        let download = gio::SimpleAction::new("download-lyrics", None);
        {
            let weak = Rc::downgrade(self);
            download.connect_activate(move |_, _| {
                if let Some(controller) = weak.upgrade() {
                    if let Some(item) = controller
                        .youtube_state
                        .borrow()
                        .as_ref()
                        .map(|state| state.item.clone())
                    {
                        controller.set_lyrics_message(
                            "Searching synchronized lyrics for this YouTube track…",
                        );
                        controller.request_youtube_lyrics(&item, true);
                        return;
                    }
                    let current = controller.state.borrow().current;
                    if let Some(index) = current {
                        controller.request_lyrics(index, true, true);
                    } else {
                        controller.show_toast("Selecione uma faixa primeiro");
                    }
                }
            });
        }
        app.add_action(&download);

        let toggle_auto = gio::SimpleAction::new("toggle-auto-lyrics", None);
        {
            let weak = Rc::downgrade(self);
            toggle_auto.connect_activate(move |_, _| {
                if let Some(controller) = weak.upgrade() {
                    let enabled = {
                        let mut config = controller.config.borrow_mut();
                        config.auto_download_lyrics = !config.auto_download_lyrics;
                        config.auto_download_lyrics
                    };
                    controller.save_config();
                    controller.show_toast(if enabled {
                        controller.tr(Message::AutomaticLyricsEnabled)
                    } else {
                        controller.tr(Message::AutomaticLyricsDisabled)
                    });
                    if enabled {
                        if let Some(item) = controller
                            .youtube_state
                            .borrow()
                            .as_ref()
                            .map(|state| state.item.clone())
                        {
                            controller.request_youtube_lyrics(&item, false);
                        } else if let Some(index) = controller.state.borrow().current {
                            controller.request_lyrics(index, false, false);
                        }
                    }
                }
            });
        }
        app.add_action(&toggle_auto);

        let focus_search = gio::SimpleAction::new("focus-search", None);
        {
            let weak = Rc::downgrade(self);
            focus_search.connect_activate(move |_, _| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                controller.close_settings_page();
                controller.search_button.set_active(true);
                controller.search_entry.grab_focus();
            });
        }
        app.add_action(&focus_search);

        self.install_nocky_connect_action(app);

        let settings = gio::SimpleAction::new("settings", None);
        {
            let weak = Rc::downgrade(self);
            settings.connect_activate(move |_, _| {
                if let Some(controller) = weak.upgrade() {
                    controller.open_settings_page();
                }
            });
        }
        app.add_action(&settings);

        let shortcuts = gio::SimpleAction::new("shortcuts", None);
        {
            let weak = Rc::downgrade(self);
            shortcuts.connect_activate(move |_, _| {
                if let Some(controller) = weak.upgrade() {
                    controller.show_shortcuts_window();
                }
            });
        }
        app.add_action(&shortcuts);

        let about = gio::SimpleAction::new("about", None);
        {
            let weak = Rc::downgrade(self);
            about.connect_activate(move |_, _| {
                if let Some(controller) = weak.upgrade() {
                    controller.show_about_window();
                }
            });
        }
        app.add_action(&about);

        let quit = gio::SimpleAction::new("quit", None);
        {
            let app = app.clone();
            quit.connect_activate(move |_, _| app.quit());
        }
        app.add_action(&quit);

        app.set_accels_for_action("app.focus-search", &["<Primary>F"]);
        app.set_accels_for_action("app.settings", &["<Primary>comma"]);
        app.set_accels_for_action("app.choose-library", &["<Primary>O"]);
        app.set_accels_for_action("app.rescan", &["F5"]);
        app.set_accels_for_action("app.download-lyrics", &["<Primary>L"]);
        app.set_accels_for_action("app.quit", &["<Primary>Q"]);
    }
}

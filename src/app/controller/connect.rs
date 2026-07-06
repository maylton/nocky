//! Controller surface for the desktop Nocky Connect entry point.
//!
//! LAN discovery, device picking and accept/deny confirmation will live here.
//! For now this module owns the placeholder action used by the footer button,
//! keeping the regular action table and footer UI small.

use super::AppController;
use adw::prelude::*;
use gtk::gio;
use std::rc::Rc;

impl AppController {
    pub(crate) fn install_nocky_connect_action(self: &Rc<Self>, app: &adw::Application) {
        let connect = gio::SimpleAction::new("nocky-connect", None);
        {
            let weak = Rc::downgrade(self);
            connect.connect_activate(move |_, _| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                controller.open_nocky_connect_surface();
            });
        }
        app.add_action(&connect);
    }

    pub(crate) fn open_nocky_connect_surface(&self) {
        self.persist_playback_session_now();
        self.show_toast("Nocky Connect: descoberta local será adicionada no próximo passo");
    }
}

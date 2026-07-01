//! GTK signal and timer callbacks for `AppController`.

#[path = "callbacks/home_density.rs"]
mod home_density;

use super::AppController;
use gtk::{glib, prelude::*};
use std::{cell::RefCell, rc::Rc, time::Duration};

impl AppController {
    pub(crate) fn setup_callbacks(self: &Rc<Self>) {
        self.mpris.send(crate::playback::mpris::MprisUpdate::Volume(
            self.volume.value(),
        ));
        self.mpris.send(crate::playback::mpris::MprisUpdate::Loop(
            self.repeat_button.is_active(),
        ));
        self.mpris
            .send(crate::playback::mpris::MprisUpdate::Shuffle(
                self.shuffle_button.is_active(),
            ));
        self.publish_mpris_capabilities();
        home_density::install(self.browser.root());

        {
            self.window
                .connect_close_request(move |_| glib::Propagation::Proceed);
        }

        {
            let weak = Rc::downgrade(self);
            let pending_save = Rc::new(RefCell::new(None::<glib::SourceId>));
            self.volume.connect_value_changed(move |scale| {
                if let Some(controller) = weak.upgrade() {
                    let value = scale.value().clamp(0.0, 1.0);
                    controller.player.set_volume(value);
                    controller.config.borrow_mut().volume = value;
                    if value > 0.001 {
                        controller.volume_before_mute.set(value);
                    }
                    controller.apply_volume_icon();
                    controller
                        .mpris
                        .send(crate::playback::mpris::MprisUpdate::Volume(value));

                    if let Some(source) = pending_save.borrow_mut().take() {
                        source.remove();
                    }
                    let weak = weak.clone();
                    let pending = pending_save.clone();
                    let source =
                        glib::timeout_add_local_once(Duration::from_millis(350), move || {
                            pending.borrow_mut().take();
                            if let Some(controller) = weak.upgrade() {
                                controller.save_config();
                            }
                        });
                    pending_save.borrow_mut().replace(source);
                }
            });
        }

        {
            let weak = Rc::downgrade(self);
            self.progress.connect_value_changed(move |scale| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                if controller.updating_progress.get() || !controller.player.is_seekable() {
                    return;
                }
                let duration = controller.player.duration_us();
                if duration > 0 {
                    controller.seek_to((scale.value() * duration as f64) as i64, true);
                }
            });
        }

        {
            let weak = Rc::downgrade(self);
            self.footer_traditional_progress
                .connect_value_changed(move |scale| {
                    let Some(controller) = weak.upgrade() else {
                        return;
                    };
                    if controller.updating_progress.get() || !controller.player.is_seekable() {
                        return;
                    }
                    let duration = controller.player.duration_us();
                    if duration > 0 {
                        controller.seek_to((scale.value() * duration as f64) as i64, true);
                    }
                });
        }

        {
            let weak = Rc::downgrade(self);
            let mut progress_ticks = 0_u8;
            glib::timeout_add_local(Duration::from_millis(50), move || {
                let Some(controller) = weak.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                controller.handle_background_messages();
                controller.handle_youtube_playlist_metadata_updates();
                controller.handle_browser_events();
                controller.poll_current_youtube_playlist_metadata();
                controller.handle_youtube_events();
                controller.handle_settings_events();
                controller.handle_mpris_commands();
                controller.handle_playback_events();

                progress_ticks = progress_ticks.wrapping_add(1);
                let cadence = if controller.player.is_playing() {
                    2
                } else {
                    10
                };
                if progress_ticks.is_multiple_of(cadence) {
                    controller.refresh_progress();
                }
                glib::ControlFlow::Continue
            });
        }

        {
            let weak = Rc::downgrade(self);
            glib::timeout_add_local(Duration::from_secs(10 * 60), move || {
                let Some(controller) = weak.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                if controller.config.borrow().youtube_auto_sync
                    && controller.youtube_library.borrow().connected
                {
                    let _ = controller.sync_youtube_library(true, false);
                }
                glib::ControlFlow::Continue
            });
        }
    }
}

//! Application startup and process entry orchestration.

use super::controller::build_application;
use crate::APP_ID;
use adw::prelude::*;
use gtk::glib;

pub fn run() -> glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_application);
    app.run()
}

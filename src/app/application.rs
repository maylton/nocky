//! Application startup and process entry orchestration.

use super::{controller::build_application, perf};
use crate::APP_ID;
use adw::prelude::*;
use gtk::glib;

pub fn run() -> glib::ExitCode {
    let _timer = perf::PerfTimer::start("app.run");
    perf::log_event("app.start", &[]);

    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(|app| {
        let _timer = perf::PerfTimer::start("app.activate");
        build_application(app);
    });
    app.run()
}

mod app;
mod artist_index;
mod background;
mod browser;
mod config;
pub mod connect;
mod dialogs;
mod i18n;
mod integrations;
mod library;
mod listening_history;
mod local_mix_cover;
mod lyrics;
mod material_palette;
mod md3_volume;
mod mode_toggle;
mod model;
mod offline_store;
mod onboarding;
pub mod playback;
mod reveal_bounce;
mod search_text;
mod theme;
mod theme_css;
mod ui;
mod visual_theme;
mod visualizer;
mod youtube;

use gtk::glib;

const APP_ID: &str = "io.github.maylton.Nocky";
const HOME_PLAYER_WIDTH: i32 = 454;

fn main() -> glib::ExitCode {
    app::run()
}

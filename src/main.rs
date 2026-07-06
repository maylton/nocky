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

use std::{env, ffi::OsStr, fs, path::Path};

use gtk::glib;

const APP_ID: &str = "io.github.maylton.Nocky";
const HOME_PLAYER_WIDTH: i32 = 454;

fn main() -> glib::ExitCode {
    let mut args = env::args_os();
    let _program = args.next();
    let command = args.next();

    if command.as_deref() == Some(OsStr::new("--nocky-connect-inspect")) {
        return match args.next() {
            Some(path) => match inspect_nocky_connect_snapshot(Path::new(&path)) {
                Ok(()) => glib::ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("Nocky Connect inspect failed: {error}");
                    glib::ExitCode::FAILURE
                }
            },
            None => {
                eprintln!("Usage: nocky --nocky-connect-inspect <snapshot.json>");
                glib::ExitCode::FAILURE
            }
        };
    }

    app::run()
}

fn inspect_nocky_connect_snapshot(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let payload = fs::read_to_string(path)?;
    let gateway = connect::NockyConnectGateway::new("desktop-dev-inspector");
    let snapshot = gateway.decode_snapshot(&payload)?;
    let restored = gateway.prepare_restore(&payload)?;
    let item_count = snapshot.queue.items.len();
    let current_index = if item_count == 0 {
        0
    } else {
        snapshot.queue.current_index.min(item_count.saturating_sub(1))
    };

    println!("Nocky Connect snapshot OK");
    println!("  file: {}", path.display());
    println!("  schema: {} v{}", snapshot.schema, snapshot.schema_version);
    println!("  session_id: {}", snapshot.session_id);
    println!("  revision: {}", snapshot.revision);
    println!("  origin_device_id: {}", snapshot.origin_device_id);
    println!("  source: {:?}", snapshot.source);
    println!("  playback_state: {:?}", snapshot.playback.state);
    println!("  position_ms: {}", snapshot.playback.position_ms);
    println!("  duration_ms: {:?}", snapshot.playback.duration_ms);
    println!("  repeat_mode: {:?}", snapshot.queue.repeat_mode);
    println!("  shuffle_enabled: {}", snapshot.queue.shuffle_enabled);
    println!("  queue_title: {:?}", snapshot.queue.title);
    println!("  queue_items: {}", item_count);
    println!("  current_index: {}", current_index);

    if let Some(item) = snapshot.queue.items.get(current_index) {
        let artists = item
            .artists
            .iter()
            .map(|artist| artist.name.trim())
            .filter(|name| !name.is_empty())
            .collect::<Vec<_>>()
            .join(", ");

        println!("  current_item:");
        println!("    title: {}", item.title);
        println!(
            "    artists: {}",
            if artists.is_empty() {
                "Unknown artist"
            } else {
                &artists
            }
        );
        println!("    provider: {}", item.provider);
        println!("    playable_id: {}", item.playable_id);
        println!("    queue_item_id: {}", item.queue_item_id);
    }

    println!("Restore plan");
    println!("  rebuilt_queue_items: {}", restored.queue.len());
    println!("  rebuilt_current_index: {:?}", restored.queue.current_index());
    println!("  restored_state: {:?}", restored.state.state);
    println!("  restored_position_ms: {}", restored.state.position_ms);
    println!("  restored_repeat_mode: {:?}", restored.state.repeat_mode);
    println!("  restored_shuffle_enabled: {}", restored.state.shuffle_enabled);
    println!("  autoplay: false");

    Ok(())
}

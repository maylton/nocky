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

use std::{
    env,
    ffi::OsStr,
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

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

    if command.as_deref() == Some(OsStr::new("--nocky-connect-restore")) {
        return match args.next() {
            Some(path) => match restore_nocky_connect_snapshot(Path::new(&path)) {
                Ok(summary) => {
                    println!("Nocky Connect restore staged");
                    println!("  source: {:?}", summary.source);
                    println!("  queue_items: {}", summary.queue_items);
                    println!("  current_index: {}", summary.current_index);
                    println!("  current_title: {}", summary.current_title);
                    println!("  position_ms: {}", summary.position_ms);
                    println!("  autoplay: false");
                    println!("Run `cargo run` to open Nocky with the restored snapshot.");
                    glib::ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("Nocky Connect restore failed: {error}");
                    glib::ExitCode::FAILURE
                }
            },
            None => {
                eprintln!("Usage: nocky --nocky-connect-restore <snapshot.json>");
                glib::ExitCode::FAILURE
            }
        };
    }

    if command.as_deref() == Some(OsStr::new("--nocky-connect-export")) {
        return match args.next() {
            Some(path) => match export_nocky_connect_snapshot(Path::new(&path)) {
                Ok(summary) => {
                    println!("Nocky Connect snapshot exported");
                    println!("  file: {}", summary.file.display());
                    println!("  source: {:?}", summary.source);
                    println!("  queue_items: {}", summary.queue_items);
                    println!("  current_index: {}", summary.current_index);
                    println!("  current_title: {}", summary.current_title);
                    println!("  position_ms: {}", summary.position_ms);
                    println!("  state: {:?}", summary.state);
                    glib::ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("Nocky Connect export failed: {error}");
                    glib::ExitCode::FAILURE
                }
            },
            None => {
                eprintln!("Usage: nocky --nocky-connect-export <output.json>");
                glib::ExitCode::FAILURE
            }
        };
    }

    app::run()
}

#[derive(Clone, Debug)]
struct RestoreSummary {
    source: playback::queue::QueueSourceKind,
    queue_items: usize,
    current_index: usize,
    current_title: String,
    position_ms: u64,
}

#[derive(Clone, Debug)]
struct ExportSummary {
    file: std::path::PathBuf,
    source: playback::queue::QueueSourceKind,
    queue_items: usize,
    current_index: usize,
    current_title: String,
    position_ms: u64,
    state: connect::NockyPlaybackState,
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
        snapshot
            .queue
            .current_index
            .min(item_count.saturating_sub(1))
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
    println!(
        "  rebuilt_current_index: {:?}",
        restored.queue.current_index()
    );
    println!("  restored_state: {:?}", restored.state.state);
    println!("  restored_position_ms: {}", restored.state.position_ms);
    println!("  restored_repeat_mode: {:?}", restored.state.repeat_mode);
    println!(
        "  restored_shuffle_enabled: {}",
        restored.state.shuffle_enabled
    );
    println!("  autoplay: false");

    Ok(())
}

fn restore_nocky_connect_snapshot(
    path: &Path,
) -> Result<RestoreSummary, Box<dyn std::error::Error>> {
    let payload = fs::read_to_string(path)?;
    let gateway = connect::NockyConnectGateway::new("desktop-dev-restore");
    let snapshot = gateway.decode_snapshot(&payload)?;
    let restored = gateway.prepare_restore(&payload)?;
    let source = restored.queue.source_kind()?.ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "snapshot queue is empty")
    })?;
    let current = restored.queue.current().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "snapshot queue has no current item",
        )
    })?;

    let mut session = playback::session::PlaybackSession::new(&current.media.source);
    session.position_us = restored
        .state
        .position_ms
        .saturating_mul(1_000)
        .min(i64::MAX as u64) as i64;
    session.was_playing = false;
    session.shuffle_enabled = restored.state.shuffle_enabled;
    session.repeat_enabled = matches!(restored.state.repeat_mode, connect::NockyRepeatMode::One);
    session.context_kind = "nocky-connect".to_string();
    session.context_id = snapshot.session_id.clone();
    session.context_title = snapshot
        .queue
        .title
        .clone()
        .unwrap_or_else(|| "Nocky Connect handoff".to_string());
    session.saved_at_unix = unix_seconds();

    playback::queue::save_for(source, &restored.queue.snapshot())?;
    playback::session::save_for(source, &session)?;

    let mut config = config::AppConfig::load();
    config.startup_source = Some(match source {
        playback::queue::QueueSourceKind::Local => config::StartupSource::Local,
        playback::queue::QueueSourceKind::YouTube => config::StartupSource::YouTube,
    });
    config.onboarding_completed = true;
    config.save()?;

    Ok(RestoreSummary {
        source,
        queue_items: restored.queue.len(),
        current_index: restored.queue.current_index().unwrap_or(0),
        current_title: current.media.title.clone(),
        position_ms: restored.state.position_ms,
    })
}

fn export_nocky_connect_snapshot(path: &Path) -> Result<ExportSummary, Box<dyn std::error::Error>> {
    let config = config::AppConfig::load();
    let source = match config.startup_source {
        Some(config::StartupSource::Local) => playback::queue::QueueSourceKind::Local,
        Some(config::StartupSource::YouTube) => playback::queue::QueueSourceKind::YouTube,
        None => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "startup source is not configured",
            )
            .into())
        }
    };
    let queue = playback::queue::load_for(source).queue;
    let current = queue.current().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "active desktop queue has no current item",
        )
    })?;
    let session = playback::session::load_for(source);
    let state = session
        .as_ref()
        .map(|session| {
            if session.was_playing {
                connect::NockyPlaybackState::Playing
            } else {
                connect::NockyPlaybackState::Paused
            }
        })
        .unwrap_or(connect::NockyPlaybackState::Paused);
    let position_ms = session
        .as_ref()
        .map(|session| session.position_us.max(0) as u64 / 1_000)
        .unwrap_or_default();
    let repeat_mode = session
        .as_ref()
        .filter(|session| session.repeat_enabled)
        .map(|_| connect::NockyRepeatMode::One)
        .unwrap_or(connect::NockyRepeatMode::Off);
    let shuffle_enabled = session
        .as_ref()
        .is_some_and(|session| session.shuffle_enabled);
    let title = session
        .as_ref()
        .and_then(|session| {
            (!session.context_title.trim().is_empty()).then(|| session.context_title.clone())
        })
        .or_else(|| Some("Nocky Desktop handoff".to_string()));
    let device_id = connect::NockyConnectDeviceIdentity::new(connect::default_connect_config_dir())
        .get_or_create()?;
    let gateway = connect::NockyConnectGateway::new(device_id);
    let session_id = format!("desktop-{}", unix_millis());
    let playback_state = connect::DesktopPlaybackState {
        state,
        position_ms,
        volume: Some(config.volume.clamp(0.0, 1.0) as f32),
        repeat_mode,
        shuffle_enabled,
        ..Default::default()
    };
    let payload = gateway.export_snapshot_json(&queue, title, playback_state, session_id, 1)?;

    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, payload)?;

    Ok(ExportSummary {
        file: path.to_path_buf(),
        source,
        queue_items: queue.len(),
        current_index: queue.current_index().unwrap_or(0),
        current_title: current.media.title.clone(),
        position_ms,
        state,
    })
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or_default()
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

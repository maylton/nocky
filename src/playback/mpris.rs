use crate::{config::VisualTheme, integrations::album_aura::AlbumAuraBridge};
use mpris_server::{LoopStatus, Metadata, PlaybackStatus, Player, Time, TrackId};
use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MprisPlayback {
    Playing,
    Paused,
    Stopped,
}

#[derive(Clone, Debug)]
pub struct MprisTrack {
    pub track_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub length_us: i64,
    pub art_url: Option<String>,
    pub url: Option<String>,
}

#[derive(Clone, Debug)]
pub enum MprisUpdate {
    Metadata(MprisTrack),
    ClearMetadata,
    Playback(MprisPlayback),
    Position(i64),
    Seeked(i64),
    Loop(bool),
    Shuffle(bool),
    Volume(f64),
    Capabilities { has_tracks: bool, can_seek: bool },
    VisualTheme(VisualTheme),
    Shutdown,
}

#[derive(Clone, Debug)]
pub enum MprisCommand {
    Ready,
    Error(String),
    Raise,
    Quit,
    Play,
    Pause,
    PlayPause,
    Stop,
    Next,
    Previous,
    Seek(i64),
    SetPosition { track_id: String, position: i64 },
    SetLoop(bool),
    SetShuffle(bool),
    SetVolume(f64),
}

pub struct MprisBridge {
    pub updates: Sender<MprisUpdate>,
    pub commands: Receiver<MprisCommand>,
}

impl MprisBridge {
    pub fn start(initial_volume: f64, initial_visual_theme: VisualTheme) -> Self {
        let (update_tx, update_rx) = mpsc::channel();
        let (command_tx, command_rx) = mpsc::channel();

        thread::Builder::new()
            .name("nocky-mpris".into())
            .spawn(move || run_server(update_rx, command_tx, initial_volume, initial_visual_theme))
            .expect("failed to start the MPRIS thread");

        Self {
            updates: update_tx,
            commands: command_rx,
        }
    }

    pub fn send(&self, update: MprisUpdate) {
        let _ = self.updates.send(update);
    }
}

fn run_server(
    updates: Receiver<MprisUpdate>,
    commands: Sender<MprisCommand>,
    initial_volume: f64,
    initial_visual_theme: VisualTheme,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("Nocky MPRIS runtime error: {error}");
            let _ = commands.send(MprisCommand::Error(error.to_string()));
            return;
        }
    };

    let local = tokio::task::LocalSet::new();
    local.block_on(&runtime, async move {
        let error_sender = commands.clone();
        if let Err(error) = serve(updates, commands, initial_volume, initial_visual_theme).await {
            eprintln!("Nocky MPRIS error: {error}");
            let _ = error_sender.send(MprisCommand::Error(error.to_string()));
        }
    });
}

async fn serve(
    updates: Receiver<MprisUpdate>,
    commands: Sender<MprisCommand>,
    initial_volume: f64,
    initial_visual_theme: VisualTheme,
) -> mpris_server::zbus::Result<()> {
    let initial_metadata = Metadata::builder()
        .trackid(TrackId::NO_TRACK)
        .title("Nocky")
        .artist(["No track selected"])
        .album("Local library")
        .build();

    let player = Player::builder("Nocky")
        .identity("Nocky")
        .desktop_entry("io.github.maylton.Nocky")
        .playback_status(PlaybackStatus::Stopped)
        .loop_status(LoopStatus::None)
        .shuffle(false)
        .metadata(initial_metadata)
        .volume(initial_volume.clamp(0.0, 1.0))
        .can_raise(true)
        .can_quit(true)
        .can_go_next(false)
        .can_go_previous(false)
        .can_play(false)
        .can_pause(false)
        .can_seek(false)
        .can_control(true)
        .build()
        .await?;

    connect_simple(&player, &commands);
    let _ = commands.send(MprisCommand::Ready);

    {
        let tx = commands.clone();
        player.connect_seek(move |_, offset| {
            let _ = tx.send(MprisCommand::Seek(offset.as_micros()));
        });
    }
    {
        let tx = commands.clone();
        player.connect_set_position(move |_, track_id, position| {
            let _ = tx.send(MprisCommand::SetPosition {
                track_id: track_id.as_str().to_string(),
                position: position.as_micros(),
            });
        });
    }
    {
        let tx = commands.clone();
        player.connect_set_loop_status(move |_, status| {
            let _ = tx.send(MprisCommand::SetLoop(status != LoopStatus::None));
        });
    }
    {
        let tx = commands.clone();
        player.connect_set_shuffle(move |_, enabled| {
            let _ = tx.send(MprisCommand::SetShuffle(enabled));
        });
    }
    {
        let tx = commands.clone();
        player.connect_set_volume(move |_, volume| {
            let _ = tx.send(MprisCommand::SetVolume(volume.clamp(0.0, 1.0)));
        });
    }

    tokio::task::spawn_local(player.run());

    let mut album_aura = AlbumAuraBridge::discover(initial_visual_theme);

    loop {
        let mut shutdown = false;
        while let Ok(update) = updates.try_recv() {
            if matches!(update, MprisUpdate::Shutdown) {
                album_aura.shutdown();
                shutdown = true;
                break;
            }
            album_aura.apply_mpris_update(&update);
            apply_update(&player, update).await?;
        }

        if shutdown {
            break;
        }
        tokio::time::sleep(Duration::from_millis(40)).await;
    }

    Ok(())
}

fn connect_simple(player: &Player, commands: &Sender<MprisCommand>) {
    macro_rules! connect {
        ($method:ident, $variant:expr) => {{
            let tx = commands.clone();
            player.$method(move |_| {
                let _ = tx.send($variant);
            });
        }};
    }

    connect!(connect_raise, MprisCommand::Raise);
    connect!(connect_quit, MprisCommand::Quit);
    connect!(connect_play, MprisCommand::Play);
    connect!(connect_pause, MprisCommand::Pause);
    connect!(connect_play_pause, MprisCommand::PlayPause);
    connect!(connect_stop, MprisCommand::Stop);
    connect!(connect_next, MprisCommand::Next);
    connect!(connect_previous, MprisCommand::Previous);
}

async fn apply_update(player: &Player, update: MprisUpdate) -> mpris_server::zbus::Result<()> {
    match update {
        MprisUpdate::Metadata(track) => {
            let track_id = TrackId::try_from(track.track_id.as_str()).unwrap_or(TrackId::NO_TRACK);
            let mut builder = Metadata::builder()
                .trackid(track_id)
                .title(track.title)
                .artist([track.artist])
                .album(track.album)
                .length(Time::from_micros(track.length_us.max(0)));

            if let Some(art_url) = track.art_url {
                builder = builder.art_url(art_url);
            }
            if let Some(url) = track.url {
                builder = builder.url(url);
            }

            player.set_metadata(builder.build()).await?;
            player.set_position(Time::ZERO);
        }
        MprisUpdate::ClearMetadata => {
            let metadata = Metadata::builder()
                .trackid(TrackId::NO_TRACK)
                .title("Nocky")
                .artist(["No track selected"])
                .album("Local library")
                .build();
            player.set_metadata(metadata).await?;
            player.set_position(Time::ZERO);
        }
        MprisUpdate::Playback(status) => {
            let status = match status {
                MprisPlayback::Playing => PlaybackStatus::Playing,
                MprisPlayback::Paused => PlaybackStatus::Paused,
                MprisPlayback::Stopped => PlaybackStatus::Stopped,
            };
            player.set_playback_status(status).await?;
        }
        MprisUpdate::Position(position) => {
            player.set_position(Time::from_micros(position.max(0)));
        }
        MprisUpdate::Seeked(position) => {
            let position = Time::from_micros(position.max(0));
            player.set_position(position);
            player.seeked(position).await?;
        }
        MprisUpdate::Loop(enabled) => {
            player
                .set_loop_status(if enabled {
                    LoopStatus::Track
                } else {
                    LoopStatus::None
                })
                .await?;
        }
        MprisUpdate::Shuffle(enabled) => player.set_shuffle(enabled).await?,
        MprisUpdate::Volume(volume) => player.set_volume(volume.clamp(0.0, 1.0)).await?,
        MprisUpdate::Capabilities {
            has_tracks,
            can_seek,
        } => {
            player.set_can_go_next(has_tracks).await?;
            player.set_can_go_previous(has_tracks).await?;
            player.set_can_play(has_tracks).await?;
            player.set_can_pause(has_tracks).await?;
            player.set_can_seek(has_tracks && can_seek).await?;
        }
        MprisUpdate::VisualTheme(_) => {}
        MprisUpdate::Shutdown => {}
    }
    Ok(())
}

# Nocky

**Nocky** is a native GTK4/libadwaita music player for Linux. It combines a polished local-library experience with synchronized lyrics, a real-time spectrum visualizer, MPRIS controls, and optional Noctalia color integration.

> **Status:** Nocky 0.1.0 is the first public beta. Core playback is stable, but feedback and bug reports are welcome.

<p align="center">
  <img src="assets/nocky-icon.png" alt="Nocky owl icon" width="180" />
</p>

## Highlights

- Native Rust, GTK4 and libadwaita interface
- Recursive local music-library scanning
- Albums, artists, playlists and liked songs
- Stable track/disc-aware playback queues
- Embedded and sidecar album artwork
- Direct GStreamer playback engine
- Real-time horizontal audio spectrum visualizer
- Five-line synchronized lyrics preview
- Full synchronized `.lrc` lyrics page
- Automatic LRCLIB lookup
- MPRIS support for media keys, `playerctl` and desktop shells
- Optional Noctalia palette integration with live CSS reload

## Supported audio formats

Playback depends on the GStreamer plugins installed on the system. With the common plugin sets, Nocky can play MP3, FLAC, OGG, Opus, M4A/MP4, WAV, AAC and many other formats.

## Quick installation

The installer builds Nocky from source and supports Debian/Ubuntu, Fedora/RHEL-family, openSUSE and Arch-based distributions.

```bash
chmod +x install.sh
./install.sh --install-deps
```

By default, Nocky is installed for the current user under `~/.local`. Use `--system` for `/usr/local`:

```bash
./install.sh --install-deps --system
```

More options:

```bash
./install.sh --help
```

## Run from source

```bash
cargo run
```

Before running, validate the playback plugins:

```bash
./scripts/check-playback.sh
```

## Installed files

A user installation places files in:

```text
~/.local/bin/nocky
~/.local/share/applications/io.github.maylton.Nocky.desktop
~/.local/share/icons/hicolor/*/apps/io.github.maylton.Nocky.png
~/.local/share/metainfo/io.github.maylton.Nocky.metainfo.xml
```

The desktop entry and application ID use the same identifier, `io.github.maylton.Nocky`, so the icon is resolved correctly by Wayland desktops, launchers and task switchers after installation.

## Uninstall

```bash
./uninstall.sh
```

For a system installation:

```bash
./uninstall.sh --system
```

## Configuration

Nocky stores settings at `~/.config/nocky/config.json` and extracted artwork at `~/.cache/nocky/covers/`.

## Lyrics

Nocky loads sidecar `.lrc` files and can automatically search LRCLIB when synchronized lyrics are missing. Downloaded lyrics are saved beside the audio file when the folder is writable.

## MPRIS

The player registers as `org.mpris.MediaPlayer2.Nocky`. Test it with:

```bash
./scripts/check-mpris.sh
```

## Noctalia theme integration

Nocky remains an independent project, but it can follow Noctalia's Material color roles. The app watches `~/.config/nocky/theme.css`. Use `assets/nocky.css.template` as a Noctalia template and set its output to that path.

## Project structure

```text
src/
├── main.rs             GTK UI and application controller
├── browser.rs          albums, artists, playlists and liked-song pages
├── playback.rs         direct GStreamer playback engine
├── visualizer.rs       theme-aware spectrum visualizer
├── mpris.rs            MPRIS service and command bridge
├── model.rs            metadata, artwork and track model
├── library.rs          recursive library scanner
├── lyrics.rs           local LRC parser
├── lyrics_provider.rs  LRCLIB client
├── config.rs           persistent settings and migration
└── theme.rs            GTK/Noctalia theme bridge
```

## Development

```bash
cargo fmt --check
cargo check
./scripts/verify-release.sh
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for contribution guidelines.

## Current beta limitations

- Internet radio is not implemented yet.
- Gapless playback is disabled; the pipeline is reset between tracks for reliability.
- Flatpak packaging is planned but not included in this beta.

## License

Nocky is licensed under the GNU General Public License v3.0 or later. See [LICENSE](LICENSE).

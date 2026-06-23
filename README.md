# Nocky

**Nocky** is a native GTK4/libadwaita music player for Linux. It combines a polished local-library experience with synchronized lyrics, a real-time spectrum visualizer, MPRIS controls, optional Noctalia color integration, and an optional YouTube Music integration.

This app is vibe-coded and any improvement suggestion is welcomed, but don't expect much as it's just a free time hobby project =)

> **Status:** Nocky 0.2.7 is a beta release. It introduces a faithful GTK/Cairo port of Noctalia's compact mirrored spectrum behavior, a zero-warning quality baseline and a synchronized universal installer.

<!-- noctalia-inspiration:start -->
> [!NOTE]
> Nocky is inspired by the visual language, theming system and desktop experience of [Noctalia Shell](https://github.com/noctalia-dev/noctalia).
>
> A heartfelt thank-you goes to the Noctalia developers and contributors for creating and sharing such an inspiring project.
>
> Nocky is an independent, unofficial hobby project. It is not affiliated with, endorsed by, maintained by, or officially connected to the Noctalia project or its development team.
<!-- noctalia-inspiration:end -->
<p align="center">
  <img src="assets/nocky-icon.png" alt="Nocky owl icon" width="180" />
</p>

## Highlights

- Native Rust, GTK4 and libadwaita interface
- Recursive local music-library scanning
- Unified local and YouTube Music albums, artists, playlists and liked songs
- Stable track/disc-aware playback queues
- Embedded and sidecar album artwork
- Direct GStreamer playback engine
- Noctalia-style 60 Hz mirrored audio spectrum visualizer
- Five-line synchronized lyrics preview
- Full synchronized `.lrc` lyrics page
- Automatic LRCLIB lookup
- MPRIS support for media keys, `playerctl` and desktop shells
- Optional Noctalia palette integration with live CSS reload
- Optional YouTube Music catalogue search and automatic account-library synchronization

## YouTube Music integration

- `ytmusicapi` provides catalogue and account-library data;
- `yt-dlp` resolves a temporary audio URL;
- Deno supplies the JavaScript runtime required by current YouTube extraction;
- Nocky's native GStreamer engine plays the stream and keeps MPRIS, visualizer and media controls working.

Public search works without connecting an account. After connecting, Nocky automatically synchronizes saved songs, liked songs and personal playlists into the main library interface on startup. Online albums and artists are grouped from the synchronized catalogue, and a **Sync with Nocky** button is still available for manual refreshes. The last synchronized library is loaded from disk immediately, while a fresh synchronization runs in the background. Nocky also prefetches the next four stream URLs and keeps separate 512 px and 1200 px artwork caches for collection cards and the now-playing view. Read [docs/YOUTUBE_MUSIC.md](docs/YOUTUBE_MUSIC.md) before connecting an account.

No cookies, sessions, `.env` files or personal data from the reference project are included in this repository.

## Supported local audio formats

Playback depends on the GStreamer plugins installed on the system. With the common plugin sets, Nocky can play MP3, FLAC, OGG, Opus, M4A/MP4, WAV, AAC and many other formats.

## Complete installation

The universal source installer supports Debian/Ubuntu, Fedora, openSUSE and Arch-based distributions.

```bash
chmod +x install.sh
./install.sh --install-deps
```

This builds Nocky, installs its desktop integration and icons, creates an isolated Python runtime for YouTube Music, installs pinned `ytmusicapi`/`yt-dlp` dependencies and bundles Deno when it is not already installed.

For local-library-only use:

```bash
./install.sh --install-deps --without-youtube
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

Create the project-local YouTube runtime once, then launch the app:

```bash
./scripts/setup-youtube-runtime.sh
cargo run
```

For local-only development, `cargo run` works without the helper runtime.

Diagnostics:

```bash
./scripts/check-playback.sh
./scripts/check-youtube.sh
./scripts/check-mpris.sh
```

## Installed files

A user installation places files in:

```text
~/.local/bin/nocky
~/.local/share/applications/io.github.maylton.Nocky.desktop
~/.local/share/icons/hicolor/*/apps/io.github.maylton.Nocky.png
~/.local/share/metainfo/io.github.maylton.Nocky.metainfo.xml
~/.local/share/nocky/helpers/nocky_youtube.py
~/.local/share/nocky/runtime/
```

The desktop entry, icon and application ID all use `io.github.maylton.Nocky`, so the icon is resolved correctly by Wayland desktops, launchers and task switchers after installation.

## Configuration and private data

Nocky stores ordinary settings at:

```text
~/.config/nocky/config.json
```

YouTube Music browser-session headers are stored in Secret Service/libsecret when available. The fallback is:

```text
~/.config/nocky/youtube-session.json
```

The fallback file is created with mode `0600`. Stream URLs and cover images are cached below `~/.cache/nocky/youtube/`. Disconnecting the account removes the saved session.

## Uninstall

```bash
./uninstall.sh
```

For a system installation:

```bash
./uninstall.sh --system
```

The uninstaller intentionally preserves user settings, session data and cache. Disconnect the account in Nocky before uninstalling, or remove `~/.config/nocky/youtube-session.json` manually when Secret Service is not available.

## First-run setup

New installations open a five-step setup wizard before the main interface. It covers the initial music source, the experimental YouTube Music integration, window blur, Noctalia palette synchronization, the Material Design 3-inspired wavy progress bar and footer behavior.

Noctalia-specific appearance options are enabled only when Noctalia Shell is detected. Existing users are migrated as already onboarded and are not interrupted after upgrading.

For development testing:

```bash
NOCKY_FORCE_ONBOARDING=1 cargo run
```

## Lyrics

Nocky loads sidecar `.lrc` files and can automatically search LRCLIB when synchronized lyrics are missing. Downloaded lyrics are saved beside local audio files when the folder is writable. Local and YouTube Music tracks can use the five-line inline preview and the full lyrics page. The focused inline line wraps only when it exceeds the available card width.

## MPRIS

The player registers as `org.mpris.MediaPlayer2.Nocky`. Local and YouTube tracks publish title, artist, album, duration, cover and source URL where available.

## Noctalia theme integration

Nocky remains an independent project, but it can follow Noctalia's Material color roles. The app watches `~/.config/nocky/theme.css`. Use `assets/nocky.css.template` as a Noctalia template and set its output to that path.

## Project structure

```text
src/
├── main.rs             GTK UI and application controller
├── youtube.rs          native YouTube Music page and helper bridge
├── browser.rs          unified local/YouTube library browser and queues
├── playback.rs         GStreamer playback and HTTP stream headers
├── visualizer.rs       theme-aware spectrum visualizer
├── mpris.rs            MPRIS service and command bridge
├── model.rs            metadata, artwork and track model
├── library.rs          recursive local-library scanner
├── lyrics.rs           local LRC parser
├── lyrics_provider.rs  LRCLIB client
├── config.rs           persistent settings and migration
└── theme.rs            GTK/Noctalia theme bridge

helpers/
└── nocky_youtube.py    ytmusicapi/yt-dlp sidecar

requirements-youtube.txt  pinned optional Python runtime
```

## Development

```bash
./scripts/quality-gate.sh
./scripts/verify-release.sh
./install.sh --version
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for contribution guidelines.

## Current beta limitations

- YouTube Music uses an unofficial browser-session integration and can require updates when the service changes.
- YouTube stream URLs are temporary, although Nocky automatically refreshes rejected or expired streams.
- YouTube Music account write actions such as liking and unliking are not included yet.
- A minor Home-player layout shift may occur while inline lyrics change from loading to synchronized content.
- Gapless playback is disabled; the pipeline is reset between tracks for reliability.
- Flatpak packaging is planned but not included in this release.

## License

Nocky is licensed under the GNU General Public License v3.0 or later. See [LICENSE](LICENSE).

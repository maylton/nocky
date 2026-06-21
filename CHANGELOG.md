# Changelog

All notable public changes to Nocky are documented here.

## 0.2.4 — 2026-06-21

### Added

- Automatic YouTube Music library synchronization on application startup when a saved session exists
- YouTube Music-inspired library home with horizontal carousels for mixes, albums, artists and playlists

### Improved

- Album and artist pages now use a richer collection header and larger artwork cards
- The installer and AppStream metadata now identify the 0.2.4 build

## 0.2.3 — 2026-06-21

### Added

- Persistent YouTube Music library cache loaded before the background network refresh
- Persistent cache for previously opened online playlist contents
- Automatic prefetch of the next four YouTube tracks in the active queue
- Separate 512 px browser artwork and 1200 px now-playing artwork

### Improved

- YouTube stream URLs and HTTP headers are reused until shortly before expiry
- Stream cache is pruned atomically and limited to the 80 freshest valid entries
- Album, artist and playlist artwork now selects the largest source thumbnail and requests an HD variant
- Artwork cache keys are based on the upgraded image URL and requested size, preventing stale low-resolution files from being reused
- Cover downloads use atomic temporary files and fall back to the original URL when an upgraded variant is unavailable
- Cached library data appears immediately at startup while synchronization continues in the background

## 0.2.2 — 2026-06-21

### Fixed

- YouTube runtime discovery now prefers project, user and installed isolated runtimes before the system Python
- `cargo run` no longer silently selects a Python interpreter without `ytmusicapi`
- Removed a duplicated library field in the YouTube synchronization snapshot
- The installer now verifies `requests`, `ytmusicapi` and `yt-dlp` immediately after setup

### Added

- First-launch choice between the local library and YouTube Music
- Persistent startup-source setting stored in the normal Nocky configuration
- **Settings** menu entry for changing the startup source later
- Project-local `scripts/setup-youtube-runtime.sh` for development builds
- Automatic YouTube runtime installation by default, with `--without-youtube` for local-only installations

## 0.2.1 — 2026-06-21

### Added

- Automatic YouTube Music library synchronization after startup and account connection
- Unified local and online tracks in the main Library and Liked Songs routes
- YouTube Music album and artist cards inside the native collection browser
- Personal YouTube Music playlists alongside editable local playlists
- Online album, artist and playlist queues with correct next/previous playback
- Cached collection artwork and clear source badges for local and online content
- Manual **Sync with Nocky** action on the YouTube Music page

### Changed

- The empty-state logic now opens the main library when only online content is available
- Playlist controls explicitly distinguish local editable playlists from synchronized online playlists
- Local liked-song icons now use hearts consistently with the player terminology

## 0.2.0 — 2026-06-21

### Added

- Dedicated YouTube Music page in the primary application navigation
- Public catalogue search for songs, videos, albums, artists and playlists
- Optional browser-session connection for account library, liked songs and playlists
- Session storage through Secret Service/libsecret with a protected-file fallback
- `ytmusicapi` catalogue/account helper derived from the author's Nocturne integration
- `yt-dlp` + Deno temporary audio-stream resolution with a short-lived cache
- HTTP request-header forwarding from yt-dlp to GStreamer's network source
- YouTube queue navigation, shuffle, repeat, seek and automatic next-track support
- YouTube track metadata, cover art and URLs in MPRIS
- Isolated YouTube Python runtime in the universal installer
- YouTube-specific diagnostic script and documentation

### Changed

- Universal installer now accepts `--install-youtube`
- Desktop metadata now describes local and YouTube Music playback
- Local-library playback remains independent when YouTube dependencies are absent

### Current limitations

- YouTube Music integration relies on unofficial interfaces and browser-session data
- Streamed-track lyrics and account write actions are not implemented yet
- Temporary stream URLs may need to be resolved again after expiry

## 0.1.0 — 2026-06-21

First official public beta.

### Added

- Native GTK4/libadwaita music-player interface
- Original Nocky owl-and-music visual identity
- Local recursive music-library scanning
- Album and artist browsing
- Persistent playlists and liked songs
- Stable collection-aware playback queues
- Track-number and disc-number album ordering
- Direct GStreamer playback engine
- Real-time theme-aware 32-band spectrum visualizer
- Five-line synchronized lyrics preview and full lyrics page
- Automatic LRCLIB lookup and sidecar `.lrc` support
- Embedded and sidecar album-art support
- MPRIS integration for media keys, `playerctl` and compatible shells
- Optional Noctalia palette integration
- Cross-distribution source installer and uninstaller
- GitHub Actions build validation

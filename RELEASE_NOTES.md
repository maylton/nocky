# Nocky 0.2.4 Beta — Automatic Sync and Library Carousels

Nocky 0.2.4 refreshes the library experience with YouTube Music-inspired carousels and starts the connected YouTube Music sync automatically when the app opens.

## Highlights

- The synchronized YouTube Music library is cached on disk and appears immediately at startup
- Saved YouTube Music sessions sync automatically on application launch
- The main library now opens to horizontal carousels for mixes, albums, artists and playlists
- Album and artist pages use larger artwork cards and a richer collection header
- Previously opened online playlists are available from the cache
- Temporary stream URLs and required HTTP headers are reused while valid
- The next four tracks in the active queue are resolved in advance
- Album, artist and playlist artwork uses the largest available thumbnail
- Collection cards use 512 px artwork, while the main player uses a 1200 px version
- Artwork is cached by final URL and size, avoiding stale low-resolution covers
- Cache writes are atomic and expired stream entries are automatically pruned

## Cache locations

```text
~/.cache/nocky/youtube/library-cache.json
~/.cache/nocky/youtube/stream-cache.json
~/.cache/nocky/youtube/covers/
```

Disconnecting the YouTube Music account clears the synchronized library cache. Ordinary uninstall operations continue preserving user data by default.

## Install

```bash
./install.sh --install-deps
```

For development:

```bash
./scripts/setup-youtube-runtime.sh
cargo run
```

## Suggested tag

```text
v0.2.4-beta
```

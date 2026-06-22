# Nocky 0.2.5 Beta — Faster browsing and distinct source modes

Nocky 0.2.5 focuses on responsiveness and makes the saved source a strict operating mode.

## Highlights

- Local mode is now fully offline and renders only local tracks, albums, artists, playlists and liked songs
- YouTube Music mode renders only synchronized online content
- No automatic YouTube request is started while local mode is active
- Search waits briefly for typing to pause before rebuilding the current page
- Online playlist contents are fetched only when opened and then reused from the persistent cache
- The previous eager preloading of up to 24 complete playlists has been removed
- Switching modes clears incompatible playback state and returns to the correct Home
- Documentation now explains cache behavior, startup behavior and the two distinct modes

## Suggested tag

```text
v0.2.5-beta
```

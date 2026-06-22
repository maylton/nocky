# Performance architecture

Nocky 0.2.5 reduces unnecessary network, process and GTK widget work.

## Fast startup

- Local mode performs no YouTube account-status or synchronization request.
- YouTube mode renders `library-cache.json` immediately and refreshes it in the background only when automatic synchronization is enabled.
- Album artwork and playlist contents remain disk-backed.

## Lazy playlists

Nocky no longer downloads the contents of many playlists during startup. A playlist is fetched when it is opened for the first time. Its tracks are then stored in the existing YouTube library cache and reused on later opens.

## Search debounce

Search text updates immediately, but page reconstruction waits 180 ms after the last keystroke. This prevents album, artist and Home widgets from being destroyed and recreated for every character typed.

## Source isolation

The browser receives only the active source:

- Local mode receives local tracks/configuration and an empty YouTube snapshot.
- YouTube mode receives the synchronized YouTube snapshot and an empty local track collection.

This both fixes mixed-source results and reduces collection grouping, sorting and widget creation.

## Existing caches

```text
~/.cache/nocky/youtube/library-cache.json
~/.cache/nocky/youtube/stream-cache.json
~/.cache/nocky/youtube/covers/
```

The stream cache retains valid temporary URLs, while the cover cache separates browser and now-playing image sizes.

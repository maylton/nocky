# Nocky Connect session sync protocol

Nocky Connect is the shared playback handoff layer between Nocky Desktop and Nocky Android.

The goal is to let a user continue playback instantly between Linux and Android while preserving the current track, queue order, playback position, shuffle/repeat state and enough metadata for the destination app to rebuild its own native playback session.

This document is the source of truth for both repositories:

- Desktop: `maylton/nocky`
- Android: `maylton/nocky-android`

## Design principles

1. **Native playback stays native.** Desktop keeps using GStreamer and Android keeps using Media3/ExoPlayer. Nocky Connect never streams audio from one device to another.
2. **Transfer intent, not media bytes.** A handoff sends a portable playback session snapshot. The receiving app resolves the playable stream or local file through its own engine.
3. **Fast path first.** YouTube Music tracks should transfer by stable IDs such as `videoId`, `setVideoId`, playlist/browse IDs and queue metadata. The destination resolves its own URL.
4. **Local files are opt-in and best-effort.** A local Linux file cannot be assumed to exist on Android. Local handoff requires a shared library identity, future file sync, or a graceful unsupported state.
5. **No account/session leakage.** Cookies, browser headers, OAuth/session tokens, `ytmusicapi` headers and Android InnerTube auth data must never be sent between devices.
6. **Conflict-safe.** When both devices are active, newer state wins only when it belongs to the same session generation and has a newer monotonic revision.
7. **Provider-agnostic shape.** The schema must support current `local` and `youtube` sources, and future `apple_music` or other sources without mixing their queues.

## Transport model

The first implementation should use local network pairing.

Recommended MVP:

- Discovery: manual pairing code or QR code containing the peer address and one-time pairing token.
- Transport: local HTTP for snapshot fetch/push plus WebSocket or Server-Sent Events for live state updates.
- Encryption/authentication: per-pair shared secret generated during pairing. Every request includes an HMAC over the body and timestamp.
- Scope: LAN only. No cloud account is required for the first version.

Future transports can be added behind the same protocol:

- mDNS/Avahi discovery on Linux.
- Android Network Service Discovery.
- Optional relay/cloud sync later.
- USB debugging/development transport for testing.

## Device model

Each installed app instance owns a stable device ID stored locally.

```json
{
  "device_id": "desktop-8f6a7c2b",
  "device_name": "Noctalia Linux",
  "platform": "linux",
  "app": "nocky-desktop",
  "app_version": "0.6.0",
  "protocol_version": 1,
  "capabilities": [
    "youtube.playback",
    "youtube.queue",
    "position.resume",
    "queue.reorder",
    "shuffle.repeat",
    "receive.handoff",
    "send.handoff"
  ]
}
```

Android should publish equivalent metadata with `platform = "android"` and `app = "nocky-android"`.

## Session snapshot

The core protocol object is `PlaybackSessionSnapshot`.

```json
{
  "schema": "io.github.maylton.nocky.connect.PlaybackSessionSnapshot",
  "schema_version": 1,
  "session_id": "01JZ4R1S2M6A7VZK4X5Y9Z0T3P",
  "revision": 42,
  "origin_device_id": "desktop-8f6a7c2b",
  "updated_at_epoch_ms": 1783283000000,
  "updated_at_monotonic_ms": 9283123,
  "source": "youtube",
  "playback": {
    "state": "paused",
    "position_ms": 92345,
    "duration_ms": 214000,
    "rate": 1.0,
    "volume": 0.82,
    "muted": false
  },
  "queue": {
    "title": "Liked songs",
    "current_index": 3,
    "repeat_mode": "all",
    "shuffle_enabled": false,
    "shuffle_seed": null,
    "items": []
  }
}
```

### Playback states

Allowed values:

- `idle`
- `loading`
- `playing`
- `paused`
- `ended`
- `error`

### Repeat modes

Allowed values:

- `off`
- `one`
- `all`

## Queue items

A queue item must be portable. It should contain enough metadata for immediate UI rendering and enough provider IDs for playback reconstruction.

```json
{
  "queue_item_id": "youtube:video:abc123",
  "source": "youtube",
  "provider": "youtube_music",
  "playable_id": "abc123",
  "set_video_id": "def456",
  "playlist_id": "PLxxxx",
  "browse_id": null,
  "title": "Song title",
  "artists": [
    {
      "id": "UCartist",
      "name": "Artist Name"
    }
  ],
  "album": {
    "id": "MPREb_album",
    "title": "Album title"
  },
  "duration_ms": 214000,
  "thumbnail_url": "https://...",
  "explicit": false,
  "is_video": false,
  "is_episode": false,
  "local": null
}
```

For local tracks:

```json
{
  "queue_item_id": "local:sha256:...",
  "source": "local",
  "provider": "nocky_local",
  "playable_id": "sha256:...",
  "title": "Local song",
  "artists": [{ "id": null, "name": "Artist" }],
  "album": { "id": null, "title": "Album" },
  "duration_ms": 180000,
  "thumbnail_url": null,
  "local": {
    "library_id": "desktop-main-library",
    "content_hash": "sha256:...",
    "relative_path": "Artist/Album/01 Song.flac",
    "file_size": 12345678,
    "modified_at_epoch_ms": 1783283000000
  }
}
```

Local tracks should transfer only when the destination can resolve them. If it cannot, the destination displays the queue item as unavailable and offers to continue with the next playable item.

## Handoff flow

### Desktop to Android

1. Desktop exports a snapshot from the active source queue.
2. Android receives the snapshot and validates protocol version, HMAC and capabilities.
3. Android maps each item into its internal `MediaMetadata`/queue representation.
4. Android resolves the current track stream independently.
5. Android seeks to `position_ms`.
6. Android starts paused by default unless the user explicitly chose **Continue on this device now**.
7. Once Android confirms readiness, Desktop pauses or stops according to the chosen handoff mode.

### Android to Desktop

1. Android exports a snapshot from Media3/ExoPlayer and its current queue model.
2. Desktop receives and validates the snapshot.
3. Desktop maps YouTube items into its YouTube queue model.
4. Desktop resolves stream URLs through its own `yt-dlp`/YouTube helper path.
5. Desktop seeks to `position_ms`.
6. Desktop starts paused by default unless the user explicitly chose **Continue on this device now**.
7. Once Desktop confirms readiness, Android pauses.

## Live sync vs explicit handoff

The MVP should implement explicit handoff first.

Explicit handoff:

- User chooses **Continue on Android** or **Continue on Linux**.
- Destination receives one snapshot.
- Source pauses after destination acknowledges readiness.

Live sync can come later:

- Queue mutations propagate continuously.
- Position updates are throttled.
- Device activity conflict resolution becomes required.

## Queue mutation events

After snapshot handoff works, both apps can add incremental events:

```json
{
  "schema": "io.github.maylton.nocky.connect.QueueMutation",
  "schema_version": 1,
  "session_id": "01JZ4R1S2M6A7VZK4X5Y9Z0T3P",
  "revision": 43,
  "origin_device_id": "android-41c2",
  "operation": "move",
  "from_index": 5,
  "to_index": 2
}
```

Operations:

- `replace_all`
- `append`
- `insert`
- `remove`
- `move`
- `set_current_index`
- `set_shuffle`
- `set_repeat`
- `clear`

For the MVP, `replace_all` is enough.

## Position update events

Live position sync should be throttled to avoid noisy traffic.

Recommended rules:

- Send immediately on play, pause, seek, skip, queue replace and app background/foreground transition.
- While playing, send at most once every 5 seconds.
- Do not send every UI progress tick.

```json
{
  "schema": "io.github.maylton.nocky.connect.PositionUpdate",
  "schema_version": 1,
  "session_id": "01JZ4R1S2M6A7VZK4X5Y9Z0T3P",
  "revision": 44,
  "origin_device_id": "desktop-8f6a7c2b",
  "state": "playing",
  "current_index": 3,
  "position_ms": 97345,
  "duration_ms": 214000,
  "updated_at_epoch_ms": 1783283005000
}
```

## Conflict resolution

For MVP explicit handoff, conflict resolution is simple:

- The explicit handoff sender is authoritative for that transfer.
- The receiver creates or replaces its local mirrored session.
- The source pauses only after the receiver acknowledges it is ready.

For live sync:

- Each session has a monotonic `revision`.
- Queue mutations increment revision.
- Position updates do not overwrite queue mutations unless they reference the same `current_index` and same queue revision.
- If both devices mutate queue concurrently, prefer the device where the user performed the most recent foreground action and show a small conflict notice if needed.

## Security and privacy

Never sync:

- YouTube cookies.
- Browser headers.
- OAuth/session tokens.
- Raw stream URLs.
- Secret Service/libsecret data.
- Android account tokens.

Allowed to sync:

- Public YouTube/YouTube Music IDs.
- Track titles, artists, album metadata, duration and artwork URLs.
- Playback position.
- Queue order.
- Shuffle/repeat mode.
- Device display names.

Pairing tokens must be revocable from both apps.

## MVP implementation phases

### Phase 1 — Shared schema and exporters

Desktop:

- Add Rust models under a `connect` module.
- Export the current active queue into `PlaybackSessionSnapshot`.
- Support YouTube queue items first.
- Add tests for JSON shape and source isolation.

Android:

- Add Kotlin serializable models under `connect`.
- Export current `MusicService`/Media3 queue into `PlaybackSessionSnapshot`.
- Map existing `MediaMetadata` to portable queue items.
- Add JVM tests for JSON compatibility.

### Phase 2 — Manual import/export dev path

Desktop:

- Add a debug action to write the current snapshot to a file.
- Add a debug action to load a snapshot from a file and restore a paused queue.

Android:

- Add a debug/developer import route for a snapshot file or pasted JSON.
- Restore queue and seek position in paused state.

This validates the protocol without networking.

### Phase 3 — Local network pairing

- Add device identity.
- Add pairing token generation.
- Add signed local HTTP endpoints.
- Add a device list UI.

### Phase 4 — One-tap handoff

- Add **Continue on Android** on Desktop.
- Add **Continue on Linux** on Android.
- Destination resolves current track first.
- Source pauses after ready acknowledgement.

### Phase 5 — Shared live queue

- Add queue mutation events.
- Add throttled position updates.
- Add conflict resolution.

## Initial acceptance criteria

1. While playing a YouTube Music track on Desktop, export a snapshot.
2. Android imports the snapshot, rebuilds the queue, selects the same current item and seeks to the same position.
3. Android can start playback from that position without needing Desktop audio data.
4. While playing a YouTube Music track on Android, export a snapshot.
5. Desktop imports the snapshot, rebuilds the queue, resolves the current stream independently and seeks to the same position.
6. Shuffle/repeat/current index survive round-trip serialization.
7. Local-only tracks are not silently misrepresented as playable on a device that cannot resolve them.
8. No cookies, raw stream URLs, account headers or private tokens appear in exported snapshots.

## Compatibility notes

Desktop currently stores user settings under `~/.config/nocky/config.json` and YouTube session data outside ordinary settings. Nocky Connect should use a separate file such as:

```text
~/.config/nocky/connect-devices.json
```

Android should use its existing DataStore/Room patterns for device pairing and session metadata, but the shared protocol models should stay independent from UI and persistence.

## Open decisions

- Whether the first LAN server runs only while the app is open or as a background service.
- Whether Android should expose a QR code, scan a QR code, or support both.
- Whether Desktop should use mDNS immediately or start with manual IP/code pairing.
- How much local-library identity should be standardized before enabling local track handoff.
- Whether live sync should be always-on per paired device or only inside an active shared session.

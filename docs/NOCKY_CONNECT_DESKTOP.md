# Nocky Connect desktop snapshot foundation

This document tracks the desktop side of Nocky Connect.

Nocky Connect is designed to let Nocky Desktop and Nocky Android hand off playback without sharing account secrets, browser headers, cookies, raw stream URLs or audio bytes.

## Current scope

This PR adds a desktop-side foundation only:

- shared portable playback session protocol models;
- JSON encode/decode through `serde` and `serde_json`;
- export mapping from the existing `PlaybackQueue` model to `PlaybackSessionSnapshot`;
- restore mapping from a received `PlaybackSessionSnapshot` back to a paused `PlaybackQueue` plan;
- local private snapshot file store;
- local desktop device identity helper;
- gateway for schema/version validation, export and restore planning;
- shared v1 JSON fixture compatibility test;
- unit tests for export, restore, schema validation, device identity and file storage.

The implementation is isolated under `src/connect/` and does not change UI, player controls, GStreamer playback, MPRIS, YouTube stream resolution, local library scanning or queue behavior.

## Protocol compatibility

The top-level snapshot schema is:

```text
io.github.maylton.nocky.connect.PlaybackSessionSnapshot
```

Current schema version:

```text
1
```

The shape is intentionally aligned with the Android fork's `PlaybackSessionSnapshot` model.

## Compatibility fixture

`docs/fixtures/nocky-connect-snapshot-v1.json` is a shared protocol fixture. The desktop gateway test decodes it and prepares a paused restore plan to verify that the Rust implementation remains compatible with the Android-side v1 snapshot contract.

## Device identity

`NockyConnectDeviceIdentity` creates and reuses a random app-local device ID stored under `nocky-connect/device-id` in the provided base directory. `default_connect_config_dir()` resolves to `$XDG_CONFIG_HOME/nocky`, `~/.config/nocky`, or a temporary fallback. The ID is intentionally not based on hardware identifiers.

## Export behavior

The desktop exporter converts a `PlaybackQueue` into a portable queue:

- local queue entries become `source = "local"` and `provider = "nocky_local"`;
- YouTube entries become `source = "youtube"` and `provider = "youtube_music"`;
- the current queue index is preserved;
- position, repeat and shuffle state are provided through `DesktopPlaybackState`;
- local file identity is best-effort and includes path, file size and modification time when available.

## Restore behavior

Restoring a snapshot is conservative:

- the received queue is rebuilt as a desktop `PlaybackQueue`;
- the current index is clamped to the available item range;
- playback state is always prepared as paused;
- position, volume, repeat and shuffle intent are preserved in the restore plan;
- no stream URL, cookie, browser header, token or account data is accepted or required.

## Next steps

1. Wire the gateway to the real desktop playback/session state with explicit export and restore methods.
2. Add a development menu/action for export/import JSON round trips.
3. Verify Android ⇄ Desktop JSON compatibility with manually exchanged snapshots.
4. Add local-network pairing only after manual round trips work both ways.

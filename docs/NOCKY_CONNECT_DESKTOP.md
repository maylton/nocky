# Nocky Connect desktop snapshot foundation

This document tracks the desktop side of Nocky Connect.

Nocky Connect is designed to let Nocky Desktop and Nocky Android hand off playback without sharing account secrets, browser headers, cookies, raw stream URLs or audio bytes.

## Current scope

This PR adds a desktop-side foundation only:

- shared portable playback session protocol models;
- JSON encode/decode through `serde` and `serde_json`;
- export mapping from the existing `PlaybackQueue` model to `PlaybackSessionSnapshot`;
- restore mapping from a received `PlaybackSessionSnapshot` back to a paused `PlaybackQueue` plan;
- development CLI inspection for manually exchanged snapshot JSON files;
- development CLI staged restore for importing a snapshot into the desktop queue/session store;
- device descriptor model for future LAN discovery and capability negotiation;
- local private snapshot file store;
- local desktop device identity helper;
- gateway for schema/version validation, export and restore planning;
- shared v1 JSON fixture compatibility tests for snapshots and device descriptors;
- unit tests for export, restore, schema validation, descriptor validation, device identity and file storage.

The implementation is isolated under `src/connect/` plus temporary command-line development hooks in `src/main.rs`. It does not change normal UI controls, GStreamer playback, MPRIS, YouTube stream resolution, local library scanning or regular queue behavior.

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

## Compatibility fixtures

`docs/fixtures/nocky-connect-snapshot-v1.json` is a shared protocol fixture. The desktop gateway test decodes it and prepares a paused restore plan to verify that the Rust implementation remains compatible with the Android-side v1 snapshot contract.

`docs/fixtures/nocky-connect-device-descriptor-v1.json` is a shared descriptor fixture. The desktop descriptor test decodes it to verify that the Rust implementation remains compatible with the future Android-side LAN discovery descriptor contract.

## Manual Android snapshot inspection

The desktop app binary accepts a temporary development command for validating a JSON snapshot exported by Android:

```bash
cargo run -- --nocky-connect-inspect /path/to/nocky-android-snapshot.json
```

This command does not open the GTK UI. It validates the schema/version, decodes the snapshot, prepares a conservative paused restore plan and prints a summary including session id, source, playback position, queue size, current item and rebuilt desktop queue state.

## Manual Android snapshot staged restore

A second temporary command imports an Android-exported snapshot into the desktop queue/session store:

```bash
cargo run -- --nocky-connect-restore /path/to/nocky-android-snapshot.json
cargo run
```

The restore command validates the snapshot, rebuilds the desktop `PlaybackQueue`, writes the source-specific Queue 2.0 state, writes a paused source-specific `PlaybackSession`, switches the startup source in `config.json` to match the snapshot source and exits. The following regular `cargo run` starts the app through its normal startup path and restores the staged queue paused at the received position.

The command intentionally stages `was_playing = false`, so opening Nocky after import must not unexpectedly start playback.

## Device identity and descriptor

`NockyConnectDeviceIdentity` creates and reuses a random app-local device ID stored under `nocky-connect/device-id` in the provided base directory. `default_connect_config_dir()` resolves to `$XDG_CONFIG_HOME/nocky`, `~/.config/nocky`, or a temporary fallback. The ID is intentionally not based on hardware identifiers.

`NockyConnectDeviceDescriptor` describes a device before any handoff happens. It includes device ID, display name, platform, app name/version, protocol version and supported features.

## LAN-first discovery direction

QR pairing is not planned as the default flow. The intended first real handoff flow is same-network discovery:

1. both apps advertise a `NockyConnectDeviceDescriptor` on the local network;
2. each app shows compatible devices discovered on the same LAN;
3. the receiving device requires an explicit accept/deny confirmation before a restore happens;
4. YouTube Music account state is not exchanged or inspected by Nocky Connect.

Using the same Google/YouTube Music account can make the handoff more likely to resolve the same songs, but it should not be used as a pairing secret. Nocky Connect should transfer public playable IDs and metadata, not account cookies, tokens or headers. A future LAN implementation can still show an optional pairing code or confirmation code if accidental-device protection is needed, but QR is not required for the main flow.

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
- only `repeat_mode = "one"` maps to the current desktop repeat toggle for now;
- no stream URL, cookie, browser header, token or account data is accepted or required.

## Next steps

1. Validate an Android-exported JSON snapshot with `--nocky-connect-inspect`.
2. Stage an Android-exported JSON snapshot with `--nocky-connect-restore`, then start Nocky normally and confirm the paused restore.
3. Replace the staged CLI import with an in-app development action.
4. Verify Android ⇄ Desktop JSON compatibility with manually exchanged snapshots.
5. Add same-network discovery and explicit accept/deny confirmation after manual JSON round trips work both ways.

# Nocky Connect Release Candidate Checklist

This checklist is for validating the current local-network Nocky Connect flow as a usable feature instead of a debug prototype.

## Scope for this RC

This RC covers:

- Desktop -> Android handoff.
- Android -> Desktop handoff.
- Paused-by-default restore.
- Device picker surfaces.
- Recent-device cache behavior.
- Discovery/handoff troubleshooting copy.
- Safer portable metadata/artwork handling.

This RC does not yet cover:

- Trusted-device memory.
- Explicit accept/decline confirmation UI.
- Android always-on presence outside the picker/player lifecycle.
- Persistent device cache across app process restarts.

## Network prerequisites

Both devices must be on the same LAN/Wi-Fi.

On Linux Desktop, the firewall must allow:

```bash
sudo ufw allow in proto udp from 192.168.0.0/24 to any port 34987 comment 'Nocky Connect LAN discovery'
sudo ufw allow in proto tcp from 192.168.0.0/24 to any port 35187 comment 'Nocky Connect handoff HTTP'
sudo ufw reload
```

Use the correct subnet if the LAN is not `192.168.0.0/24`.

## Build checks

Desktop:

```bash
cargo check --all-targets
```

Android:

```bash
./gradlew --no-configuration-cache :app:compileFossDebugKotlin
```

Both checks should pass before manual validation.

## Manual validation: Desktop -> Android

1. Start Desktop Nocky.
2. Start Android Nocky and play or queue a track.
3. Open the Android Nocky Connect sheet so Android becomes visible.
4. On Desktop, open Nocky Connect.
5. Confirm Android appears as available or recently seen.
6. Select Android.
7. Confirm Android receives the queue paused.
8. Confirm title, artist, duration and artwork are reasonable.
9. Confirm no stream URL, cookie, token or request header is present in logs/snapshot payloads.

Expected result:

- Desktop reports delivery success only after Android returns `restored_paused`.
- Android queue changes to the Desktop queue.
- Android playback is paused by default.
- YouTube artwork uses a safe HTTP thumbnail or a `i.ytimg.com` fallback.
- Local desktop-only artwork paths are not treated as portable Android artwork.

## Manual validation: Android -> Desktop

1. Start Desktop Nocky.
2. Start Android Nocky with a non-empty queue.
3. Open Android Nocky Connect.
4. Confirm Desktop appears as available or recently seen.
5. Tap Desktop.
6. Confirm the Android row moves through waiting/delivered or failed state.
7. Confirm Desktop receives the queue paused.
8. Confirm title, artist, duration and artwork are reasonable on Desktop.

Expected result:

- Android shows delivered/restored-paused state after success.
- Desktop applies the received queue paused.
- Desktop does not turn remote HTTP artwork into a local file path.
- Missing YouTube artwork can be reconstructed from the video id.

## Manual validation: cache and failure states

1. Open Nocky Connect on each side and confirm devices are discovered.
2. Close and reopen the picker.
3. Confirm recently seen devices remain visible for the cache window.
4. Temporarily block Desktop TCP `35187` and try Android -> Desktop.
5. Confirm Android shows a clear failed state.
6. Temporarily block Desktop UDP `34987` and scan again.
7. Confirm troubleshooting copy points to Wi-Fi/network/firewall.

Expected result:

- Cached rows remain usable for retry.
- Failed rows do not silently look like success.
- Copy mentions same Wi-Fi and the relevant discovery/handoff ports.

## Metadata/artwork acceptance criteria

Portable snapshots should include safe metadata only:

- title;
- artist names;
- album title/id when available;
- duration;
- playable id;
- set video id when available;
- playlist/browse id when available;
- safe HTTP thumbnail URL or generated YouTube thumbnail fallback.

Portable snapshots must not include:

- account tokens;
- cookies;
- request headers;
- raw stream URLs;
- private local cover paths as cross-device artwork.

## Current release blockers

Before calling this production-ready:

- add receiver accept/decline or trusted-device flow;
- add a clearer Android lifecycle strategy outside the picker;
- decide whether Desktop surface remains popover or becomes a full internal page;
- review/reduce remaining diagnostic logs;
- run final end-to-end validation on a clean install/build.

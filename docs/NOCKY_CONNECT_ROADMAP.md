# Nocky Connect Roadmap

This document records the current state of Nocky Connect and the product direction change from a manual `Send / Receive` diagnostic flow to a Spotify Connect-like device picker.

Nocky Connect is not an implementation of Spotify Connect and must not depend on proprietary Spotify protocols. The intended user experience is similar: devices advertise local presence, the app shows available devices, and the user can move the current playback session safely between them.

## Product direction

The main Nocky Connect surface should become a live `Available devices` surface instead of two separate `Send` and `Receive` actions.

Target experience:

```text
Nocky Connect

This device
✓ Nocky Desktop

Available on your network
📱 samsung SM-S936B
   Android · last seen just now

Troubleshooting
No devices? Check same network and firewall UDP/TCP rules.
```

Selecting a device should start a safe local handoff flow:

1. discover LAN devices;
2. show devices in a list;
3. user selects a target device;
4. sender offers a playback-session snapshot;
5. receiver shows accept/decline, at least until the device is trusted;
6. accepted snapshot is imported paused by default;
7. receiver can show a clear `Play` action.

## Current implementation status

### Shared protocol pieces

- Playback session snapshot schema version 1 exists and is used by both desktop and Android.
- Device descriptor schema version 1 exists and identifies device id, device name, platform, app name, app version, supported features, and optional handoff endpoint.
- LAN discovery schema version 1 exists with:
  - UDP port `34987`;
  - magic `NOCKY_CONNECT_DISCOVERY_V1`;
  - message kinds `hello` and `announce`.
- Handoff message schema version 1 exists with:
  - `handoff_offer`;
  - `handoff_accept`;
  - `handoff_decline`;
  - `handoff_result`.
- Local HTTP handoff currently uses TCP port `35187` and paths:
  - `/nocky-connect/handoff`;
  - `/nocky-connect/snapshot`.
- Discovery and handoff packets intentionally do not contain account tokens, cookies, request headers, stream URLs, or other secrets.

### Validated end-to-end flows

Validated:

- Desktop -> Android:
  - Desktop discovers Android;
  - Desktop sends handoff offer;
  - Android accepts;
  - Desktop transfers snapshot;
  - Android saves pending restore as fallback;
  - Android applies restore automatically;
  - Android player switches to the desktop queue paused.
- Android -> Desktop:
  - Android discovers Desktop;
  - Android sends handoff offer;
  - Desktop accepts;
  - Android transfers snapshot;
  - Desktop applies restore paused.

Known implementation notes:

- Desktop receiver is currently started from the Nocky Connect surface and guarded as a singleton to avoid duplicate binds on TCP `35187`.
- Android snapshot export must read ExoPlayer state on the main thread.
- The Android manual `Apply pending restore` debug action should not be part of the normal user-facing flow; pending restore remains as an internal fallback.
- Restored queues can still have incomplete artwork/metadata. Metadata hydration is a later polish phase.

## Important environment findings

On the tested Linux desktop, UFW was active with default input policy `drop`. UDP broadcast packets from Android were visible in `tcpdump`, but were not delivered to Nocky or to a minimal Python UDP listener until UDP port `34987` was allowed.

Permanent discovery rule used on the desktop:

```bash
sudo ufw allow in proto udp from 192.168.0.0/24 to any port 34987 comment 'Nocky Connect LAN discovery'
sudo ufw reload
```

The reverse Android -> Desktop handoff also requires local TCP access to the desktop receiver:

```bash
sudo ufw allow in proto tcp from 192.168.0.0/24 to any port 35187 comment 'Nocky Connect handoff HTTP'
sudo ufw reload
```

Troubleshooting commands used:

```bash
sudo tcpdump -ni any udp port 34987
sudo tcpdump -ni any tcp port 35187
systemctl is-active ufw
systemctl is-active firewalld
systemctl is-active nftables
sudo nft list ruleset | grep -niE 'hook input|policy|drop|reject|34987|35187'
sudo iptables-save | grep -niE '34987|35187|DROP|REJECT'
```

This firewall finding should be reflected in user-facing troubleshooting copy before release.

## What should be reused

Keep and build on:

- snapshot schema and mappers;
- descriptor schema and persistent device identity;
- discovery envelope format;
- UDP discovery transport;
- HTTP handoff offer/snapshot transport;
- firewall troubleshooting note;
- paused-by-default restore semantics;
- strict no-secrets rule;
- pending restore store as an internal Android fallback.

Use the current `Send` and `Receive` actions only as temporary diagnostics. They should not remain the main UX.

## What should change

### UX

Replace the main `Send / Receive` UX with:

- a live list of devices;
- device row states:
  - scanning;
  - available;
  - connecting;
  - waiting for confirmation;
  - failed / firewall hint;
- current device section;
- troubleshooting footer.

Desktop should eventually move from a popover to an internal Nocky Connect page/surface similar to the Queue surface.

Android should keep the player-menu entry point, but the bottom-sheet page should become a device list instead of two static actions.

### Always-on discovery / presence

Future product behavior should not require opening a temporary receiver action to make a device visible.

Target behavior:

- Desktop starts local presence/listener when the app is running, not only when the Nocky Connect popover opens.
- Android starts local presence/listener when the app/player is active.
- Each side maintains a small live cache of recently seen devices.
- Opening the Nocky Connect surface only renders the already-known device list and can trigger a manual refresh.
- Discovery should be rate-limited and lifecycle-aware, especially on Android, to avoid unnecessary battery/network usage.
- Receiver HTTP should become a background singleton with clear lifecycle ownership.

### Transport

Discovery only finds devices. Actual handoff uses a reliable local transport:

- TCP/local HTTP over LAN;
- explicit offer/accept/decline/result messages;
- clear timeout handling;
- no secrets;
- snapshot summary before transfer;
- restore paused by default.

### Trust model

Initial version:

- receiver always shows accept/decline.

Later version:

- remember trusted device ids;
- allow auto-accept only for trusted devices;
- provide a way to revoke trust.

## Updated roadmap

### Phase 0 - Stabilize bidirectional diagnostic handoff

- Keep temporary desktop UDP logs while transport work continues.
- Confirm clean desktop validation:

```bash
cargo fmt --all
cargo test connect
cargo check --all-targets
```

- Confirm clean Android validation:

```bash
./gradlew --no-configuration-cache :app:testFossDebugUnitTest --tests 'com.metrolist.music.connect.*'
./gradlew --no-configuration-cache :app:compileFossDebugKotlin
```

### Phase 1 - UX cleanup for current handoff

- Hide/remove user-facing debug-only restore actions.
- Improve toast/copy for send, receive, restore, timeout, and firewall failures.
- Keep pending restore as internal fallback.
- Keep manual `Send`/`Receive` only until the live device list exists.

### Phase 2 - Device list model

Create shared concepts on both platforms:

- discovered device model;
- last-seen timestamp;
- source address;
- device status;
- current local device row;
- helper to merge/dedupe discovery results.

The list model should support automatic scan/listen loops without needing separate send/receive buttons.

### Phase 3 - Spotify-style surfaces

Desktop:

- show `This device` and `Available devices`;
- show devices as clickable rows;
- keep troubleshooting text for UFW/UDP `34987` and TCP `35187`;
- later migrate from popover to internal ViewStack page.

Android:

- keep player bottom-sheet entry;
- replace two menu actions with a live device list;
- show empty/scanning/error states;
- show device rows matching the desktop semantics.

### Phase 4 - Always-on discovery and background receiver

- Desktop starts presence/receiver at app startup.
- Android starts presence/receiver when app/player lifecycle allows it.
- Device picker reads from live cache instead of starting discovery from scratch.
- Rate-limit discovery and avoid battery-heavy loops.

### Phase 5 - Trust, confirmation, and polish

- Receiver confirmation surface;
- trusted device ids;
- remember friendly names;
- revoke trust;
- better error copy;
- remove or gate verbose diagnostic logs;
- document firewall setup in README/user docs.

### Phase 6 - Metadata hydration

- Improve restored artwork/thumbnail handling.
- Hydrate YouTube metadata from ids when needed.
- Preserve local metadata where available.
- Avoid treating temporary cache paths as permanent artwork identities.

## Release guardrails

Do not ship Nocky Connect as a secret/session syncing feature. It is a playback-session handoff feature.

Do not send:

- cookies;
- OAuth tokens;
- request headers;
- stream URLs;
- account identifiers beyond non-secret local device metadata.

Do not auto-play imported sessions by default. Restore paused unless the user explicitly chooses to play.

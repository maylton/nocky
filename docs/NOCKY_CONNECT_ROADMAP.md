# Nocky Connect Roadmap

This document records the current state of Nocky Connect and the product direction change from a manual `Send / Receive` flow to a Spotify Connect-like device picker.

Nocky Connect is not an implementation of Spotify Connect and must not depend on proprietary Spotify protocols. The intended user experience is similar: a user opens a device surface, sees available devices on the local network, selects one, and moves the current playback session there.

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
No devices? Check same network and firewall UDP port 34987.
```

Selecting a device should start a safe local handoff flow:

1. discover LAN devices;
2. show devices in a list;
3. user selects a target device;
4. sender offers a playback-session snapshot;
5. receiver shows accept/decline, at least until the device is trusted;
6. accepted snapshot is imported paused by default;
7. receiver can show a clear `Play` action.

## What has been implemented and validated

### Shared protocol pieces

- Playback session snapshot schema version 1 exists and is used by both desktop and Android.
- Device descriptor schema version 1 exists and identifies device id, device name, platform, app name, app version, and supported features.
- LAN discovery schema version 1 exists with:
  - UDP port `34987`;
  - magic `NOCKY_CONNECT_DISCOVERY_V1`;
  - message kinds `hello` and `announce`.
- Discovery packets intentionally do not contain account tokens, cookies, headers, stream URLs, or other secrets.

### Desktop

Validated desktop capabilities:

- exports current persisted playback session snapshot to JSON;
- imports Android snapshot into a paused desktop queue/session;
- validates Android-exported snapshot JSON;
- creates a persistent desktop device id;
- has a footer entry point for Nocky Connect;
- sends LAN discovery `hello` packets;
- receives Android `hello` packets;
- responds with `announce`;
- receives Android `announce` packets;
- deduplicates devices by device id;
- runs discovery off the GTK main thread;
- prints temporary diagnostic logs for UDP discovery.

Manual validation already performed:

- Android snapshot export -> desktop inspect/restore worked.
- Desktop snapshot export -> Android debug import worked.
- Android -> Desktop LAN discovery worked after opening UDP 34987 in UFW.
- Desktop -> Android LAN discovery worked after opening UDP 34987 in UFW.

### Android

Validated Android capabilities:

- exports a playback session snapshot from persisted queue/player state;
- imports a desktop snapshot paused into Android player state;
- creates a persistent Android device id;
- exposes a player menu entry under the player bottom sheet;
- sends LAN discovery `hello` packets;
- receives desktop `hello` packets;
- responds with `announce`;
- receives desktop `announce` packets;
- deduplicates devices by device id.

Manual validation already performed:

- Nocky Connect menu item appears in the Android player menu.
- Android debug export created a real snapshot file.
- Android debug import restored a desktop snapshot.
- Android `Receive from desktop` found the desktop after firewall was fixed.
- Android `Send to desktop` was received by the desktop after firewall was fixed.

## Important environment finding

On the tested Linux desktop, UFW was active with default input policy `drop`. UDP broadcast packets from Android were visible in `tcpdump`, but were not delivered to Nocky or to a minimal Python UDP listener until UDP port `34987` was allowed.

Permanent fix used on the desktop:

```bash
sudo ufw allow in proto udp from 192.168.0.0/24 to any port 34987 comment 'Nocky Connect LAN discovery'
sudo ufw reload
```

Troubleshooting commands used:

```bash
sudo tcpdump -ni any udp port 34987
systemctl is-active ufw
systemctl is-active firewalld
systemctl is-active nftables
sudo nft list ruleset | grep -niE 'hook input|policy|drop|reject|34987'
sudo iptables-save | grep -niE '34987|DROP|REJECT'
```

This firewall finding should be reflected in user-facing troubleshooting copy before release.

## What should be reused

Keep and build on:

- snapshot schema and mappers;
- descriptor schema and persistent device identity;
- discovery envelope format;
- UDP discovery transport;
- firewall troubleshooting note;
- CLI inspect/export/restore tools on desktop;
- Android debug export/import activities while the transport is still evolving;
- paused-by-default restore semantics;
- strict no-secrets rule.

Use the current `Send` and `Receive` actions only as temporary diagnostics. They should not remain the main UX.

## What should change

### UX

Replace the main `Send / Receive` UX with:

- automatic scan/listen when opening Nocky Connect;
- a live list of devices;
- device row states:
  - scanning;
  - available;
  - connecting;
  - waiting for confirmation;
  - failed / firewall hint;
- current device section;
- troubleshooting footer.

Desktop should eventually move from a modal popup to an internal Nocky Connect page/surface similar to the Queue surface.

Android should keep the player-menu entry point, but the bottom-sheet page should become a device list instead of two static actions.

### Transport

Discovery only finds devices. Actual handoff should use a reliable local transport:

- TCP or local HTTP over LAN;
- explicit offer/accept/decline messages;
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

### Phase 0 - Stabilize current discovery diagnostic build

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

### Phase 1 - Device list model

Create shared concepts on both platforms:

- discovered device model;
- last-seen timestamp;
- source address;
- device status;
- current local device row;
- helper to merge/dedupe discovery results.

The list model should support automatic scan/listen loops without needing separate send/receive buttons.

### Phase 2 - Spotify-style surfaces

Desktop:

- show `This device` and `Available devices`;
- auto-start discovery when the surface opens;
- show devices as clickable rows;
- keep troubleshooting text for UFW/UDP 34987;
- later migrate from modal window to internal ViewStack page.

Android:

- keep player bottom-sheet entry;
- replace two menu actions with a live device list;
- show empty/scanning/error states;
- show device rows matching the desktop semantics.

### Phase 3 - Handoff protocol contract

Add pure data models and tests for:

- `handoff_offer`;
- `handoff_accept`;
- `handoff_decline`;
- `handoff_result`;
- snapshot summary fields;
- error codes.

No socket transfer should be added until these contracts are tested on both platforms.

### Phase 4 - Local reliable transfer

Implement a simple reliable local transport:

- receiver opens a local TCP/HTTP listener for handoff offers;
- sender connects to selected target;
- receiver shows accept/decline;
- accepted snapshot is transferred and restored paused;
- sender gets success/failure result.

### Phase 5 - Trust and polish

- trusted device ids;
- remember friendly names;
- revoke trust;
- better error copy;
- remove or gate verbose diagnostic logs;
- document firewall setup in README/user docs.

## Release guardrails

Do not ship Nocky Connect as a secret/session syncing feature. It is a playback-session handoff feature.

Do not send:

- cookies;
- OAuth tokens;
- request headers;
- stream URLs;
- account identifiers beyond non-secret local device metadata.

Do not auto-play imported sessions by default. Restore paused unless the user explicitly chooses to play.

# Nocky Connect Roadmap

This document records the current state of Nocky Connect and the product direction change from a manual `Send / Receive` diagnostic flow to a Spotify Connect-like device picker.

Nocky Connect is not an implementation of Spotify Connect and must not depend on proprietary Spotify protocols. The intended user experience is similar: devices advertise local presence, the app shows available devices, and the user can move the current playback session safely between them.

## Product direction

The main Nocky Connect surface should be a live `Available devices` surface instead of two separate `Send` and `Receive` actions.

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
- Android -> Desktop with Desktop popover open:
  - Android discovers Desktop;
  - Android sends handoff offer;
  - Desktop accepts;
  - Android transfers snapshot;
  - Desktop applies restore paused.
- Android -> Desktop with only the Desktop app open:
  - Desktop starts its handoff receiver during Nocky Connect action wiring;
  - Desktop starts a background UDP discovery responder;
  - Android discovers Desktop without the Desktop Nocky Connect popover being opened;
  - Android sends the handoff offer and snapshot;
  - Desktop applies the received queue paused.

Known implementation notes:

- Desktop receiver and UDP discovery responder are now started from the Nocky Connect action wiring and guarded as singleton background services.
- Desktop foreground discovery scans use an ephemeral UDP port so they can coexist with the background responder owning UDP `34987`.
- Android snapshot export must read ExoPlayer state on the main thread.
- Android pending restore remains an internal fallback for received desktop snapshots.
- Restored queues can still have incomplete artwork/metadata. Metadata hydration is a later polish phase.

### Current Desktop product surface

The Desktop entry point currently opens a Nocky Connect device-picker popover.

Implemented on Desktop:

- popover shows `This device` and `Available devices`;
- local Desktop descriptor advertises a local HTTP handoff endpoint;
- discovered devices are cached for 5 minutes;
- opening the popover renders cached devices immediately;
- Desktop starts its handoff receiver when Nocky Connect action wiring is installed;
- Desktop starts a background UDP discovery responder when Nocky Connect action wiring is installed;
- opening the popover starts a LAN scan immediately;
- while the popover remains open, Desktop refreshes LAN discovery every 15 seconds;
- scan overlap is guarded to avoid concurrent discovery workers;
- foreground scans coexist with the background discovery responder;
- selecting an Android row sends the current Desktop playback snapshot to Android;
- Android -> Desktop handoff applies the received queue paused even when the Desktop popover was never opened.

Still temporary on Desktop:

- the background responder currently runs as a lightweight loop, but still needs cleaner shutdown/error lifecycle semantics;
- surface is still a popover, not a full internal page;
- row states do not yet distinguish `available now` from `recently seen`;
- verbose diagnostic logs should be removed or gated before release.

### Current Android product surface

Android now uses a device-picker bottom-sheet instead of the old manual Send/Receive actions.

Implemented on Android:

- player main action opens Nocky Connect directly;
- duplicate three-dot menu entry has been removed;
- surface shows `This device` and `Available devices`;
- opening the surface starts a short Android presence window automatically;
- `Scan again` refreshes the list and starts a new short presence window;
- discovered devices are cached in memory for 5 minutes;
- failed scans keep recently seen devices visible;
- selecting a Desktop row sends the current Android playback snapshot to Desktop;
- old manual receive action and diagnostic branching were removed from the player surface;
- Android still receives Desktop -> Android through the HTTP receiver behind the presence flow.

Still temporary on Android:

- presence is currently a foreground short window, not lifecycle-aware app/player presence;
- strings are still hardcoded in the Kotlin surface and should move to resources;
- receiver confirmation UI is still future trust/polish work;
- cache is in-memory only and resets when the app process dies.

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
- pending restore store as an internal Android fallback;
- Desktop device cache;
- Desktop background receiver;
- Desktop background UDP discovery responder;
- Desktop periodic refresh while the popover is open;
- Android short presence window;
- Android device-picker surface.

Do not reintroduce the old manual `Send / Receive` Android UI as the primary flow. It has been replaced by the device picker.

## What should change next

### UX

Continue evolving the device picker with:

- live device list semantics;
- device row states:
  - scanning;
  - available now;
  - recently seen;
  - connecting;
  - waiting for confirmation;
  - failed / firewall hint;
- current device section;
- troubleshooting footer;
- localized strings.

Desktop should eventually move from a popover to an internal Nocky Connect page/surface similar to the Queue surface.

### Always-on discovery / presence

Future product behavior should not require opening the Nocky Connect surface to make a device visible.

Current status:

- Desktop starts a background receiver and UDP discovery responder when the app wires Nocky Connect actions.
- Android still uses a short foreground presence window when the Nocky Connect surface opens or refreshes.

Target behavior:

- Desktop keeps local presence/listener available while the app is running.
- Android starts local presence/listener when the app/player lifecycle allows it.
- Each side maintains a small live cache of recently seen devices.
- Opening the Nocky Connect surface renders the already-known device list and can trigger a manual refresh.
- Discovery should be rate-limited and lifecycle-aware, especially on Android, to avoid unnecessary battery/network usage.
- Receiver HTTP should remain a background singleton with clear lifecycle ownership.

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

- receiver may auto-accept only while the experimental local surface/presence flow is active.

Later version:

- show receiver accept/decline confirmation;
- remember trusted device ids;
- allow auto-accept only for trusted devices;
- provide a way to revoke trust.

## Updated roadmap

### Phase 0 - Stabilize bidirectional diagnostic handoff

Status: complete for the validated local prototype.

Validation commands:

```bash
cargo fmt --all
cargo test connect
cargo check --all-targets
```

```bash
./gradlew --no-configuration-cache :app:testFossDebugUnitTest --tests 'com.metrolist.music.connect.*'
./gradlew --no-configuration-cache :app:compileFossDebugKotlin
```

### Phase 1 - UX cleanup for current handoff

Status: mostly complete for Android; partially complete for Desktop.

Done:

- hide/remove user-facing Android debug-only restore actions;
- remove Android manual `Send / Receive` surface actions;
- keep pending restore as internal fallback;
- hide endpoint details from user-facing toasts;
- keep direct player entry point for Android Nocky Connect.

Remaining:

- improve failure copy and firewall hints;
- localize strings;
- gate verbose diagnostics.

### Phase 2 - Device list model

Status: in progress, usable prototype.

Done:

- Desktop discovered-device model with last-seen cache;
- Android discovered-device cache with last-seen timestamps;
- device list rendering on both platforms;
- dedupe by device id;
- cache expiry after 5 minutes.

Remaining:

- expose `recently seen` vs `available now` row states;
- share more naming/status semantics between Android and Desktop;
- persist friendly trusted device names later.

### Phase 3 - Spotify-style surfaces

Status: in progress.

Desktop:

- shows `This device` and `Available devices`;
- shows clickable device rows;
- keeps troubleshooting text for UFW/UDP `34987` and TCP `35187`;
- renders cached devices immediately;
- refreshes periodically while the popover is open;
- starts receiver/discovery responder in the background while the app is running;
- later migrate from popover to internal ViewStack page.

Android:

- keeps player bottom-sheet entry;
- uses a live device list instead of two static actions;
- shows empty/scanning/error states;
- shows device rows matching the desktop semantics;
- starts short presence automatically when the surface opens.

### Phase 4 - Always-on discovery and background receiver

Status: partially complete for Desktop; next major milestone for Android.

Done:

- Desktop starts background handoff receiver from Nocky Connect action wiring;
- Desktop starts background UDP discovery responder from Nocky Connect action wiring;
- Desktop foreground scans coexist with the background UDP responder.

Remaining:

- Android should start presence/receiver when app/player lifecycle allows it;
- Device picker should expose live/recently-seen row states;
- Discovery must be rate-limited and avoid battery-heavy loops;
- Desktop responder lifecycle should gain cleaner shutdown/restart semantics.

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

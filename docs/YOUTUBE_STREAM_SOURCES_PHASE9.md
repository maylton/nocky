# Phase 9 — Native stream-source preferences

## Goal

Expose Nocky's existing YouTube stream-client fallback policy through a native settings surface without requiring environment variables and without weakening the automatic defaults.

## Scope

This phase is limited to YouTube stream-source preferences and privacy-safe diagnostics.

Out of scope:

- replacing yt-dlp or GStreamer;
- changing local Home or local-library behavior;
- storing authentication material in new locations;
- exposing stream URLs, cookies, request headers or authorization values;
- implementing assisted browser login.

## Existing policy

The helper currently supports these client profiles:

1. `web_music` — WEB_REMIX, prefers the connected browser session;
2. `web_creator` — authenticated creator client, requires a connected session;
3. `tv` — TVHTML5 fallback, can use the connected session;
4. `android_vr` — unauthenticated native fallback;
5. `web` — general web compatibility fallback;
6. `ios` — optional manual client, disabled by default.

The automatic default order remains:

```text
web_music, web_creator, tv, android_vr, web
```

## Persisted model

Add a version-tolerant configuration object to `AppConfig`:

```text
youtube_stream_sources:
  order: [client keys]
  disabled: [client keys]
```

Normalization rules:

- unknown keys are discarded;
- duplicate keys are removed while preserving the first occurrence;
- known keys missing from `order` are appended in the built-in canonical order;
- profiles unavailable for the current authentication state remain visible but cannot be selected for that run;
- at least one runnable client must remain enabled;
- an empty or invalid stored object resolves to the built-in defaults;
- reset replaces both lists with the built-in defaults.

## Runtime boundary

`YouTubeBridge` owns the effective enabled order used for helper processes.

- The bridge passes the normalized order to the helper process through `NOCKY_YOUTUBE_STREAM_CLIENTS`.
- No process-global environment mutation is required.
- Existing automatic callers continue to work when no custom value is stored.
- Updating preferences affects future resolutions; a currently playing stream is not interrupted.
- Forced recovery still moves the previously rejected client to the end of the effective order.

## Native settings surface

Location: **Settings → YouTube Music → Fontes de stream**.

Each source row shows:

- display label;
- short capability description;
- authentication requirement;
- enabled switch;
- priority position;
- move-up and move-down actions.

The page also contains:

- **Restaurar padrões** action;
- explanation that the first compatible source is tried first;
- note that unavailable authenticated profiles are skipped automatically;
- current effective order summary;
- privacy-safe current-stream diagnostic when available.

## Interaction rules

- Reordering is immediately reflected in the saved configuration.
- Disabling a source keeps its relative position for later re-enabling.
- The UI prevents disabling the final runnable source.
- `web_creator` is shown as unavailable when no account session is connected.
- `ios` is visible but disabled by default.
- Reset is non-destructive to account, library, cache and offline downloads.

## Diagnostics

Allowed fields:

- selected client key and label;
- attempted client keys;
- whether fallback was used;
- format ID, protocol, container and audio codec;
- timestamp of the current resolution.

Forbidden fields:

- stream URL;
- webpage URL containing identifiers in copied reports;
- cookies;
- authorization values;
- request headers;
- raw yt-dlp command lines.

## Delivery sequence

### 9A — Persisted policy foundation

- configuration model and normalization;
- bridge propagation to helper subprocesses;
- deterministic Rust tests;
- compatibility with missing legacy fields.

### 9B — Native settings UI

- source rows with switches and ordering actions;
- reset to defaults;
- localized Portuguese, English and Spanish labels;
- immediate persistence and bridge refresh.

### 9C — Diagnostics and validation

- current effective-order summary;
- current-stream client metadata;
- smoke test proving the custom order reaches the helper;
- recovery test proving the failed client still rotates to the end;
- narrow-window and keyboard validation;
- manual confirmation that reordering, enabling/disabling, final-source
  protection, reset, persistence and diagnostics work in the native dialog.

## Acceptance criteria

- A fresh or legacy configuration uses the current automatic defaults.
- Preferences persist across restarts.
- Unknown or duplicate stored keys cannot break stream resolution.
- The final runnable source cannot be disabled.
- Reordering changes the next real resolution attempt order.
- Reset restores the current default order and enabled state.
- Forced recovery does not retry the rejected client first.
- No sensitive stream or account material is displayed or logged.
- Local Home and `src/browser.rs` remain outside the implementation diff.
- Format, check, tests, Clippy and Python tests pass in the Quality Gate.
- Manual stream-source dialog validation passes before closing the phase.

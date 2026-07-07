# Nocky Performance Audit Baseline

This document tracks the first optimization phase for Nocky. The goal is to measure before changing behavior.

## Scope

Phase 1 adds lightweight, opt-in performance tracing. It must not change UI design, playback behavior, theme behavior, cache policy, or YouTube/Local feature semantics.

Tracing is silent by default and only emits logs when explicitly enabled.

## Enable tracing

Run Nocky with:

```bash
NOCKY_PERF_TRACE=1 cargo run
```

Truthy values are:

```text
1, true, yes, on
```

Any other value, or an unset variable, disables tracing.

## Log format

Trace lines are written to stderr using a compact key/value format:

```text
[perf] event=<event.name> duration_ms=<milliseconds> key=value
```

Values containing whitespace are quoted.

## Initial events

Phase 1 currently emits these events:

| Event | Meaning |
| --- | --- |
| `app.start` | Process entered the GTK application run path. |
| `app.run` | Total GTK application run duration. Usually prints when the app exits. |
| `app.activate` | GTK activation path, including controller construction, callback setup, saved-library scheduling, and initial presentation. |
| `controller.setup_callbacks` | Signal/timer callback installation for the app controller. |
| `library.load_saved` | Startup check for an existing local music directory. |
| `library.scan_request` | Scheduling path for a local library scan request. |
| `library.scan_worker` | Background local library scan duration. Includes `root=<path>`. |
| `library.apply_scanned` | Applying scanned local tracks to app state and refreshing visible browser state. Includes `tracks=<count>` when the scan changed the library. |

## Manual baseline checklist

Use this checklist before and after later optimization phases:

1. Start the app with tracing enabled:

   ```bash
   NOCKY_PERF_TRACE=1 cargo run 2>&1 | tee /tmp/nocky-perf.log
   ```

2. Exercise the same flow each time:

   - launch app;
   - wait for first Home paint;
   - switch between Local and YouTube if configured;
   - open a large playlist;
   - open an album;
   - open an artist;
   - return Home;
   - trigger a local library refresh if a local folder is configured;
   - close app.

3. Compare the generated trace:

   ```bash
   grep '^\[perf\]' /tmp/nocky-perf.log
   ```

## Current limitations

Phase 1 intentionally starts with very low-risk instrumentation points. It does not yet measure every desired surface.

Planned next instrumentation targets:

- `browser.refresh` and `browser.navigate` timing;
- Home V3 section rendering counts;
- YouTube Home request, cached-first response, remote response and cover-cache timing;
- playlist/album/artist first-paint timing;
- artwork cache hit/miss and decode timing;
- stream resolution timing;
- animation/frame callback cost.

## Phase 2 direction

The next optimization phase should focus on browser/Home rebuild auditing:

- count refresh and navigation calls;
- identify refreshes caused by playback updates, artwork completion, chip changes and continuations;
- preserve the current Home V3 behavior;
- avoid visual changes;
- add focused tests where possible.


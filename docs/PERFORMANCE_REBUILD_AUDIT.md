# Browser and Home Rebuild Performance Audit

This document defines Phase 2 of the Nocky performance work. Phase 2 is an audit phase only: it should measure Browser/Home rebuilds before any behavior-changing optimization.

## Goals

- Count and time Browser refresh work.
- Count and time route navigation work.
- Identify whether YouTube Home V3 is rebuilding or reusing the existing shell.
- Capture enough context to explain expensive rebuilds without logging private media data.
- Keep all tracing opt-in through `NOCKY_PERF_TRACE=1`.

## Non-goals

- No visual or CSS changes.
- No Home V3 design changes.
- No Queue behavior changes.
- No playback behavior changes.
- No cache policy changes yet.
- No Android changes.

## Planned events

### `browser.refresh`

Emitted by the controller refresh path after the visible browser content has been refreshed.

Planned fields:

| Field | Meaning |
| --- | --- |
| `route` | Current browser route debug label. |
| `home_visible` | Whether the route is Home with no active search query. |
| `query` | Whether the search query is active. |
| `tracks` | Effective local track count visible to the browser. |
| `youtube_sections` | Legacy YouTube Home section count. |
| `youtube_native_sections` | Native Home V3 section count, when available. |
| `youtube_loading` | Whether YouTube Home is marked as loading. |
| `has_library` | Whether the app is showing library content instead of empty state. |

### `browser.refresh.deferred`

Emitted if the refresh path is skipped because a required borrow is unavailable.

Planned fields:

| Field | Meaning |
| --- | --- |
| `reason` | Skip reason, initially `config_borrow`. |

### `browser.navigate`

Emitted by the controller route navigation path after the browser has navigated.

Planned fields:

| Field | Meaning |
| --- | --- |
| `from` | Previous route debug label. |
| `to` | Target route debug label. |
| `changed` | Whether the route actually changed. |
| `query` | Whether the search query is active. |
| `tracks` | Effective local track count visible to the browser. |
| `youtube_sections` | Legacy YouTube Home section count. |
| `youtube_native_sections` | Native Home V3 section count, when available. |
| `youtube_loading` | Whether YouTube Home is marked as loading. |

### `browser.navigate.skipped`

Emitted if navigation is skipped because a required borrow is unavailable.

Planned fields:

| Field | Meaning |
| --- | --- |
| `reason` | Skip reason, initially `config_borrow`. |

### `home.v3.feed_shell`

Emitted by the YouTube Home V3 feed shell renderer.

Planned fields:

| Field | Meaning |
| --- | --- |
| `cached` | Whether the existing Home V3 shell was reused. |
| `sections` | Number of sections in the Home V3 page. |
| `items` | Total item count across sections. |
| `chips` | Filter chip count. |
| `loading` | Whether the shell was rendered while loading. |

## Smoke flow

Run:

```bash
NOCKY_PERF_TRACE=1 cargo run 2>&1 | tee /tmp/nocky-perf-phase2.log
```

Exercise this flow consistently:

1. launch app;
2. wait for Home to finish first paint;
3. navigate to Albums;
4. navigate back Home;
5. navigate to Artists;
6. navigate back Home;
7. switch to YouTube mode if configured;
8. wait for YouTube Home;
9. press Load more if available;
10. close the app.

Filter useful lines:

```bash
grep '^\[perf\] event=browser' /tmp/nocky-perf-phase2.log
grep '^\[perf\] event=home.v3' /tmp/nocky-perf-phase2.log
```

Summarize timings and counts:

```bash
scripts/perf-log-summary.py /tmp/nocky-perf-phase2.log
```

The summary helper accepts a file path or stdin:

```bash
NOCKY_PERF_TRACE=1 cargo run 2>&1 | scripts/perf-log-summary.py
```

## How to interpret

Repeated `browser.refresh` lines with the same route and unchanged section/item counts suggest redundant refresh work.

Repeated `browser.navigate` lines where `changed=false` suggest route reapplication that may be avoidable later.

Repeated `home.v3.feed_shell cached=false` lines for identical Home V3 content suggest the shell is being rebuilt instead of reused.

`home.v3.feed_shell cached=true` confirms the existing Home V3 shell reuse path is working.

The summary helper is useful for quick comparisons: event counts show frequency, while `min_ms`, `avg_ms` and `max_ms` show timing spread.

## Phase 2 acceptance criteria

- Tracing remains silent unless `NOCKY_PERF_TRACE=1` is enabled.
- Browser/Home audit events include enough context to identify redundant rebuilds.
- No UI changes.
- No behavior changes.
- No optimization yet; optimization belongs to Phase 3+ after log analysis.

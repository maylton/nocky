# Smooth lyrics and performance pass

This patch is based on commit `1e949d932eba8e5fc1535c6a3803e49048ba8407`
from `feat/compositor-blur`.

## Lyrics

- The active line is found with binary search.
- GTK labels are updated only when the active line changes.
- The full lyrics view scrolls the active line toward the center.
- The five-line Home preview uses a short crossfade.
- Opening the Lyrics view recenters the current line.
- YouTube lyrics are no longer cloned on every progress tick.

## Navigation

- Collection and detail pages use directional stack transitions.
- Top-level pages keep a short crossfade.
- Transition durations are explicit and consistent.

## Resource use

- The 50 ms event loop remains responsive, but GStreamer position queries run
  every 100 ms while playing and every 500 ms while paused.
- The visualizer renders at approximately 30 FPS and skips work while hidden or
  inactive.
- Volume and blur-opacity configuration writes are debounced.
- The artwork texture cache uses bounded least-recently-used eviction and
  invalidates entries when the source file modification timestamp changes.
- Automatic playlist prefetch is limited to four playlists.
- Cover downloads reuse a single HTTP client.
- The persisted YouTube library cache uses compact JSON.

## Validation

Run:

```bash
cargo fmt
cargo test
cargo check
git diff --check
```

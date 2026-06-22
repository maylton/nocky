# Cache-first YouTube playlist loading

This patch addresses the long pause after opening a YouTube Music playlist.

## Changes

- Navigation switches to the playlist page immediately.
- A spinner row is shown while an uncached playlist is loading.
- Existing in-memory or persisted playlist cache remains the first path.
- Runtime cache is now also reused for mixes and generated playlists.
- Only the latest playlist click is kept pending while another request finishes.
- Home playlist prefetch uses a bounded pool of three workers instead of
  downloading every playlist sequentially.
- Cold playlist requests are limited to 120 tracks for the first implementation.
- The first 32 rows are painted immediately.
- Remaining rows are appended in idle batches of 24 to preserve GTK
  responsiveness.
- Scheduled row rendering is cancelled when navigation changes.

This does not introduce the persistent Python helper yet. That remains the next
architectural optimization if cold network requests are still slow.

## Validation

```bash
cargo fmt
cargo test
cargo check
python3 -m py_compile helpers/nocky_youtube.py
git diff --check
```

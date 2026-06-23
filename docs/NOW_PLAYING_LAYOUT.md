# Consistent now-playing layout

- Hero artwork grows from 230 px to 280 px.
- Placeholder and real artwork share the same expandable centered slot.
- Metadata and controls keep the same vertical position with or without media.
- The artwork transition uses a 180 ms crossfade.
- Home preload, cache, playback, lyrics, blur, and YouTube recovery are unchanged.

Validation:

```bash
cargo fmt
cargo test
cargo check
git diff --check
```

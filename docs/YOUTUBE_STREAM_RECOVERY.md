# YouTube stream recovery

Base commit: `cc7474fe58ef3aa5051993959d4262d188682b59`

This change handles temporary Googlevideo stream rejection without requiring
the user to delete the cache manually.

## Behavior

1. GStreamer reports a recoverable HTTP 401, 403, or 410 from `souphttpsrc`.
2. Nocky stops the rejected pipeline and requests a fresh signed URL with
   `force=true`.
3. The rejected cached entry is invalidated.
4. `yt-dlp` verifies the selected format with `--check-formats`.
5. The refreshed stream is loaded with its HTTP headers.
6. Lyrics, cover and queue state are preserved.
7. Playback resumes near the previous position after GStreamer reports that the
   new stream is ready.

Only one automatic refresh is attempted for each selected track. This prevents
an infinite retry loop.

## Interface and diagnostics

- Toast markup is disabled, so URLs containing `&` do not trigger GTK markup
  parsing warnings.
- Full signed Googlevideo query strings are redacted from application logs.
- The user sees short, translated-friendly error messages instead of the
  internal GStreamer pipeline path.
- Vulkan `VK_SUBOPTIMAL_KHR` warnings are not treated as audio failures.

## Validation

```bash
cargo fmt
cargo test
cargo check
python3 -m py_compile helpers/nocky_youtube.py
git diff --check
```

# Cache-first YouTube albums and artists

This follow-up completes the Home collection loading work started by the
playlist cache-first patch.

## Behavior

- Suggested album and artist identifiers are preserved instead of being reduced
  to display-only title strings.
- Clicking a Home or collection-page album/artist navigates immediately.
- An inline spinner is displayed during a cold request.
- Results are persisted in the existing YouTube library cache.
- Reopening a loaded album or artist is immediate.
- The first 32 tracks are painted synchronously and remaining rows are appended
  in idle batches.
- Up to three album/artist prefetch workers warm the first six suggested albums
  and first six suggested artists.
- Catalog-derived albums/artists without a browse ID continue using the already
  synchronized library as a fallback.
- Episode-only playlists no longer generate a noisy expected preload warning.
- Unsupported GTK CSS `max-width` declarations are removed.

## External GTK warnings

Warnings about missing files in `~/.config/gtk-4.0` and the deprecated
`gtk-application-prefer-dark-theme` setting come from the user's GTK
configuration, not Nocky. Use the optional cleanup script included with the
patch package; it creates backups before changing anything.

## Validation

```bash
cargo fmt
cargo test
cargo check
python3 -m py_compile helpers/nocky_youtube.py
git diff --check
```

# Home V2 polish validation

The PR #82 validation covers these regressions:

- generated `RD...` playlists remain read-only without false metadata mismatch errors;
- playlist headers without a numeric count do not fail pre-caching;
- playlist, album and artist artwork is prioritized within the Home cover budget;
- tracks inside generated playlists and mixes fall back to the canonical `videoId` thumbnail when artwork is missing or the original URL fails;
- the chip carousel reserves 80 px of height plus a larger bottom inset for its horizontal scrollbar;
- load-more appends sections without rebuilding the current Home page.

A real-account smoke test should clear the Home, library and cover caches, reopen
Home V2, inspect the chip spacing, open generated playlists/mixes and confirm that
track artwork is restored, load another recommendation page, and confirm that an
empty owned playlist no longer logs an integer conversion error.

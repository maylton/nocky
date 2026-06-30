# Home V2 polish validation

The PR #82 validation covers these regressions:

- generated `RD...` playlists remain read-only without false metadata mismatch errors;
- playlist headers without a numeric count do not fail pre-caching;
- playlist, album and artist artwork is prioritized within the Home cover budget;
- a valid playlist video ID supplies a safe YouTube thumbnail fallback when artwork is absent;
- the chip carousel reserves enough height and bottom inset for its horizontal scrollbar;
- load-more appends sections without rebuilding the current Home page.

A real-account smoke test should clear only the Home feed cache, reopen Home V2,
inspect collection artwork and chip spacing, load another recommendation page,
and confirm that an empty owned playlist no longer logs an integer conversion error.

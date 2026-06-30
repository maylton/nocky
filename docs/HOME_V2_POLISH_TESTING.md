# Home V2 polish validation

The PR #82 validation covers these regressions:

- generated `RD...` playlists remain read-only without false metadata mismatch errors;
- playlist headers without a numeric count do not fail pre-caching;
- Home artwork caching is distributed in round-robin order across card sections, preventing collection rows from starving song rows;
- tracks inside generated playlists and mixes fall back to the canonical `videoId` thumbnail when artwork is missing or the original URL fails;
- the chip carousel reserves 88 px of height plus a larger bottom inset for its horizontal scrollbar;
- load-more appends sections without rebuilding the current Home page.

The artwork smoke test must include both collection-heavy rows and playable song
rows such as `Escolha a dedo`, because the former previously exhausted the global
cover budget before the latter were processed.

A real-account smoke test should clear the Home, library and cover caches, reopen
Home V2, inspect the chip spacing, verify artwork in both collection and song rows,
open generated playlists/mixes and confirm that track artwork is restored, load
another recommendation page, and confirm that an empty owned playlist no longer
logs an integer conversion error.

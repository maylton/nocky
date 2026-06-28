# YouTube Music Library Roadmap

This roadmap isolates the YouTube Music experience from Nocky's local Home. The local Home, local cards, local routes and offline/local playback model remain unchanged.

## Guardrails

- All work is developed on `codex/youtube-library-structured-feed`.
- The existing flat `home` helper command remains available for the current synchronization path.
- The new structured feed is consumed only by the dedicated **YouTube Music** page.
- No changes are made to `src/browser.rs` Home composition or to local-library routes.
- Authentication remains browser-session based and is stored through Secret Service when available.
- Every network-facing addition has a cache/fallback path and sanitized parser fixtures.

## Phase 1 — Versioned feed contract

**Goal:** stop flattening YouTube Music sections before they reach GTK.

Deliverables:

- `home_v2` helper command with a versioned response.
- Ordered sections with title, label, layout, endpoint, items and continuation.
- Preservation of songs, videos, albums, artists, playlists, podcasts and episodes.
- Stable section IDs and per-section deduplication.
- Sanitized fixtures and Python unit tests.

Acceptance criteria:

- Section order matches the upstream payload.
- Duplicate items inside a section are removed without merging different sections.
- Unknown renderer shapes are ignored safely rather than crashing the whole page.

## Phase 2 — Native Rust domain model

**Goal:** make the structured response a first-class Nocky model.

Deliverables:

- `YouTubeHomePage`, `YouTubeHomeSection`, `YouTubeHomeChip` and endpoint models.
- Merge behavior for continuation pages.
- Backend boundary (`YouTubeMusicBackend`) so ytmusicapi can later be replaced incrementally.
- Rust tests for contract deserialization and continuation merging.

Acceptance criteria:

- Older/missing fields deserialize through `#[serde(default)]`.
- Continuation merging does not duplicate tracks or collections.

## Phase 3 — Dedicated YouTube Music feed UI

**Goal:** render the online library/feed in the YouTube Music page without touching local Home.

Deliverables:

- **Para você** action for the structured recommendation feed.
- **Visão geral** action for account-library sections.
- Section headers and preserved grouping in the existing GTK page.
- Playable rows, playlist navigation and a continuation row.
- Automatic initial feed load after a valid account session is detected.

Acceptance criteria:

- Switching between feed, library, likes, playlists and search does not affect local Home.
- Section headers cannot be activated as media.
- Continuation appends rather than replacing already rendered sections.

## Phase 4 — Cache and resilient loading

**Goal:** keep the online library useful during transient YouTube failures.

Deliverables:

- Atomic, permission-restricted feed cache under Nocky's YouTube cache directory.
- Stale fallback when a refresh fails.
- Visual indication when cached feed data is being shown.
- Synthetic section continuation compatible with ytmusicapi's current `get_home` API.

Acceptance criteria:

- A valid cached feed is returned after a network/API failure.
- Cache writes are atomic and mode `0600`.

## Phase 5 — Authentication hardening

**Goal:** reduce stored session surface while preserving the current working login flow.

Deliverables:

- Require a SAPISID-family cookie before accepting imported data.
- Persist only the minimum headers required by ytmusicapi and stream extraction.
- Recompute `SAPISIDHASH` instead of trusting a copied authorization header.
- Add a button that opens YouTube Music in the system browser; Nocky never asks for a Google password.
- Keep manual cURL/Cookie import as the compatibility fallback.

Acceptance criteria:

- Arbitrary cookies are rejected.
- Unrelated copied request headers are not stored.
- Existing Secret Service/protected-file behavior remains intact.

## Phase 6 — Broader account-library coverage and quality gate

**Goal:** make the structured page useful beyond songs and playlists.

Deliverables:

- Account overview sections for recently added songs, likes, playlists, albums and artists.
- Podcast/episode-compatible item contract.
- Python parser tests integrated into `scripts/quality-gate.sh`.
- Installation of the new helper module and roadmap documentation.

Acceptance criteria:

- The source installer includes every runtime helper file.
- Rust formatting, compilation, tests, Clippy and Python tests pass in CI.

## Research track — Native InnerTube backend

A direct Rust InnerTube backend remains a research track rather than a forced rewrite. The backend trait introduced here is the migration seam. Playback continues to use `yt-dlp + Deno + GStreamer` because replacing signature, `n` and PO-token handling prematurely would reduce reliability. A native backend should be introduced endpoint by endpoint behind fixture-based contract tests, with ytmusicapi retained as fallback until parity is demonstrated.

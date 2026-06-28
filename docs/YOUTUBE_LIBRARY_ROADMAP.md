# YouTube Music integration roadmap

This roadmap isolates the YouTube Music experience from Nocky's local Home. The local Home, local cards, local routes and offline/local playback model remain unchanged.

## Guardrails

- New work is developed in small stacked branches and draft pull requests.
- The existing flat `home` helper command remains available for the current synchronization path.
- Structured YouTube Music content is consumed only by the dedicated **YouTube Music** page.
- `src/browser.rs` Home composition and local-library routes remain untouched.
- Authentication stays browser-session based until an assisted login flow is proven safe and optional.
- Every network-facing addition requires a cache/fallback path, redacted diagnostics and fixture-based tests.
- A phase is complete only after its acceptance criteria and validation gate have passed.

## Current delivery status

| Phase | Status | Delivery |
| --- | --- | --- |
| 1. Versioned feed contract | Complete | PR #40 |
| 2. Native Rust domain model | Complete | PR #40 |
| 3. Dedicated feed UI | Implemented; live validation pending | PR #40 |
| 4. Cache and resilient loading | Complete | PR #40 |
| 5. Authentication hardening | Complete for manual session import | PR #40 |
| 6. Broader account-library contract | Partial: data contract complete, navigation incomplete | PR #40 |
| 7. Stream-client fallback policy | Implemented; stacked-PR CI pending | PR #41 |
| 8. Integration hardening and real-account validation | Next | `codex/youtube-integration-hardening` |
| 9. Native stream-source preferences | Planned | after Phase 8 |
| 10. Assisted browser login | Planned, optional | after Phase 9 |
| 11. Remote library mutations and account profiles | Planned | later |
| 12. Native InnerTube backend | Research track | later |
| 13. Release hardening and observability | Planned | before stable release |

## Phase 1 — Versioned feed contract

**Goal:** stop flattening YouTube Music sections before they reach GTK.

Delivered:

- `home_v2` helper command with a versioned response.
- Ordered sections with title, label, layout, endpoint, items and continuation.
- Preservation of songs, videos, albums, artists, playlists, podcasts and episodes.
- Stable section IDs, per-section deduplication and sanitized parser fixtures.

## Phase 2 — Native Rust domain model

**Goal:** make the structured response a first-class Nocky model.

Delivered:

- `YouTubeHomePage`, section, chip and endpoint models.
- Continuation merge behavior.
- `YouTubeMusicBackend` migration boundary.
- Serde-compatible defaults and Rust contract tests.

## Phase 3 — Dedicated YouTube Music feed UI

**Goal:** render the online library/feed without touching local Home.

Implemented:

- **Para você** and **Visão geral** actions.
- Structured section headers, playable rows and continuation rows.
- Automatic load after a valid session is detected.

Still required before completion:

- Real-account validation of ordering, stale fallback and continuation behavior.
- Responsive behavior for narrow windows.
- Accessibility review of rows, headings and loading state.

## Phase 4 — Cache and resilient loading

**Goal:** keep the online library useful during transient failures.

Delivered:

- Atomic permission-restricted feed cache.
- Stale fallback and visible stale state.
- Synthetic section continuation compatible with the current ytmusicapi API.

## Phase 5 — Authentication hardening

**Goal:** reduce stored session surface while preserving the working login flow.

Delivered:

- Required SAPISID-family cookie.
- Minimum persisted header allowlist.
- Local `SAPISIDHASH` recomputation.
- System-browser shortcut and manual cURL/Cookie import fallback.

## Phase 6 — Broader account-library coverage

**Goal:** support the full set of useful YouTube Music collection types.

Delivered:

- Recently added songs, likes, playlists, albums and artists in the account overview.
- Podcast and episode-compatible data contract.
- Parser tests in the quality gate and complete helper installation.

Incomplete:

- Dedicated activation/navigation for albums and artists from the structured page.
- Podcast and episode navigation behavior.
- Actionable section chips and section endpoints.
- Graceful unsupported-item messaging rather than silent no-op behavior.

## Phase 7 — Stream-client fallback policy

**Goal:** avoid repeatedly resolving a rejected URL with the same YouTube client identity.

Implemented in PR #41:

- Ordered client policy using supported yt-dlp clients.
- Client rotation after recoverable GStreamer/CDN failures.
- Terminal availability-error detection.
- Redacted diagnostics, selected-client metadata and deterministic tests.

Pending validation:

- Run CI for stacked pull requests.
- Exercise at least one real fallback after a rejected or expired stream URL.
- Confirm Premium and non-authenticated behavior on the target workstation.

## Phase 8 — Integration hardening and real-account validation

**Goal:** close functional gaps before exposing more settings.

Deliverables:

- Allow the Quality Gate workflow to run for stacked `codex/**` pull-request bases.
- Add structured-page events for opening albums and artists.
- Define podcast and episode activation behavior; unsupported shapes must show a clear message.
- Make chips and section endpoints actionable where the helper provides valid endpoints.
- Preserve current page state while loading a collection and recover cleanly from errors.
- Improve the action bar for narrow windows using wrapping or an adaptive container.
- Review keyboard activation, accessible labels and focus order.
- Add fixture and Rust tests for item-action routing.
- Perform a manual smoke test with a connected account covering feed, overview, continuation, album, artist, playlist, playback recovery and stale cache.

Acceptance criteria:

- No structured item silently does nothing.
- Album, artist and playlist rows navigate to the correct native view.
- Podcast/episode rows either work or show an explicit supported-state message.
- Stacked PRs receive an automated quality-gate result.
- The local Home remains byte-for-byte outside the implementation diff.

## Phase 9 — Native stream-source preferences

**Goal:** expose the fallback policy without requiring environment variables.

Deliverables:

- Native **Fontes de stream** page within YouTube Music settings.
- Enabled/disabled state and ordered priority persisted in Nocky's configuration.
- Safe reset to defaults.
- Availability/authentication explanation for each client.
- Diagnostics showing the client used by the current stream without exposing URLs or headers.

The automatic default policy must remain reliable without user configuration.

## Phase 10 — Assisted browser login

**Goal:** reduce manual cookie-copy friction without turning Nocky into a web wrapper.

Deliverables:

- Optional isolated WebKitGTK login window.
- Strict navigation allowlist for Google Accounts and YouTube Music.
- Capture only the minimum session data after successful login.
- No permanent JavaScript bridge and no password access.
- Manual import remains available as fallback.

This phase requires a separate privacy and packaging review before implementation.

## Phase 11 — Remote library mutations and account profiles

Planned capabilities:

- Like/unlike feedback in all relevant views.
- Create, rename and edit remote playlists where supported.
- Add/remove playlist tracks with optimistic UI and rollback.
- Account/channel profile selection and clear active-profile indication.

## Phase 12 — Native InnerTube backend research

A direct Rust InnerTube backend remains a research track rather than a forced rewrite. The backend trait is the migration seam. Playback continues to use `yt-dlp + Deno + GStreamer` until signature, `n` and PO-token parity is demonstrated. Endpoints should migrate individually behind fixture-based contract tests while ytmusicapi remains available as fallback.

## Phase 13 — Release hardening and observability

Before a stable release:

- Define migration and rollback behavior for caches and persisted settings.
- Add privacy-safe counters for resolver attempts and fallback outcomes in debug logs.
- Test first-run, disconnected, expired-session, offline and partial-service states.
- Verify Flatpak permissions and runtime helper packaging.
- Complete localization, accessibility and responsive-layout review.
- Publish user-facing troubleshooting documentation.

## Critical decision gates

Stop for explicit review before:

- embedding a browser/login engine;
- persisting new authentication material;
- replacing yt-dlp or GStreamer;
- adding remote destructive mutations;
- changing local Home or local-library behavior;
- enabling telemetry beyond local privacy-safe debug diagnostics.

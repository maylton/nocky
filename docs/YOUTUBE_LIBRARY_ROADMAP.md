# YouTube Music integration roadmap

This roadmap isolates the YouTube Music experience from Nocky's local Home. The
local Home, local cards, local routes and offline/local playback model remain
unchanged. When the user selects YouTube Music as the Home source, the Home
route should follow the Android fork's online Home model: chips and section
headers come from the structured YouTube Music feed.

## Guardrails

- New work is developed in small stacked branches and draft pull requests.
- The existing flat `home` helper command remains available for the current synchronization path.
- Structured YouTube Music content is consumed by source-aware YouTube surfaces:
  YouTube Home, Library, Liked songs and Search.
- Local-library routes remain untouched unless a phase explicitly targets the
  local Home.
- Do not add arbitrary local filters, time-window chips or local history
  groupings to the YouTube Home. YouTube feed chips are the primary tab/filter
  model for that surface.
- Authentication stays browser-session based until an assisted login flow is proven safe and optional.
- Every network-facing addition requires a cache/fallback path, redacted diagnostics and fixture-based tests.
- A phase is complete only after its acceptance criteria and validation gate have passed.

## Current delivery status

| Phase | Status | Delivery |
| --- | --- | --- |
| 1. Versioned feed contract | Complete | PR #40 |
| 2. Native Rust domain model | Complete | PR #40 |
| 3. Dedicated feed UI | Complete and validated | PR #40 / PR #42 |
| 4. Cache and resilient loading | Complete and validated | PR #40 / PR #42 |
| 5. Authentication hardening | Complete for manual session import | PR #40 |
| 6. Broader account-library contract | Complete for currently returned account content | PR #40 / PR #42 |
| 7. Stream-client fallback policy | Complete; authenticated recovery rotation validated | PR #41 / PR #42 |
| 8. Integration hardening and real-account validation | Complete | PR #42 |
| 9. Native stream-source preferences | Complete and manually validated | PR #43 |
| 10. Android-parity YouTube Home organization | Complete and manually validated | PR #46 |
| 11. Assisted browser login | Planned, optional | later |
| 12. Remote library mutations and account profiles | Planned | later |
| 13. Native InnerTube backend | Research track | later |
| 14. Release hardening and observability | Planned | before stable release |

## Source and page model

Nocky has one **Home** route, but its content is source-aware:

- **Local Home** uses local library, local listening history, local playlists,
  local mixes and local privacy controls.
- **YouTube Music Home** uses the structured YouTube feed contract. It should
  expose feed chips, then render each returned section with the YouTube title,
  label, thumbnail, endpoint and continuation semantics.
- **Library** is for account/library inventory such as playlists, albums,
  artists and liked content. It should not replace the feed-oriented Home.
- **Search** is query-driven and can keep category groups, pagination and
  cached fallback states.
- **Queue** and **Lyrics** remain playback/task surfaces, not feed tabs.

The Android fork reference is:

- `HomeViewModel.homePage` stores the current structured `HomePage`.
- `selectedChip` swaps the visible feed by calling the YouTube Home endpoint
  with the selected chip params.
- `HomeScreen` renders `ChipsRow(homePage.chips)` first.
- `HomeScreen` then appends `HomePageSection(index)` for each
  `homePage.sections` entry and renders `NavigationTitle` from the section's
  own title, label, thumbnail and endpoint.

Desktop Nocky should translate that model into GTK rather than inventing a
parallel taxonomy.

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

Delivered and validated:

- **Para você** and **Visão geral** actions.
- Structured section headers, playable rows, collection carousels and continuation rows.
- Quick picks and collection sections render as cards before long song lists.
- Automatic load after a valid session is detected.
- Card-first rendering in **Para você**, **Visão geral**, **Biblioteca** and **Curtidas**.
- Continuation append with preserved vertical scroll position.
- Keyboard activation and narrow-window horizontal usability.
- The local Home remains unchanged.

## Phase 4 — Cache and resilient loading

**Goal:** keep the online library useful during transient failures.

Delivered and validated:

- Atomic permission-restricted feed cache.
- Stale fallback and visible stale state.
- Synthetic section continuation compatible with the current ytmusicapi API.
- Offline requests return cached structured sections with `stale: true`.
- Cache permissions remain `0600`.

## Phase 5 — Authentication hardening

**Goal:** reduce stored session surface while preserving the working login flow.

Delivered:

- Required SAPISID-family cookie.
- Minimum persisted header allowlist.
- Local `SAPISIDHASH` recomputation.
- System-browser shortcut and manual cURL/Cookie import fallback.

## Phase 6 — Broader account-library coverage

**Goal:** support the full set of useful YouTube Music collection types.

Delivered and validated:

- Recently added songs, likes, playlists, albums and artists in structured account pages.
- Card-first **Visão geral**, **Biblioteca** and **Curtidas** layouts.
- Album and artist fallbacks derived from song metadata when dedicated library endpoints are empty.
- Podcast and episode-compatible data contract.
- Parser tests in the quality gate and complete helper installation.
- Explicit unsupported-item feedback rather than silent no-op behavior.
- Native album, artist and playlist transitions from the YouTube dialog into routed browser pages.

Conditional follow-up:

- Confirm podcast/episode behavior when this content is returned by the connected account.
- Keep chips non-actionable until a stable helper endpoint is available.

## Phase 7 — Stream-client fallback policy

**Goal:** avoid repeatedly resolving a rejected URL with the same YouTube client identity.

Delivered and validated:

- Ordered client policy using supported yt-dlp clients.
- Client rotation after recoverable GStreamer/CDN failures.
- Terminal availability-error detection.
- Redacted diagnostics, selected-client metadata and deterministic tests.
- A real authenticated track resolved with an initial client.
- Forced recovery did not retry the rejected client first.
- Recovery traversed the configured order and selected a working client.
- The original stream cache was restored after the smoke test.

Useful before stable release:

- Confirm behavior without an authenticated account.
- Confirm behavior for Premium-only content when available.

## Phase 8 — Integration hardening and real-account validation

**Goal:** close functional gaps before exposing more settings.

Completed and validated:

- Quality Gate workflow runs for stacked `codex/**` pull-request bases.
- Structured-page routing for albums, artists, playlists and playable items.
- Podcast and episode activation behavior with explicit unsupported feedback.
- Current page state preserved while collection data loads.
- Native collection navigation closes the YouTube dialog before revealing the browser route.
- Continuation rebuilds preserve the previous vertical scroll position.
- Horizontal action bar and carousels remain usable at the minimum window width.
- Card buttons activate through mouse, `Enter` and space; focus traversal works with `Tab` and `Shift+Tab`.
- Fixture, Rust and Python tests cover item-action routing and account-page ordering.
- `scripts/smoke-youtube-structured.sh` validates the connected structured contract without exposing sensitive data.
- `scripts/smoke-youtube-stale-cache.sh` validates offline stale fallback.
- `scripts/smoke-youtube-stream-recovery.sh` validates forced client rotation and restores the original stream cache.
- Local and GitHub Quality Gates pass.

Acceptance criteria met:

- No structured item silently does nothing.
- Album, artist and playlist items navigate to the correct native view.
- Unsupported podcast/episode states produce explicit feedback.
- Stacked PRs receive automated quality-gate results.
- The local Home remains outside the implementation diff.

## Phase 9 — Native stream-source preferences

**Goal:** expose the fallback policy without requiring environment variables.

Implemented:

- Version-tolerant `youtube_stream_sources` configuration with ordered and disabled client keys.
- Normalization of unknown, duplicated and missing values with a safe built-in fallback.
- Protection against disabling every runnable source.
- Automatic helper consumption of the persisted order while preserving environment override priority.
- Native **Fontes de stream** settings entry and dialog.
- Enabled/disabled switches, move-up and move-down actions, and immediate persistence.
- Safe **Restaurar padrões** action.
- Portuguese, English and Spanish labels and client capability descriptions.
- Effective-order summary in the settings surface.
- Privacy-safe last-stream diagnostic showing only client, format, protocol, container, codec and fallback state.
- Controller safeguards that preserve the latest stream policy when unrelated settings are saved.

Automated validation completed:

- Legacy configurations resolve to the current defaults.
- Unknown and duplicate client keys are normalized deterministically.
- A custom persisted order reaches the helper.
- `NOCKY_YOUTUBE_STREAM_CLIENTS` retains priority as an explicit override.
- Forced recovery moves the rejected source to the end of the effective order.
- Formatting, compilation, Rust tests, strict Clippy, Python tests, shell checks and release metadata pass in the complete Quality Gate.
- Local Home and `src/browser.rs` remain outside the implementation diff.

Manual validation completed:

- Dialog layout and scrolling were validated at the minimum supported window width.
- Reordering up and down updates the effective-order summary immediately.
- Enable/disable switches work, and the final active source remains protected.
- Reset restores the standard order and enabled state.
- Closing, reopening and restarting Nocky preserve the saved source policy.
- Keyboard operation works on the dialog controls.
- A subsequent YouTube resolution respects the saved order.
- The last-stream diagnostic remains limited to privacy-safe metadata.

The automatic default policy remains reliable without user configuration.

## Phase 10 — Android-parity YouTube Home organization

**Goal:** make the desktop YouTube Home feel structurally aligned with the
Android fork while preserving GTK conventions.

Implemented in PR #46:

- Render YouTube feed chips at the top of the YouTube Home.
- Extract localized chip titles and browse params from the Web InnerTube Home
  response before `ytmusicapi` flattens the page into section rows.
- Selecting a chip loads the corresponding `FEmusic_home` browse params and
  preserves the chip list from the root feed.
- Render each YouTube section using the returned header title, optional label,
  thumbnail shape hint and endpoint.
- Keep section continuation/load-more behavior tied to the selected YouTube
  Home params.
- Treat Quick Picks as a feed/pinned online section, not as a local history
  filter.
- Keep Local Home personalized sections separate from the YouTube Home.
- Add fixture tests for chip extraction, selection request bodies,
  continuation params, section order and header preservation.
- Provide optimistic chip selection, localized loading feedback and explicit
  feedback when YouTube returns unchanged recommendation sections.
- Keep the horizontal scrollbar below the chip controls without overlaying them.

Manual validation completed:

- The connected account returns localized server-provided chips beyond **Tudo**.
- Selecting chips highlights the active choice immediately and displays localized loading feedback in the main Home.
- Filtered responses replace the feed sections; identical server responses produce explicit feedback instead of appearing inert.
- Returning to **Tudo** restores the root feed and preserves the chip list.
- Rapid chip switching keeps the final request and selection.
- Filtered load-more requests retain the selected chip params.
- The horizontal scrollbar remains below the chip controls without overlap at narrow widths.
- Local Home behavior remains unchanged.

Acceptance criteria:

- YouTube Home section headings match the structured feed.
- Chip selection replaces only the feed sections and can return to the root feed.
- Local Home history controls do not appear in YouTube Home.
- Albums, artists, playlists and playable rows keep existing native routing.
- Narrow-window horizontal usability and keyboard activation remain intact.

## Phase 11 — Assisted browser login

**Goal:** reduce manual cookie-copy friction without turning Nocky into a web wrapper.

Planned deliverables:

- Optional isolated WebKitGTK login window.
- Strict navigation allowlist for Google Accounts and YouTube Music.
- Capture only the minimum session data after successful login.
- No permanent JavaScript bridge and no password access.
- Manual import remains available as fallback.

This phase requires a separate privacy and packaging review before implementation.

## Phase 12 — Remote library mutations and account profiles

Planned capabilities:

- Like/unlike feedback in all relevant views.
- Create, rename and edit remote playlists where supported.
- Add/remove playlist tracks with optimistic UI and rollback.
- Account/channel profile selection and clear active-profile indication.

## Phase 13 — Native InnerTube backend research

A direct Rust InnerTube backend remains a research track rather than a forced rewrite. The backend trait is the migration seam. Playback continues to use `yt-dlp + Deno + GStreamer` until signature, `n` and PO-token parity is demonstrated. Endpoints should migrate individually behind fixture-based contract tests while ytmusicapi remains available as fallback.

## Phase 14 — Release hardening and observability

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

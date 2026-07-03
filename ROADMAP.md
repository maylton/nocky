<!-- youtube_likes_source_aware_navigation_roadmap_v2 -->
<!-- personalized_home_privacy_controls_v3 -->
<!-- optional_personalized_home_history_v1 -->
<!-- personalized_home_resume_v2 -->
# Nocky Roadmap

<!-- roadmap_rebaseline_2026_06_24_v2 -->

> Last updated: 2026-07-03
> Status legend: ✅ completed · 🟡 in progress · ⬜ planned  
> Current development priority: **Material 3 Expressive visual-system consolidation**

Nocky is a modern Linux music player built with Rust, GTK4 and Libadwaita,
combining Material 3 Expressive ideas with close integration with the Noctalia
desktop experience.

This roadmap reflects the current implementation rather than only the original
feature plan. Completed foundations remain documented, while partially
implemented areas list only the work still required.

---

## Product principles

- Keep the interface expressive without sacrificing GTK responsiveness.
- Prefer subtle spring, tonal, crossfade and shared-element motion over layout
  scaling.
- Preserve a clear playback hierarchy: play/pause is primary; skip, repeat and
  shuffle are secondary.
- Keep the compact footer focused on track information and utilities.
- Respect reduced-motion preferences throughout the application.
- Keep local-library and YouTube Music behavior clearly separated.
- Keep the completed Android-parity YouTube Music Home aligned with the
  structured feed contract while visual-system work continues.
- Never reserve invisible layout space for collapsed controls.
- Prevent stale asynchronous results after rapid navigation or track changes.
- Validate implementation changes with:
  - `cargo fmt --all`
  - `git diff --check`
  - `cargo check --all-targets --all-features`
  - `cargo test --all-targets --all-features`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `./scripts/quality-gate.sh`, when available

---

# Current foundation

## ✅ Native playback and media integration

- Local playback through the native Rust/GStreamer engine.
- YouTube Music playback integration.
- MPRIS metadata, playback, volume, repeat and shuffle controls.
- Local and YouTube playback sources represented independently.
- Automatic stream recovery infrastructure for temporary YouTube URLs.
- Synchronized play state between the main player and full footer.

## ✅ Adaptive footer architecture

- Full, compact, automatic and hidden footer modes.
- Full footer includes playback transport, progress, source and utilities.
- Compact footer prioritizes metadata, lyrics and volume.
- Automatic mode adapts to the current application surface.
- Hidden controls release their layout allocation.
- Footer implementation is split into focused modules:
  - layout;
  - now playing;
  - transport;
  - progress;
  - utilities;
  - view composition.

## ✅ Material Expressive transport

- PixelPlayer-inspired playback hierarchy.
- Dominant play/pause control.
- Smaller permanent previous and next controls.
- Press compression and state motion.
- Repeat-one and shuffle use shared MD3 toggle geometry.
- Explicit repeat-one `1` badge.
- Consistent order:

  `Shuffle — Previous — Play/Pause — Next — Repeat`

## ✅ WaveSeekBar

- Custom animated wavy progress rendering.
- Precise click and drag seeking.
- Active animation during playback.
- Calm state while paused.
- Material palette integration.
- Traditional progress fallback outside Material Expressive.
- Reduced-motion-compatible behavior.

## ✅ Track-change motion

- Real title, artist and album transitions.
- Staggered metadata entrance.
- Coordinated main-player and footer updates.
- Artwork fade during real cover-to-cover replacement.
- Generation-based cancellation for rapid track changes.
- Reduced-motion fallback.

## ✅ Dynamic Material palette

- Representative color extraction from album artwork.
- Accessible Material tonal-role generation.
- Stable fallback when artwork is unavailable.
- Background extraction outside GTK's main loop.
- Generation protection against stale palette results.
- Smooth interpolation between album palettes.
- Palette application to:
  - player surfaces;
  - footer;
  - WaveSeekBar;
  - repeat and shuffle states;
  - utility surfaces;
  - visualizer.
- Noctalia/Album Aura integration remains compatible with the established
  artwork and palette contract.

## ✅ Core visual and interaction foundations

- Expressive library cards and stable carousel motion.
- Compact expandable volume control.
- Material and Noctalia visual themes.
- Configurable blur behavior.
- English, Portuguese and Spanish interface foundations.
- Stable about, settings and shortcut surfaces.
- Zero-warning quality gate.

---

# Active development

## 1. ✅ Source-aware Home

Local personalized history is implemented, and the YouTube Music Home now uses
YouTube-provided chips, headers, labels, thumbnails and endpoints rather than
locally invented category filters.

### Implemented

- Persistent listening events.
- Separate Local and YouTube history sources.
- Play count, last-played time and total-listening duration.
- Completed-listen signal.
- Protection against counting accidental plays shorter than 30 seconds.
- Background and atomic history writes.
- Ranked artists and albums.
- Recently played album data.
- Home sections based on listening behavior.
- ✅ Suggested playlists, mixtapes and YouTube sync hints hidden from local-only Home.
- YouTube recommendations kept separate from local statistics.
- ✅ YouTube Music Home renders server-provided `home_v2` chips, section
  headers, labels, thumbnails and continuation endpoints.
- ✅ YouTube Music Home preserves the structured feed order while keeping
  Local Home history controls out of the YouTube source.
- ✅ Home V2 artwork now covers raw InnerTube renderer rows, Shorts, live
  sections, mixes and other non-carousel sections rendered by the GTK Home.
- ✅ YouTube Home and playlist first paint no longer block on full artwork
  downloads; cached artwork is reused immediately and fresh covers update
  afterward.

### Remaining follow-ups

- ✅ Unified chronological recent activity for tracks, albums and playlists.
- ✅ Resumable position directly on incomplete track cards.
- ✅ Optional Home visibility control for personalized history sections.
- ✅ First-run explanation and opt-in control for personalized Home history.
- ✅ Recently added local albums ordered by filesystem creation/modification time.
- ✅ Clear-history action.
- ✅ History privacy controls.
- ✅ Per-section empty states.
- Better deduplication when metadata differs only slightly.
- Optional time-window filters for local personalized history, outside the
  primary YouTube feed.
- YouTube Music Home ordering should follow the feed contract, with only small
  Nocky-specific pinned sections when they are clearly source-aware.
- Avoid adding artificial history-window chips to the YouTube Home.

---

## 2. 🟡 Material Expressive visual-system consolidation

The active priority is making Nocky's reusable GTK widgets, loading states and
surface treatments feel coherent with Material 3 Expressive while preserving
Noctalia, Frosted Glass and dynamic album-palette identities.

### Implemented

- ✅ Material Expressive contextual menu and popover surfaces.
- ✅ Material Expressive dialogs and confirmation surfaces.
- ✅ Material Expressive switches, toggles and segmented controls.

### Active checkpoint

- 🟡 Cards, containers and shape hierarchy.

### Planned checkpoints

- Expressive buttons and button-state motion.
- Cards, containers and shape hierarchy.
- Shared navigation transitions.
- Global motion tokens.
- Reduced-motion behavior.
- Accessibility and contrast audit.

---

## 3. 🟡 Richer album, artist and playlist cards

### Implemented

- Material Expressive card surfaces.
- Artwork texture cache.
- Stable edge-aware carousel motion.
- Local and YouTube card variants.
- Listening-based details for artists and albums.
- Navigation from cards to collection pages.
- Current playback information on media rows.
- ✅ Contextual play/pause and overflow actions on supported Home collection cards.
- ✅ Dedicated first-paint placeholder rails for an empty remote YouTube Home.

### Remaining

- ✅ Reusable play/pause and overflow actions are applied to album grids and playlist rows.
- ✅ Collection actions have contextual accessible names and visible keyboard focus.
- Keep artist cards navigation-first until deterministic artist queue resolution is available.
- ✅ Favorite action inside each collection card overflow menu.
- ✅ Skeleton treatment during remote loading is implemented on active collection cards and initial Home placeholder rails.
- 🟡 Type-aware placeholders for missing artwork; palette-derived variants remain planned.
- 🟡 Spring-based animated insertion is implemented for collection grids; animated removal remains planned.
- ✅ Vertical top/end overscroll matches the Home carousel's 520 ms spring timing and responds to wheel, touch and scrollbar dragging.
- ✅ Active album and playlist cards are identified by the contextual play/pause control.
- ✅ Shared collection-card descriptor centralizes local and YouTube album, artist and playlist presentation.

---

## 4. 🟡 Categorized and incremental search

### Implemented

- Separate track, album, artist and playlist result groups.
- Immediate local-library filtering.
- Debounced remote YouTube search.
- Independent incremental limits for each result category.
- Loading, empty and error state foundations.
- Local and YouTube results remain source-aware.
- ✅ Expiring search cache with a 10-minute fresh TTL, one-hour stale-while-revalidate window and bounded LRU eviction.
- ✅ Real per-category remote pagination backed by opaque YouTube Music continuations.
- ✅ Local recent-query history with MRU ordering, individual removal and clear-all controls.
- ✅ Route-aware cancellation for stale YouTube Music search responses.
- ✅ Accessible search-result summaries update after each categorized result rebuild.
- ✅ Search rows, result headings, status banners and pagination controls expose descriptive accessible labels.
- ✅ Relevance-ranked mixed local and remote results while YouTube Music is active.
- ✅ Album and playlist results expose one compact play/pause action.
- ✅ Collection-result rows support arrow navigation and Enter/Space activation.

### Remaining


---

## 5. 🟡 YouTube Music robustness

### Implemented

- Temporary stream URL recovery.
- Retry without manually restarting the track.
- Cached library metadata and artwork.
- Playlist and collection prefetch.
- Concurrent bounded background workers.
- Loading states before slow network requests complete.
- Redacted stream URLs in diagnostic messages.
- User-facing playback error messages.
- Request-generation protection for stale responses.
- Home and playlist first-paint updates that avoid waiting for every browser
  cover download before rendering remote content.

### Remaining

- ✅ Real YouTube Music like/unlike mutations synchronized with the authenticated account.
- ✅ Optimistic like-state updates with rollback when the remote mutation fails.
- ✅ Like/unlike updates persist immediately and reconcile silently with the server after success.
- ✅ Dedicated **Liked songs — YouTube Music** page backed by the synchronized YouTube liked library.
- ✅ Source-aware navigation: show **Local liked songs** only when Local is the active source.
- ✅ Source-aware navigation: hide **Local liked songs** completely when YouTube Music is the active source.
- ✅ Show **Liked songs — YouTube Music** only when YouTube Music is the active source.
- ✅ Never expose liked-song pages from the inactive source in the sidebar or library navigation.
- ✅ Replace the current source-inaccurate “Local liked” title when YouTube Music is active.
- ✅ Localized liked-page titles and empty states in Portuguese, English and Spanish.
- ✅ Clear offline, permission and expired-session feedback for YouTube like mutations.
- ✅ Prevent duplicate remote requests when a user clicks like/unlike repeatedly.
- 🟡 Unit coverage includes error-state classification; broader integration coverage remains planned.
- Preserve the complete edited Queue 2.0 state during stream recovery.
- More explicit handling for unavailable or region-blocked tracks.
- Incremental library synchronization.
- Cache expiration and invalidation rules.
- Offline indicators for cached-only content.
- Retry policy with bounded exponential delay.
- Diagnostics view for connection and runtime problems.

---

## 6. 🟡 Synced lyrics

### Implemented

- Automatic lyric lookup and download.
- Sidecar lyric storage.
- Synchronized line presentation.
- Inline Home lyrics.
- Dedicated lyrics page.
- Manual refresh.
- Automatic download preference.
- Focused-line recentering.

### Remaining

- ✅ Natural centered line-to-line motion with clickable seeking.
- Word-level highlighting when timestamps support it.
- Karaoke presentation mode.
- Manual lyric-version selection.
- Per-track timing offset.
- Persist corrected offsets.
- Better unsynchronized lyric fallback.
- Optional floating lyrics integration with Noctalia.

---

## 7. 🟡 Audio visualizer

### Implemented

- Native visualizer surface.
- Playback-aware activation.
- Material palette integration.
- Visibility preference.
- Stable layout allocation.

### Remaining

- Intensity related to actual audio level.
- Smoother decay during pause and stop.
- Multiple visualization modes.
- Optional compact visualizer.
- Better rhythm response.
- CPU-budget controls.
- Reduced-motion and power-saving simplification.

---

# Next milestone

## 8. ✅ Queue 2.0

Queue 2.0 is implemented across the source-aware data model, playback bridge,
dedicated interface, persistence, shuffle history and recovery foundations.

### Queue data model

- Introduce stable queue-entry IDs independent of library indexes.
- Represent local and YouTube entries through one source-aware queue type.
- Track the current entry explicitly.
- Separate the active queue from browser-visible collections.
- Preserve queue mutations when the library view changes.
- Keep queue invariants testable without GTK.

### Queue operations

- Play next.
- Add to end of queue.
- Remove an individual entry.
- Clear upcoming entries.
- Reorder entries through drag and drop.
- Jump directly to an entry.
- Keep the current track anchored during edits.
- Define repeat and shuffle behavior after manual queue changes.

### Queue interface

- Upgrade the footer queue popover into a dedicated Queue 2.0 view.
- Show artwork, title, artist and source.
- Display a clear current-playing indicator.
- Add drag handles and keyboard reordering.
- Animate insertion, movement and removal.
- Provide empty and end-of-queue states.
- Keep full keyboard and screen-reader operation.

### Persistence and recovery

- Persist queue order and current position.
- Restore local entries safely after a library rescan.
- Restore YouTube entries from cached metadata.
- Ignore missing entries without invalidating the whole queue.
- Preserve edited queue state during temporary stream URL recovery.
- Version the persisted queue schema for future migrations.

### Queue 2.0 completion criteria

- Queue behavior is covered by unit tests.
- Reordering never changes the current track unexpectedly.
- Previous and next follow the edited order.
- Shuffle does not destroy the manually arranged queue.
- Local and YouTube entries can coexist safely.
- Recovery and restart preserve the expected queue state.
- No GTK warnings, Clippy warnings or quality-gate failures.

---

# Planned work

## 9. ⬜ Shared transitions and navigation polish

- Animate artwork from a card into an album or artist header.
- Return artwork toward its original card while navigating back.
- Coordinate collection title, subtitle and artwork motion.
- Use snapshots or overlays where native GTK transitions are insufficient.
- Preserve focus, keyboard navigation and screen-reader context.
- Provide reduced-motion alternatives.

## 10. ⬜ Accessibility, responsiveness and performance

- Complete keyboard navigation across every view and popover.
- Audit focus order and visible focus rings.
- Add accessible names for every icon-only control.
- Announce changing search, queue and playback states.
- Validate dynamic-palette contrast across representative artwork.
- Test narrow windows, HiDPI and fractional scaling.
- Continue eliminating negative GTK allocations.
- Lazy-load artwork and remote sections.
- Profile CPU and memory during long playback sessions.
- Add power-saving behavior for animation-heavy components.

## 11. ⬜ Source-module reorganization

Reorganize the current flat `src/` layout into domain-focused modules after the
0.3.2 reliability work, when feature churn is lower and large file moves will
create less review and merge noise.

### Proposed domains

- `library/`: local models, scanning and indexing.
- `lyrics/`: LRC parsing, providers and synchronization.
- `playback/`: GStreamer engine, recovery and source-specific playback.
- `ui/`: browser, visualizer and reusable presentation modules.
- `youtube/`: bridge, authenticated library and remote metadata.

### Constraints

- Treat the work as a pure refactor with no behavior changes.
- Move one domain at a time in small reviewable pull requests.
- Preserve Queue 2.0 and source-isolated Local/YouTube sessions.
- Preserve public module boundaries where practical.
- Run the complete quality gate after every move.
- Do not cherry-pick the older fork commit directly because the current
  codebase has diverged substantially.

## 12. ⬜ Packaging and release readiness

- Final Flatpak permission review.
- Stable application ID and desktop metadata.
- Complete AppStream metadata.
- Updated screenshots for Material and Noctalia modes.
- Translation review for Portuguese, English and Spanish.
- Configuration, history and queue-schema migrations.
- Release notes and changelog automation.
- Automated CI validation.
- AUR packaging after the Flatpak release path is stable.

---

# Recommended implementation order

1. ✅ Build the source-independent Queue 2.0 data model and tests.
2. ✅ Add queue operations: play next, append, remove and reorder.
3. ✅ Integrate previous, next, repeat and shuffle with the new queue.
4. ✅ Build the dedicated Queue 2.0 interface.
5. ✅ Persist and restore queue state.
6. ✅ Consolidate Material Expressive loading indicators and visual-system primitives.
7. 🟡 Finish card actions and loading placeholders.
8. ✅ Polish dialogs and confirmation surfaces.
9. Harden YouTube unavailable-track and recovery behavior.
10. Resume remote playlist metadata editing, removal and deletion checkpoints.
11. Add shared card-to-page transitions.
12. Reorganize source modules by domain after reliability stabilization.
13. Finish lyrics, visualizer, accessibility and release polish.

---

# Decision log

## Playback hierarchy

- The main player and full footer share one expressive playback language.
- Play/pause remains visually dominant.
- Skip and mode controls remain secondary.
- Compact footer behavior is adaptive and intentionally simpler than the full
  footer.

## PixelPlayer influence

- PixelPlayer inspired the expressive transport hierarchy and WaveSeekBar.
- Nocky implements those ideas natively for GTK rather than copying Android
  implementation details.

## Track transitions

- Metadata must not update as an imperceptible direct text replacement.
- Title, artist and album use coordinated staggered motion.
- Artwork replacement must animate even when both tracks have real covers.
- Rapid track changes invalidate stale animation callbacks.

## Dynamic palette

- Album artwork is the Material palette source.
- Color extraction occurs outside GTK's main loop.
- Every asynchronous result carries generation protection.
- Palette changes interpolate rather than flash.
- Foreground roles remain contrast-safe during interpolation.
- Missing artwork uses a stable fallback palette.

## Queue

- Queue order must be independent from the currently visible browser route.
- Manual ordering must survive navigation, recovery and restart.
- Shuffle must not silently destroy a user-curated order.
- Local and YouTube queue entries share behavior but retain their source.

## Motion

- Prefer reveal, crossfade, spring and shared motion.
- Avoid hover scaling that changes layout allocation.
- Every expressive animation requires a reduced-motion fallback.
- Hidden controls must release all unused allocation.

## Deferred source-module organization

- Organize modules by domain only after the current reliability work is stable.
- Keep the reorganization behavior-neutral and split it into small pull requests.
- Use the older fork only as architectural inspiration, not as a commit source.

## Source separation

- Local history and YouTube recommendations remain separate.
- Local-only mode must not display suggested YouTube playlists or mixtapes.
- Source badges and error states should remain clear to the user.



## Local and YouTube separation

- Local likes and YouTube Music likes are separate concepts and must never share one persisted dataset.
- The liked-songs route, title, empty state and available actions must reflect the active source.
- **Local liked songs** must exist in navigation only while Local is active.
- **Liked songs — YouTube Music** must exist in navigation only while YouTube Music is active.
- Switching sources must rebuild the navigation immediately and remove routes belonging to the previous source.
- YouTube like/unlike actions must mutate the remote account state, not only the local interface.
- Remote failures must be visible and must restore the previous like state when necessary.


- 🟡 YouTube Music mixes now use rich rows with artwork, subtitle, type badge and cached track count.

- ✅ Collection pages now share one modular compact header for local/YouTube albums, artists, playlists and mixes.

- ✅ YouTube artist discography pages now reuse the modular compact collection header with artwork, metadata, album count and track count.

- ✅ Album and artist collection cards now share artwork, subtitle and detail rendering across Home and dedicated library pages.
- ✅ Dedicated artist listings now use the same rich collection-card module as Home and album pages.
- ✅ Playlist creation and maintenance controls were consolidated into a collapsed local playlist manager, keeping navigation as the primary layout.

- ✅ Artist collection cards now use a compact horizontal layout with 56 px circular artwork and condensed metadata.

- ✅ Compact artist cards now reuse the same expressive outline and surface language as the other collection cards.

- ✅ Compact artist rows no longer inherit the large collection-button height, preserving the shared outlined surface without vertical gaps.

- ✅ The Playlists page now uses one bounded outer scroller instead of letting the playlist list and local manager dictate the window minimum size.
- ✅ Playlist navigation, mixes and local management now live in one compact responsive page flow that remains resizable after opening.

- ✅ Playlist rows now constrain title, subtitle and metadata widths so the Playlists page cannot raise the application minimum width.
- ✅ Mix rows now prefer the first cached track artwork over malformed or text-heavy mix thumbnails, with the mix image retained as fallback.

- ✅ YouTube artist pages now compose a rich header, up to five popular tracks and the existing album grid using shared collection modules.
- ✅ Popular tracks on artist pages are directly playable with the visible selection preserved as the playback queue.

- ✅ Artist pages now fall back to the synchronized YouTube catalog when the artist collection cache has no tracks, so the featured-tracks section is actually visible whenever matching songs exist.
- ✅ The artist section was renamed from Popular Tracks to Featured Tracks because no real popularity ranking is available yet.

- ✅ Featured-track rows on artist pages now preserve the same 16 px corner radius during hover, focus and press states.

- ✅ Featured-track rows reuse the expressive search-result surface and hover colors while keeping a stable 16 px corner radius.

- ✅ Track credits with explicit collaboration separators are now indexed under each credited artist in local and YouTube artist pages.
- ✅ Artist filtering, featured tracks and album subtitles share one central credit parser; bare ampersands remain intact for band names.

- ✅ Personalized Home artist rankings now split collaborative track credits and count each credited artist independently.
- ✅ Existing listening-history events are reinterpreted at display time, so no history migration or reset is required.

- ✅ Track overflow menus in playlists, albums and YouTube mixes now include Go to artist and Go to album actions when metadata is available.
- ✅ Collaborative credits use the primary credited artist for direct navigation, while missing metadata hides the corresponding action.

- ✅ Personalized artist cards now prefer dedicated artist profiles or solo-track artwork instead of sharing a collaboration cover.
- ✅ Ranked Home artist cards no longer inherit the legacy album-count subtitle; they show only listening statistics and artist-specific metadata.
- ✅ When no artist-specific image exists, the generic artist placeholder is used instead of a misleading collaborative cover.

- ✅ Opening Artists now revalidates up to 36 YouTube artist profiles, including split artists that must first be resolved by title.
- ✅ Opening an individual artist always refreshes its profile and discography while keeping cached content visible.
- ✅ Confirmed artwork is shared by the Artists grid, personalized Home cards and the artist page header/summary.
- ✅ Bulk profile refreshes trigger one final browser rebuild; when Home is open, that rebuild uses the existing Home GtkStack crossfade instead of producing one crossfade per artist.

- ✅ Artist profile confirmation now refreshes only the header, summary and featured tracks when the cached discography has not changed.
- ✅ Album cards are rebuilt only when the artist discography actually changes, eliminating the visible flash caused by clear-and-recreate refreshes.
- ✅ Navigation between Artists and artist detail uses a stable crossfade instead of the one-pixel GtkStack slide allocation that produced GtkButton size warnings.
- ✅ The personalized Home keeps its existing dedicated crossfade behavior.

- ✅ The Artists page now renders “Load more artists” as a compact artist-list row instead of an oversized album-style collection card.
- ✅ The control reuses the page’s circular artwork surface, typography, outline and row spacing for a visually consistent result.

- ✅ The main player artist and album metadata are now clickable navigation targets with localized tooltips and pointer affordance.
- ✅ Local playback opens the matching local artist or album route, while YouTube playback reuses cached collection identifiers or resolves the collection by title when necessary.
- ✅ Collaborative credits navigate through the primary credited artist, matching the track overflow-menu behavior.

- ✅ YouTube album and artist routes now carry a stable collection key, preferring browse IDs and falling back to title + artist for albums.
- ✅ Version 5 collection caches are migrated in place to the new stable key format.
- ✅ Background cache updates are coalesced by a dedicated debounced writer, keeping JSON serialization and disk I/O away from GTK's main loop.
- ✅ Collection identity tests cover browse-ID priority and duplicate album titles.

- ✅ Artist overview responses now canonicalize the open route to the resolved browse-ID key and migrate any temporary title-keyed cache entries.
- ✅ An open artist page refreshes directly instead of rebuilding the entire browser, so footer progress sliders are not transiently compressed during background updates.
- ✅ Album cards keep their current presentation during background confirmation and do not replay the entrance spring, eliminating the visible page flash.

- ✅ Opening Artists now revalidates the same alphabetically sorted entries that are actually visible, instead of an unrelated fixed unsorted group of 36.
- ✅ Loading more artists automatically schedules profile confirmation for the newly revealed batch.
- ✅ Resolved profiles update the exact requested collection entry by stable key, even when the returned display title differs slightly.
- ✅ Batch completion rebuilds only the Artists grid without replaying card entrance animations; Home continues using its own crossfade.

- ✅ Added the minimal LocalArtistIndex foundation without changing the Artists page, YouTube artist cache, routes, profile refresh, grid rendering or animations.
- ✅ Production integration remains limited to ranked local-artist artwork and preserves the former first-solo-track behavior exactly.
- ✅ The index contains no unused future-facing fields, keeping clippy -D warnings clean.
- ✅ Added regression tests for collaboration credits, ampersand band names, normalized identity and first-solo-track artwork semantics.

- ✅ Artist cards now use cached profile artwork immediately even when an older title-keyed cache entry has not yet been migrated to the current browse-ID key.
- ✅ The canonical key remains the fast path; browse ID and normalized display title are read-only fallbacks before album artwork is considered.
- ✅ The same lookup is shared by Artists, regular Home artist cards and ranked Home artist artwork without changing revalidation or grid refresh behavior.

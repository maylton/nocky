# Nocky Roadmap

> Last updated: 2026-06-24  
> Status legend: ✅ completed · 🟡 in progress · ⬜ planned

Nocky is a modern Linux music player built with Rust, GTK4 and Libadwaita,
combining Material 3 Expressive ideas with close integration with the
Noctalia desktop experience.

This roadmap records both the planned work and the visual decisions already
made, so future changes remain consistent.

---

## Product principles

- Keep the interface expressive without sacrificing GTK responsiveness.
- Prefer subtle spring, tonal and shared-element motion over simple scaling.
- Preserve a clear visual hierarchy: play is primary; mode controls are
  secondary.
- Keep the compact footer focused and free from transport controls.
- Respect reduced-motion preferences.
- Keep local-library and YouTube Music behavior clearly separated.
- Avoid UI elements that reserve invisible layout space.
- Validate every implementation with:
  - `cargo fmt --all`
  - `git diff --check`
  - `cargo check --all-targets --all-features`
  - `cargo test --all-targets --all-features`
  - `cargo clippy --all-targets --all-features -- -D warnings`

---

## Current foundation

### ✅ Footer modes

- Complete, compact and hidden footer modes.
- Compact mode keeps track information, lyrics and volume utilities.
- Compact mode intentionally hides transport controls and progress.
- Home pages use the compact presentation.
- Pages outside Home may use the complete footer.
- Manual Complete / Compact / Hidden modes remain available as overrides.

### ✅ Compact volume utility

- Volume button expands a Material 3-styled slider to the right.
- Custom-drawn MD3 slider avoids `GtkScale` allocation warnings.
- Slider supports click, drag and mouse-wheel interaction.
- Light spring motion on expand and collapse.
- Collapsed state releases hidden layout width.
- Rounded capsule geometry aligned with circular utility buttons.
- Full-footer volume button retains mute/unmute behavior.

### ✅ Home card motion

- Horizontal carousel cards use edge-aware spring motion.
- Hover growth that destabilized layout was removed.
- Motion is designed to avoid clipping and card displacement.

---

### ✅ Repeat and shuffle MD3 toggles

- Shared 40 px circular geometry in the main player and complete footer.
- Transparent inactive state and tonal active state.
- Explicit repeat-one `1` badge.
- Consistent order: Shuffle — Previous — Play/Pause — Next — Repeat.
- Hidden from compact footer mode.

---

# Development phases

## 1. 🟡 PixelPlayer-inspired playback system

This phase replaces the older generic proposal to merely “unify the main
playback buttons.”

The agreed direction is a shared PixelPlayer-inspired playback language for
the main player and the complete footer.

### Expressive play controls

- Bind the main play button and complete-footer play button to the same state.
- Shared states:
  - `Idle`
  - `Playing`
  - `Paused`
- Main-player play button remains clearly dominant over skip and mode controls.
- Previous and next use smaller permanent tonal circular buttons.
- Play/pause uses a larger, softer and slightly elevated treatment.
- Add subtle press compression and state transition motion.
- Keep the compact footer outside this transport-control system.

### PixelPlayer-style WaveSeekBar

- Replace the conventional progress presentation with a custom wavy fill.
- Progress follows:

  `progress_x = width × (position / duration)`

- Wave shape follows a sinusoidal path based on frequency, amplitude and phase.
- Animate the wave while music is playing.
- Calm or stop the wave while paused.
- Slightly increase wave response while dragging.
- Derive active colors from the current album palette.
- Keep seeking precise despite the expressive rendering.
- Provide a reduced-motion fallback with a stable rounded track.

### ✅ Repeat and shuffle toggles

- Transparent background while inactive.
- Circular tonal state layer on hover.
- Tonal filled circle only while active.
- Active icon uses the appropriate primary-container foreground.
- Repeat-one displays a small, clear `1` indicator.
- Keep repeat and shuffle hidden in compact footer mode.
- Preserve the order:

  `Shuffle — Previous — Play/Pause — Next — Repeat`

---

## 2. 🟡 Track-change transitions and dynamic palette

### ✅ Metadata and artwork motion

- Crossfade or gently slide the title when the track changes.
- Introduce the artist text with a slight stagger.
- Transition cover art with controlled opacity and scale.
- Reset progress smoothly without flashing.
- Coordinate the main player and complete footer transitions.

### Dynamic Material palette

- Extract representative colors from album artwork.
- Generate accessible Material 3 tonal roles.
- Animate between the previous and next palettes.
- Apply colors consistently to:
  - player surfaces;
  - footer;
  - WaveSeekBar;
  - active toggles;
  - utility cards;
  - visualizer.
- Provide a stable fallback when artwork is unavailable.
- Refine automatic synchronization with Noctalia theme templates.

---

## 3. ⬜ Personalized Home

Use listening history instead of generic static recommendations.

### Local library

- Recently played tracks.
- Most-played artists.
- Most-played albums.
- Continue listening.
- Recently added music.
- Do not show suggested playlists or mixtapes while the local source is active.

### YouTube Music

- Recently played.
- Personalized albums and artists.
- Relevant playlists and mixes.
- Continue listening across sessions.
- Keep YouTube-derived sections separate from local statistics.

### History infrastructure

- Record play count, last-played date and completed-listen signals.
- Avoid counting accidental very short plays.
- Store history efficiently.
- Allow history clearing and privacy controls.

---

## 4. ⬜ Richer album, artist and playlist cards

- Current-playing indicator.
- Contextual play button.
- Contextual favorite action.
- Quick overflow menu.
- Skeleton placeholders during loading.
- Palette-based placeholders for missing artwork.
- Smooth insertion and removal animations.
- Preserve edge-spring behavior without layout shifts.

---

## 5. ⬜ Shared transitions and navigation polish

- Animate an album cover from its card into the album header.
- Return it toward its previous position when navigating back.
- Coordinate title, artist and artwork motion.
- Use overlays and snapshots where native GTK transitions are insufficient.
- Respect reduced-motion settings.
- Preserve keyboard and screen-reader navigation during transitions.

---

## 6. ⬜ Queue 2.0

- Reorder tracks by drag and drop.
- Remove individual queue entries.
- “Play next.”
- “Add to end of queue.”
- Clear current-playing indication.
- Display local or YouTube source.
- Persist and restore the queue between sessions.
- Animate item insertion, movement and removal.
- Preserve queue state when recovering an expired stream URL.

---

## 7. ⬜ Categorized and incremental search

- Separate results into:
  - tracks;
  - albums;
  - artists;
  - playlists.
- Immediate local-library search.
- Debounced remote YouTube search.
- Incremental loading and pagination.
- Search-result cache.
- Clear loading, empty and error states.
- Keyboard-first result navigation.

---

## 8. ⬜ YouTube Music robustness

- Refresh expired temporary playback URLs automatically.
- Retry playback without forcing the user to restart the track.
- Cache metadata and artwork.
- Recover gracefully from unavailable tracks.
- Incrementally synchronize library changes.
- Avoid reloading entire collections unnecessarily.
- Preserve playback and queue state during recovery.
- Surface useful errors without exposing implementation details.

---

## 9. ⬜ Synced lyrics improvements

- More natural transitions between lyric lines.
- Word-level highlighting when timestamps are available.
- Karaoke-style view.
- Manual lyric-version selection.
- Per-track offset correction.
- Save corrected offsets.
- Better fallback for unsynchronized lyrics.
- Optional floating lyrics integrated with the desktop shell.

---

## 10. ⬜ Audio visualizer refinement

- Colors derived from album artwork.
- Intensity related to actual playback volume.
- Smooth decay while pausing.
- Multiple visualization modes.
- Optional compact visualization.
- Better rhythm response with controlled CPU use.
- Disable or simplify under reduced-motion or power-saving settings.

---

## 11. ⬜ Accessibility, responsiveness and performance

- Full keyboard navigation.
- Reliable focus rings and focus order.
- Screen-reader labels for every icon-only control.
- Reduced-motion support for springs, waves and shared transitions.
- High-contrast validation for dynamic palettes.
- Narrow-window and HiDPI testing.
- Avoid negative GTK size allocations.
- Lazy-load artwork and remote sections.
- Profile CPU and memory use during long playback sessions.

---

## 12. ⬜ Packaging and release readiness

- Final Flatpak permissions review.
- Stable application ID and desktop metadata.
- AppStream metadata and screenshots.
- Translation completion for Portuguese, English and Spanish.
- Migration handling for configuration and history schema changes.
- Release notes and changelog.
- Automated CI validation.
- AUR packaging after the Flatpak release path is stable.

---

# Recommended implementation order

1. Finish repeat and shuffle toggle states.
2. Complete PixelPlayer-inspired play-button synchronization.
3. Implement the WaveSeekBar.
4. Add track-change and palette transitions.
5. Build the personalized-history Home.
6. Upgrade the queue.
7. Implement categorized incremental search.
8. Improve YouTube playback recovery.
9. Add shared card-to-page transitions.
10. Finish lyrics, visualizer, accessibility and release polish.

---

# Decision log

## Playback hierarchy

- The main player and complete footer share one expressive playback language.
- The main play button is visually dominant.
- The compact footer intentionally has no transport controls.

## PixelPlayer influence

- PixelPlayer replaces the earlier generic play-button proposal.
- Its influence covers both the synchronized expressive controls and the
  animated wavy progress presentation.
- Nocky adapts the concept to GTK rather than copying Android implementation
  details.

## Repeat and shuffle

- Inactive: transparent.
- Active: tonal circular container.
- Repeat-one must have an explicit `1` indicator.
- These controls remain secondary to play/pause.

## Motion

- Prefer edge spring, reveal, crossfade and shared motion.
- Avoid hover scaling that changes layout allocation.
- Every expressive animation must have a reduced-motion fallback.

## Compact footer

- Keep track information and utilities.
- Hide transport controls and progress.
- Lyrics and volume remain accessible.
- Expandable controls must release all hidden layout space when collapsed.

# Rust UI Refactor — Phase 3C

## Scope

Extraction of the visual footer now-playing component from `main.rs`.

## New module

- `src/footer_now_playing.rs`

## Extracted construction

- footer artwork placement;
- track title;
- artist;
- favorite action and icon;
- source badge;
- metadata rows;
- now-playing button and CSS classes.

## Preserved in `AppController`

- `CoverView` creation and ownership;
- click callbacks;
- favorite-state updates;
- local and YouTube metadata updates;
- queue dialog behavior;
- footer transport, progress and volume;
- mode and responsive-layout application.

## Tests

The module verifies that the three translated strings required during
construction exist for Portuguese, English and Spanish.

## Non-goals

This phase does not alter widget hierarchy, CSS classes, dimensions, labels,
callbacks or runtime behavior. It does not yet extract the transport, progress
or volume construction.

Marker: `nocky_rust_ui_phase3c_footer_now_playing_v1`.

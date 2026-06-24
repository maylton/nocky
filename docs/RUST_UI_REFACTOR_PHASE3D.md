# Rust UI Refactor — Phase 3D

## Scope

Extraction of the complete-footer transport construction from `main.rs`.

## New module

- `src/footer_transport.rs`

## Extracted construction

- Shuffle toggle;
- previous button;
- play/pause button and icon;
- next button;
- repeat-one toggle;
- footer `ExpressiveTransport`;
- transport container and CSS classes.

## Preserved in `AppController`

- playback callbacks;
- local and YouTube queue navigation;
- play/pause state synchronization;
- repeat and shuffle state;
- theme-dependent effect decision;
- footer mode and responsive policy;
- progress, volume and lyrics utilities.

## Tests

The module verifies that every transport tooltip required during construction
exists for Portuguese, English and Spanish.

## Non-goals

This phase does not alter order, icons, tooltips, CSS classes, spacing,
expressive-motion parameters or runtime behavior.

Marker: `nocky_rust_ui_phase3d_footer_transport_v2`.

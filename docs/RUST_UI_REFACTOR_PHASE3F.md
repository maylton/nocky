# Rust UI Refactor — Phase 3F

## Scope

Extraction of the footer utility and compact-volume construction from
`main.rs`.

## New module

- `src/footer_utilities.rs`

## Extracted construction

- lyrics toggle;
- mute button and icon;
- GTK scale used as the volume model;
- custom Material 3 volume canvas;
- fixed clipping slot;
- right-sliding revealer;
- right-side utility group.

## Preserved in `AppController`

- mute and unmute callbacks;
- previous-volume state;
- volume model callbacks and MPRIS synchronization;
- compact expanded/collapsed state;
- child-revealed callback;
- theme gating;
- spring animation and generation cancellation;
- footer mode and adaptive layout.

## Tests

The module freezes translated utility copy, compact-volume geometry, reveal
duration, group spacing and volume step.

## Non-goals

This phase does not alter widget order, CSS classes, dimensions, clipping,
reveal direction, animation duration, interaction model or runtime behavior.

Marker: `nocky_rust_ui_phase3f_footer_utilities_v1`.

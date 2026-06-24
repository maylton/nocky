# Rust UI Refactor — Phase 3E

## Scope

Extraction of the footer progress-area construction from `main.rs`.

## New module

- `src/footer_progress.rs`

## Extracted construction

- Material `WaveProgress`;
- traditional GTK scale;
- Classic/Material progress stack;
- crossfade duration and page names;
- elapsed and duration labels;
- progress row;
- central footer container joining transport and progress.

## Preserved in `AppController`

- seek callbacks;
- elapsed and duration updates;
- playback-position synchronization;
- theme-dependent Classic/M3 selection;
- transport ownership and callbacks;
- footer mode and responsive-layout application.

## Tests

The module freezes the stack page names, crossfade duration, initial time text
and approved center geometry.

## Non-goals

This phase does not alter progress rendering, seeking, timing, CSS classes,
dimensions, stack page names or runtime behavior.

Marker: `nocky_rust_ui_phase3e_footer_progress_v1`.

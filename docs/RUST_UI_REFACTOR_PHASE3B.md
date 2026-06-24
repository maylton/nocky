# Rust UI Refactor — Phase 3B

## Scope

Extraction of the footer mode and responsive-layout policy from `main.rs`.

## New module

- `src/footer_layout.rs`

## Extracted policy

- Automatic mode resolution;
- Full, Compact and Hidden geometry;
- CSS mode-class selection;
- adaptive breakpoints at 1040 px and 790 px;
- wide, medium and narrow visibility plans.

## Preserved in `AppController`

- every GTK widget mutation;
- compact-volume state and animation;
- route and Home-player visibility detection;
- footer callbacks;
- CSS classes and widget hierarchy.

## Added tests

- Automatic mode behavior;
- explicit Hidden behavior;
- approved Full and Compact geometry;
- exact responsive breakpoint boundaries;
- medium and narrow control visibility.

## Non-goals

This phase does not move footer construction or callbacks and does not alter
CSS, dimensions, visibility rules or runtime behavior.

Marker: `nocky_rust_ui_phase3b_footer_layout_policy_v1`.

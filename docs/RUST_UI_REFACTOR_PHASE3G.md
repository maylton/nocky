# Rust UI Refactor — Phase 3G

## Scope

Final visual assembly of the footer in a dedicated module.

## New module

- `src/footer_view.rs`

## Composed components

- now-playing card;
- playback transport;
- progress area;
- utility and compact-volume controls;
- root `GtkCenterBox`.

## Main.rs after this phase

`main.rs` creates the cover model, calls one footer builder, preserves the
compact-volume `child-revealed` callback, appends the root widget and connects
all application behavior.

## Preserved in `AppController`

- every playback callback;
- metadata and favorite updates;
- local and YouTube navigation;
- seek and timing synchronization;
- volume and mute state;
- MPRIS integration;
- compact expansion and spring motion;
- footer mode and responsive-layout application.

## Tests

The assembly module freezes the approved root height and root CSS class
contract.

## Non-goals

This phase does not alter widget order, CSS classes, geometry, callbacks,
motion, themes, MPRIS behavior or application state.

Marker: `nocky_rust_ui_phase3g_footer_view_assembly_v1`.

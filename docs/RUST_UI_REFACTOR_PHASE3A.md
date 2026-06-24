# Rust UI Refactor — Phase 3A

## Scope

Mechanical extraction of compact-footer volume spring motion from `main.rs` using an exact struct-boundary transformation.

## Extracted module

- `src/compact_volume_motion.rs`

## Moved without behavioral changes

- `CompactVolumeSpring`;
- `run_compact_volume_spring`;
- spring width calculation;
- cubic easing functions;
- linear interpolation helper.

## Preserved in `AppController`

- theme gating;
- reduced-motion gating;
- `GtkRevealer` state;
- visibility timing;
- expanded/collapsed target widths;
- CSS state-class ownership;
- the existing call site and all arguments.

## Added tests

- initial expansion width;
- final target width;
- expansion overshoot;
- collapse rebound;
- safe 96 px minimum width.

## Non-goals

This phase does not alter CSS, timings, amplitudes, geometry, settings or
footer behavior.

Marker: `nocky_rust_ui_phase3a_compact_volume_motion_v2`.

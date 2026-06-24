# Rust UI Refactor — Phase 3 Closure

<!-- nocky_rust_ui_phase3_closure_v1 -->

## Status

Phase 3 is complete and visually approved.

## Extracted modules

- `src/compact_volume_motion.rs`
- `src/footer_layout.rs`
- `src/footer_now_playing.rs`
- `src/footer_transport.rs`
- `src/footer_progress.rs`
- `src/footer_utilities.rs`
- `src/footer_view.rs`

## Final footer behavior

### Full

- metadata card fills the available footer height;
- artwork tracks the card allocation;
- roomier metadata hierarchy;
- progress, transport and utilities remain responsive.

### Compact

- metadata card stays vertically centered;
- four pixels of vertical breathing room are preserved;
- artwork remains compact;
- utility and volume controls share the intended optical baseline.

## Preserved ownership

`AppController` remains responsible for:

- playback and navigation callbacks;
- local and YouTube state;
- MPRIS synchronization;
- seeking and timing updates;
- favorite and metadata updates;
- mute and volume state;
- compact volume reveal and spring motion;
- footer mode and adaptive-layout application.

## Validation gates

The closure workflow requires:

- CSS structural audit;
- `cargo fmt --all`;
- `git diff --check`;
- `cargo check --all-targets --all-features`;
- `cargo test --all-targets --all-features`;
- `cargo clippy --all-targets --all-features -- -D warnings`;
- visual approval in Material Expressive and Noctalia;
- visual approval of Full, Compact, Automatic and Hidden modes.

## Merge strategy

The feature branch is merged into `main` through a fast-forward-only update.
The feature branch is retained after the merge as a temporary recovery point.

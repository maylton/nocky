# CSS Refactor — Phase 2A

## Scope

Consolidation of the approved PixelPlayer-inspired expressive transport CSS.
No Rust behavior or fixed-slot geometry was changed.

## Result

| Metric | Before | After | Change |
|---|---:|---:|---:|
| Transport CSS lines | 355 | 206 | -149 |
| Transport CSS bytes | 12042 | 6639 | -5403 |
| Versioned transport markers | 6 | 1 semantic phase marker | — |

## Preserved behavior

- circular resting Play/Pause state;
- 24 px main-player playing radius;
- 19 px footer playing radius;
- click-only primary interaction;
- no primary focus/click glow;
- 18 px main secondary radius;
- 16 px footer secondary radius;
- existing secondary hover, active and keyboard-focus treatment;
- Rust-owned geometry animation.

## Removed historical layers

- `pixel_player_expressive_transport_v1`
- `pixel_player_transport_click_only_v6`
- `pixel_player_transport_detach_legacy_main_glow_v7`
- `pixel_player_transport_no_primary_click_glow_v4`
- `pixel_player_transport_one_second_v3`
- `pixel_player_transport_remove_cross_glow_v5`

## Validation

- balanced CSS blocks;
- modular byte-count test updated;
- `scripts/audit_css.py --check`;
- full Rust format/check/test/clippy gate;
- visual runtime comparison still required.

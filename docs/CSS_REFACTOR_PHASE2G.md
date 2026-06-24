# CSS Refactor — Phase 2G

## Scope

Removal of the alternate Home-card motion engine that was compiled but
permanently guarded by `if false`.

## Result

| Metric | Before | After | Change |
|---|---:|---:|---:|
| Total CSS lines | 3863 | 3757 | -106 |
| Total CSS bytes | 111857 | 109025 | -2832 |
| Removed Rust modules | — | 1 | `home_card_motion.rs` |
| Removed disabled CSS rules | — | 11 | -11 |
| Removed disabled declarations | — | 31 | -31 |
| Verified active cascades | — | 10 | unchanged |

## Removed Rust path

- `mod home_card_motion`;
- `HomeCardKind`;
- unused card-kind classification;
- `if false` alternate installation branch;
- `src/home_card_motion.rs`.

## Removed CSS path

- `expressive-home-card-button`;
- `expressive-home-card-slot`;
- `expressive-home-card-motion`;
- `expressive-home-media-artwork`;
- `expressive-home-artist-artwork`;
- `expressive-home-mix-card`;
- `is-hovered` and `is-clicking` states belonging to that retired component.

## Preserved active behavior

- `home-card-no-hover-scale`;
- visual hover without scale;
- carousel edge spring;
- `home-card-edge-spring`;
- `home-card-edge-spring-surface`;
- Rust-owned width animation;
- approved card geometry, color and shadow states.

## Validation

- exact disabled-branch preflight;
- active producer preflight;
- balanced CSS;
- active-selector cascade comparison;
- absence of executable retired Home-motion constructs and the exact alternate branch after mutation;
- CSS audit;
- complete Rust format/check/test/clippy gate;
- visual runtime comparison required.

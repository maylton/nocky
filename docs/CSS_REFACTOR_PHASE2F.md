# CSS Refactor — Phase 2F

## Scope

Extraction of the complete compact-footer volume presentation from the Home and
footer metadata modules into one semantic component module.

## Result

| Metric | Before | After | Change |
|---|---:|---:|---:|
| Total CSS lines | 3900 | 3863 | -37 |
| Total CSS bytes | 112763 | 111857 | -906 |
| CSS modules | 12 | 13 | +1 semantic component |
| Volume presentation locations | 2 modules | 1 module | -1 location |
| Removed fully superseded rules | — | 4 | -4 |
| Removed dead declarations | — | 8 | -8 |
| Verified selector cascades | — | 22 | unchanged |

## New boundary

- `080-home-browser.css`: carousel and Home cards only;
- `085-compact-volume.css`: complete compact-volume presentation;
- `090-footer.css`: footer metadata transition presentation.

## Pruned safely

Only complete single-selector rules whose every declaration was redefined by a
later rule for the same exact selector were removed.

## Preserved

- Rust-owned spring width animation;
- compact collapsed and expanded geometry;
- custom MD3 volume canvas;
- hover and focus states;
- capsule geometry and optical alignment;
- current footer and Home card visuals;
- final declarations for every moved selector.

## Validation

- semantic module header added after dead-rule pruning;
- deterministic marker boundaries;
- balanced CSS;
- exact-selector cascade comparison before and after;
- ordered module registration;
- module-boundary assertions;
- CSS audit;
- complete Rust format/check/test/clippy gate;
- visual runtime comparison required.

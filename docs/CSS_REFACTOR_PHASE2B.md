# CSS Refactor — Phase 2B

## Scope

Consolidation of Repeat and Shuffle controls shared by the main player and the
complete footer.

## Result

| Metric | Before | After | Change |
|---|---:|---:|---:|
| Total Material Expressive CSS lines | 4026 | 3979 | -47 |
| Total Material Expressive CSS bytes | 116833 | 114987 | -1846 |
| CSS modules | 10 | 11 | +1 semantic controls module |
| Toggle presentation locations | 4 modules | 1 module | -3 locations |

## Preserved behavior

- shared Repeat/Shuffle widget factory;
- repeat-one badge;
- Material Expressive inactive, hover, active and checked states;
- Noctalia hover and checked states;
- approved main-player geometry;
- approved footer geometry;
- transparent inactive surfaces;
- tonal active surfaces.

## Structural improvements

- created `095-controls.css`;
- removed obsolete toggle presentation from foundation, footer and home modules;
- detached playback toggles from compact-volume CSS;
- replaced the patch marker in `mode_toggle.rs` with module documentation;
- retained context classes only for genuine geometry differences.

## Validation

- balanced CSS in every edited module;
- no legacy toggle rules outside `095-controls.css`;
- modular byte-count test updated;
- CSS audit;
- complete Rust format/check/test/clippy gate;
- visual runtime comparison required.

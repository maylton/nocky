# CSS Refactor — Phase 2D

## Scope

Removal of footer declarations that were already superseded by later,
same-selector refinements in the Material Expressive cascade.

## Result

| Metric | Before | After | Change |
|---|---:|---:|---:|
| Total CSS lines | 4000 | 3913 | -87 |
| Total CSS bytes | 115329 | 112999 | -2330 |
| Removed complete rules | — | 12 | -12 |
| Removed dead declarations | — | 62 | -62 |
| Verified selector cascades | — | 25 | unchanged |

## Pruned areas

- footer outer geometry from the original foundation layer;
- footer metadata-card geometry superseded by compact refinements;
- artwork, title, artist and source-pill sizing;
- center and transport geometry;
- primary and skip-control sizing;
- early progress-row sizing;
- early utility-group and utility-action geometry;
- obsolete classic volume-track rules;
- first-generation compact-footer geometry.

## Preserved

- final computed declarations for every affected exact selector;
- colors and tonal roles;
- borders and outline opacity;
- shadows;
- state-specific behavior;
- compact-footer left/right padding inherited by later rules;
- Rust-owned animation and allocation.

## Validation

- structural selector parser;
- declaration-level preflight;
- shorthand-aware final-cascade comparison before and after;
- balanced CSS;
- CSS audit;
- complete Rust format/check/test/clippy gate;
- visual runtime comparison required.

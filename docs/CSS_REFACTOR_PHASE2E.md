# CSS Refactor — Phase 2E

## Scope

Correction of the architectural boundary between the player and Home CSS
modules.

## Result

| Metric | Before | After | Change |
|---|---:|---:|---:|
| Total CSS lines | 3913 | 3900 | -13 |
| Total CSS bytes | 112999 | 112763 | -236 |
| Misplaced component blocks | 2 | 0 | -2 |
| Removed obsolete rules | 1 | 0 | -1 |
| Verified selector cascades | — | 13 | unchanged |

## Moved to `080-home-browser.css`

- Home carousel horizontal scrollbar;
- scrollbar trough;
- Material queue-inspired thumb;
- scrollbar hover state.

## Moved to `070-player.css`

- main-player Play/Pause icon compatibility reset;
- normal, hover, active, focus and focus-visible image states.

## Removed

The first carousel hover rule used `alpha(@m3_primary, 0.62)`, but was entirely
superseded by the later Material queue-thumb hover rule before any other
selector could use it.

## Preserved

- exact final declarations of all moved selectors;
- ExpressiveTransport geometry and state classes;
- Home card motion and edge-spring behavior;
- carousel dimensions and thumb appearance;
- every Rust animation and allocation rule.

## Validation

- deterministic marker boundaries;
- balanced CSS;
- exact-selector final-declaration comparison;
- module-boundary assertions;
- CSS audit;
- complete Rust format/check/test/clippy gate;
- visual runtime comparison required.

# CSS Refactor — Phase 2C

## Scope

Consolidation of identical tonal color roles used by main-player metadata,
footer now-playing metadata and favorite actions.

## Result

| Metric | Before | After | Change |
|---|---:|---:|---:|
| Total CSS lines | 3979 | 4000 | +21 |
| Total CSS bytes | 114987 | 115329 | +342 |
| CSS modules | 11 | 12 | +1 semantic tonal module |
| Shared palette declarations | duplicated | centralized | one source |

## Centralized

- metadata foreground;
- metadata primary-container surface;
- track-title foreground;
- favorite-action resting foreground and background;
- favorite-action hover background.

## Preserved locally

- width and height;
- padding and margins;
- border radius;
- borders and shadows;
- footer-card hover shade;
- typography size and weight;
- every animation and transition.

## Validation

- exact rule/property preflight;
- balanced CSS;
- no duplicated moved declarations in `000-foundation.css`;
- ordered module registration;
- CSS audit;
- complete Rust format/check/test/clippy gate;
- visual runtime comparison required.

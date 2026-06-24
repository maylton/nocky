# CSS Refactor — Phase 2H

## Scope

Removal of an orphan metadata-transition implementation consisting of one
unregistered Rust module and one dedicated CSS module.

## Result

| Metric | Before | After | Change |
|---|---:|---:|---:|
| Total CSS modules | 13 | 12 | -1 |
| Total CSS lines | 3757 | 3721 | -36 |
| Total CSS bytes | 109025 | 108383 | -642 |
| Removed Rust files | — | 1 | `track_transition.rs` |
| Removed Rust lines | — | 108 | -108 |
| Removed CSS rules | — | 7 | -7 |

## Removed

- `src/track_transition.rs`;
- `assets/themes/material-expressive/090-footer.css`;
- the `090-footer.css` loader entry.

## Why it was safe

- `track_transition.rs` was not registered by any `mod track_transition;`;
- no other Rust module referenced `MetadataTransition`;
- no declarative UI used the CSS classes;
- the active player uses direct `set_text()` calls in
  `PlayerViewHandle::set_metadata()`.

## Preserved

- immediate title updates;
- immediate artist updates;
- immediate album updates;
- current cover updates;
- player and footer geometry;
- all remaining CSS bytes and module order.

## Validation

- Rust module reachability audit;
- external-reference audit;
- declarative UI class audit;
- exact CSS selector-set validation;
- exact CSS byte and line delta validation;
- CSS audit;
- complete Rust format/check/test/clippy gate;
- visual runtime comparison required.

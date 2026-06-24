# CSS Architecture

## Purpose

The Material Expressive theme was migrated from one large CSS file to an
ordered set of modules in `assets/themes/material-expressive/`.

Phase 1 is deliberately mechanical. The generated modules concatenate to the
exact bytes of the former monolith, so selector order and cascade behavior do
not change.

## Loading

`src/theme_css.rs` owns the ordered list of embedded CSS modules.
`VisualThemeManager` receives one combined string at startup.

Do not load individual modules through separate `CssProvider` instances.
Separate providers can change cascade behavior because provider priority and
insertion order become part of the result.

## Module naming

Files use a numeric prefix followed by the dominant responsibility detected in
that contiguous section. The numeric prefix is authoritative and preserves the
original cascade order.

## Refactoring rules

1. Preserve visual behavior before consolidating rules.
2. Move a rule only when its cascade dependencies are understood.
3. Introduce a shared utility class only when a pattern occurs at least three
   times and has the same semantic meaning.
4. Rust owns state and geometry; CSS owns visual presentation.
5. Avoid selector specificity increases unless they remove a compatibility
   override.
6. Delete obsolete version markers after their rules are consolidated.
7. Run the audit and the complete Rust quality gate after every phase.

## Audit

```bash
python3 scripts/audit_css.py
python3 scripts/audit_css.py --check
```

The report guides Phase 2. Repeated selectors are not treated as errors yet.

## Phase 2 targets

- consolidate circular icon-button geometry;
- consolidate tonal surface borders and shadows;
- consolidate player/footer mode toggle states;
- consolidate repeated hover and checked states;
- replace patch-oriented selectors with semantic component classes;
- move unavoidable compatibility rules to the final ordered module.

## Phase 2A — Expressive transport consolidation

The historical PixelPlayer transport correction layers were replaced by one
semantic component block in `070-player.css`.

This phase intentionally leaves transport geometry and Rust behavior untouched.
It establishes the pattern for later consolidation work:

1. identify the final computed component behavior;
2. preserve component-specific geometry;
3. replace corrective layers with one semantic definition;
4. remove obsolete version markers;
5. record before/after metrics;
6. validate visually before moving to the next component.

## Phase 2B — Shared playback mode controls

Repeat and Shuffle now have one semantic presentation module:
`095-controls.css`.

The Rust factory remains the single source for widget construction. CSS shares
all visual states and retains player/footer classes only where the approved
geometry differs. The toggle system is no longer coupled to compact-volume
overrides or early foundation rules.

## Phase 2C — Shared tonal metadata surfaces

Identical Material color roles for player metadata, footer now-playing
information and favorite actions now live in `096-tonal-surfaces.css`.

Component geometry remains in `000-foundation.css`; only shared palette
properties were moved.

## Phase 2D — Footer cascade cleanup

Superseded footer geometry declarations were removed from
`000-foundation.css` and from the early sections of `010-footer.css`.

The cleanup compares the final exact-selector cascade before and after every
mutation. Colors, borders, surfaces and shadows that still contribute to the
approved result remain in place.

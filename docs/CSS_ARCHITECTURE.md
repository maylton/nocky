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

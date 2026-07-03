# Material Expressive switches, toggles and segmented controls

## Objective

This checkpoint adds a Material Expressive polish layer for binary controls,
toggle buttons, segmented navigation and compact option controls. It keeps the
existing GTK widgets and callbacks intact while giving the theme a consistent
state model for hover, focus, selected, checked and disabled states.

## Files changed

- `assets/themes/material-expressive/106-switches-toggles-segmented-controls.css`
- `src/theme_css.rs`
- `src/app/controller/construction.rs`
- `src/mode_toggle.rs`
- `src/ui/settings/page.rs`
- `src/ui/settings/stream_sources.rs`
- `src/ui/widgets/animated_page_switcher.rs`
- `docs/SWITCHES_TOGGLES_SEGMENTED_CONTROLS.md`
- `ROADMAP.md`

## Covered surfaces

- Settings `GtkSwitch` rows.
- YouTube stream-source enable/disable switches.
- Header toggle controls for sidebar, search and settings.
- Shared repeat/shuffle playback toggles.
- Animated page switchers used by the app header and Settings tabs.
- Settings and playlist dialog dropdown option controls.

## Styling contract

- Switches use larger Material track geometry, rounded handles and clear checked
  contrast.
- Toggle buttons preserve their current behavior while sharing selected,
  hover, active, focus-visible and disabled visual states.
- Segmented controls keep the existing animated indicator architecture and add
  semantic classes for track, indicator and buttons.
- Dropdown controls receive the same option-control treatment without changing
  their model or selected values.

## Patch limits

- This patch does not alter Local or YouTube search behavior.
- It does not introduce mixed Local + YouTube ranking.
- It does not change Home data loading or route behavior.
- It does not add new dependencies.
- It does not replace the custom `AnimatedPageSwitcher`; it only adds semantic
  classes and theme-scoped styling.
- New CSS is scoped to `theme-material-expressive` and does not restyle
  Noctalia or Frosted Glass.

## Recommended validation

- Toggle sidebar, search and Settings header buttons in Material Expressive.
- Toggle repeat and shuffle in both player and footer controls.
- Open Settings and verify all switches, dropdowns and Settings tabs.
- Open YouTube stream sources and verify enabled/disabled switch states,
  including the protected final active source.
- Check keyboard focus rings on switches, header toggles and segmented tabs.
- Run:
  - `cargo fmt --all`
  - `cargo fmt --all -- --check`
  - `cargo check --all-targets`
  - `cargo test --all-targets`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `git diff --check`

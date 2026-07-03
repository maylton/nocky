# Material Expressive dialog and confirmation surfaces

## Objective

This checkpoint adds a progressive Material Expressive polish layer for dialogs,
modal surfaces and confirmation/decision areas. The goal is visual consistency:
dialog containers, titles, descriptions, action rows and destructive actions now
have theme-scoped tonal surfaces and state layers without changing app flows.

## Files changed

- `assets/themes/material-expressive/105-dialog-confirmation-surfaces.css`
- `src/theme_css.rs`
- `src/dialogs.rs`
- `src/youtube/playlist_create.rs`
- `src/ui/settings/stream_sources.rs`
- `docs/DIALOG_CONFIRMATION_SURFACES.md`
- `ROADMAP.md`

## Covered surfaces

- `adw::Dialog` surfaces in the Material Expressive theme.
- Startup source decision dialog.
- YouTube settings dialog surface.
- YouTube playlist creation dialog.
- YouTube stream-source settings dialog.
- Generic helper classes for future confirmation surfaces:
  - `material-dialog-surface`
  - `material-dialog-content`
  - `material-dialog-title`
  - `material-dialog-description`
  - `material-dialog-action-row`
  - `confirmation-dialog`
  - `alert-dialog`
  - `destructive-confirmation-area`
  - `material-dialog-primary-action`
  - `material-dialog-secondary-action`
  - `material-dialog-destructive-action`

## Patch limits

- This patch does not change Local or YouTube search behavior.
- It does not introduce mixed Local + YouTube ranking.
- It does not alter Home layout or Home data loading.
- It does not add new dependencies.
- It does not change playlist creation, stream-source persistence or startup
  source selection semantics.
- New CSS selectors are scoped to `theme-material-expressive` so Noctalia and
  Frosted Glass are not restyled by this checkpoint.

## Recommended validation

- Open the startup source dialog in Material Expressive and verify title,
  description, choice buttons and cancel action states.
- Open YouTube Music settings, then open stream sources and verify modal
  surface, rows, reset action and disabled icon buttons.
- Open the YouTube playlist creation dialog and verify disabled/enabled create
  button states.
- Check hover, active, focus and disabled states for primary, secondary and
  destructive-style dialog actions.
- Run:
  - `cargo fmt --all`
  - `cargo fmt --all -- --check`
  - `cargo check --all-targets`
  - `cargo test --all-targets`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `git diff --check`

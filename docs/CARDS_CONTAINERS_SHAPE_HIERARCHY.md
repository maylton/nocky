# Material Expressive cards, containers and shape hierarchy

## Objective

This checkpoint consolidates Material Expressive card and container shape roles
without changing card behavior or navigation. It adds a final cascade layer for
surface hierarchy, corner radii and active/loading card treatment so repeated
surfaces read as one system instead of unrelated per-page patches.

## Files changed

- `assets/themes/material-expressive/107-cards-containers-shape-hierarchy.css`
- `src/theme_css.rs`
- `docs/CARDS_CONTAINERS_SHAPE_HIERARCHY.md`
- `ROADMAP.md`

## Covered surfaces

- Material card variants: elevated, filled and outlined.
- Home and collection cards, including featured, compact, track-row and compact
  artist cards.
- Settings groups and rows.
- Queue page header, scroll container and queue rows.
- Search section cards and result surfaces.
- Collection page headers and library page containers.
- Playlist editor and YouTube auth card containers.
- Generic future helper classes:
  - `material-container-surface`
  - `material-page-container`
  - `material-card-hero-shape`
  - `material-card-standard-shape`
  - `material-card-compact-shape`
  - `material-card-row-shape`
  - `material-shape-extra-large`
  - `material-shape-large`
  - `material-shape-medium`
  - `material-shape-small`

## Styling contract

- Extra-large page/container surfaces use 32 px corners.
- Large card and grouped surfaces use 28 px corners.
- Standard cards use 24 to 26 px corners depending on density.
- Compact rows and controls use 20 to 22 px corners.
- Active cards keep a primary-tonal outline without changing layout.
- Loading and skeleton cards keep stable shape and neutral tonal surfaces.

## Patch limits

- This patch does not alter Local or YouTube search behavior.
- It does not introduce mixed Local + YouTube ranking.
- It does not change card click behavior, queue operations or navigation.
- It does not add dependencies.
- It does not replace existing card widgets or the carousel layout.
- New CSS is scoped to `theme-material-expressive` and does not restyle
  Noctalia or Frosted Glass.

## Recommended validation

- Open Home and compare featured, compact and track-row cards.
- Open Albums, Artists and Playlists routes and check card/header hierarchy.
- Open Settings and verify grouped surfaces and row containers.
- Open Queue and verify header, scroll container and active queue row.
- Search locally and in YouTube mode and verify section/result surfaces.
- Run:
  - `cargo fmt --all`
  - `cargo fmt --all -- --check`
  - `cargo check --all-targets`
  - `cargo test --all-targets`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `git diff --check`

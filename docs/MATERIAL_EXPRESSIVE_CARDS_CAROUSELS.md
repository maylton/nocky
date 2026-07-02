# Material Expressive Cards And Carousels

This document records the first card/carousel checkpoint for Nocky.

References:

- Cards: <https://m3.material.io/components/cards>
- Carousel: <https://m3.material.io/components/carousel>

## Contract

Material cards use explicit semantic classes:

- `material-card`;
- one variant class: `material-card-elevated`, `material-card-filled`, or
  `material-card-outlined`.

Material carousels use:

- `material-carousel`;
- one variant class: `material-carousel-multi-browse` or
  `material-carousel-hero`.

Card actions use semantic roles without replacing the existing clickable card
surface:

- `material-card-primary-action` for the floating play/resume action;
- `material-card-overflow-trigger` for the overflow menu trigger;
- `material-card-menu-action` for actions inside the card overflow menu.
- state classes `material-card-menu-action-selected`,
  `material-card-menu-action-loading`, and `material-card-menu-action-success`
  for favorite/offline feedback.

## First Checkpoint

The first migration keeps the existing Home geometry and browser-owned scroll
physics intact:

- Home collection card surfaces are elevated Material cards because they sit on
  section containers and need separation.
- Collection-grid and compact artist card surfaces share the same card
  contract.
- Home visual rails receive `material-carousel-multi-browse`.
- Featured visual rails keep the same card geometry as compact rails. The M3
  carousel morphing reference is not enabled yet, so Nocky avoids mixing card
  sizes in the static carousel state.
- Chip rails are not Material carousels because they are filter controls rather
  than visual item collections.
- Home card action controls now expose Material card-action roles while keeping
  the final cascade in `080-home-browser.css`.
- Card action polish adds visible focus, selected favorite state, offline
  loading/success states and accessible labels for icon-only card actions.
- Settings and YouTube stream-source surfaces now reuse the Material card
  contract: hero entries are elevated, grouped sections are filled, and
  scannable rows are outlined while keeping their existing controls.

No transport controls, page switchers, queue rows or full-card click targets are
replaced by the card contract. Clickable wrappers remain buttons, while the
inner surface carries the card semantic.

## Validation

Automated validation for this checkpoint is `cargo fmt`, `git diff --check` and
`cargo test`. Manual validation should cover Material Expressive Home
carousels, collection grids and compact artist rows at narrow and wide widths,
with all cards in the same carousel rendered at uniform size.

The continuous item morphing shown in the animated M3 reference requires a
separate scroll-position-driven checkpoint. Until that exists, this checkpoint
keeps static carousel cards uniform instead of mixing large and compact items.

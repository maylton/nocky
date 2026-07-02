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

## First Checkpoint

The first migration keeps the existing Home geometry and browser-owned scroll
physics intact:

- Home collection card surfaces are elevated Material cards because they sit on
  section containers and need separation.
- Collection-grid and compact artist card surfaces share the same card
  contract.
- Home visual rails receive `material-carousel-multi-browse`.
- Featured visual rails now follow the Material multi-browse composition: the
  first visible item is large, while trailing items use the compact card
  geometry to create the peek-and-browse effect from the M3 carousel reference.
- While scrolling a featured rail, the large item follows the first
  substantially visible card, approximating the animated M3 carousel focus
  change without replacing GTK's native scrolling.
- Featured items now interpolate width, height, artwork size, text width and
  detail visibility by distance from the focal keyline, matching the M3
  principle that carousel items smoothly expand and collapse between large,
  medium and small roles.
- Chip rails are not Material carousels because they are filter controls rather
  than visual item collections.
- Home card action controls now expose Material card-action roles while keeping
  the final cascade in `080-home-browser.css`.

No transport controls, page switchers, queue rows or full-card click targets are
replaced by the card contract. Clickable wrappers remain buttons, while the
inner surface carries the card semantic.

## Validation

Automated validation for this checkpoint is `cargo fmt`, `git diff --check` and
`cargo test`. Manual validation should cover Material Expressive Home
carousels, collection grids and compact artist rows at narrow and wide widths.

The implementation keeps GTK's native scrolling and approximates the M3
keyline model with distance-based interpolation. It does not yet implement true
content parallax or snap-to-keyline behavior.

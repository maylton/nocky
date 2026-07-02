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

## First Checkpoint

The first migration keeps the existing Home geometry and browser-owned scroll
physics intact:

- Home collection card surfaces are elevated Material cards because they sit on
  section containers and need separation.
- Collection-grid and compact artist card surfaces share the same card
  contract.
- Home visual rails receive `material-carousel-multi-browse`.
- Chip rails are not Material carousels because they are filter controls rather
  than visual item collections.

No transport controls, page switchers, queue rows or full-card click targets are
replaced by the card contract. Clickable wrappers remain buttons, while the
inner surface carries the card semantic.

## Validation

Automated validation for this checkpoint is `cargo fmt`, `git diff --check` and
`cargo test`. Manual validation should cover Material Expressive Home
carousels, collection grids and compact artist rows at narrow and wide widths.

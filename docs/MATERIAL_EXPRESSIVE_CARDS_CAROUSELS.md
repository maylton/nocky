# Material Expressive Cards And Carousels

This document records the Material card and carousel implementation used by
Nocky.

References:

- Cards: <https://m3.material.io/components/cards>
- Carousel: <https://m3.material.io/components/carousel>
- Compose carousel implementation: <https://developer.android.com/develop/ui/compose/components/carousel>

## Card contract

Material cards use explicit semantic classes:

- `material-card`;
- one variant class: `material-card-elevated`, `material-card-filled`, or
  `material-card-outlined`.

Card actions use semantic roles without replacing the existing clickable card
surface:

- `material-card-primary-action` for the floating play/resume action;
- `material-card-overflow-trigger` for the overflow menu trigger;
- `material-card-menu-action` for actions inside the card overflow menu;
- state classes `material-card-menu-action-selected`,
  `material-card-menu-action-loading`, and `material-card-menu-action-success`
  for favorite/offline feedback.

## Carousel contract

Material carousels use:

- `material-carousel`;
- one variant class: `material-carousel-multi-browse`,
  `material-carousel-hero`, or `material-carousel-uncontained`.

The controller in `src/ui/widgets/material_card.rs` implements the M3 keyline
behavior for GTK:

- each item retains a stable layout slot;
- only its visible mask width changes while scrolling;
- the outside edge is clipped while the visible edge remains aimed at the focal
  keyline;
- mask width is interpolated continuously instead of switching between fixed
  CSS sizes;
- large, medium and small mask states adjust corner shape and content density;
- kinetic scrolling remains owned by `GtkScrolledWindow`.

This avoids the layout feedback and horizontal jumping that would happen if the
real widget allocation were repeatedly expanded and collapsed.

### Nocky mapping

- Featured Home sections are inferred as **Hero** carousels. One item receives a
  strong focal keyline while neighboring cards collapse into medium and small
  previews.
- Compact collection sections use **Multi-browse**. Cards remain large in the
  central browsing region and progressively collapse near either viewport edge.
- TrackRows use **Uncontained** behavior. Track cards keep one stable width and
  are not masked like image-first collection cards.
- Chip rails are not Material carousels because they are filter controls rather
  than visual item collections.

## Preserved Home hierarchy

Material semantics do not flatten the Home hierarchy introduced in Nocky 0.6.0:

- Featured outer/card/artwork widths remain `220/196/176 px`;
- Compact outer/card/artwork widths remain `168/152/128 px`;
- TrackRows retain their horizontal row geometry.

The keyline mask may temporarily reveal a smaller portion of a card, but the
underlying Featured and Compact geometry remains distinct.

## Typography

The Material Expressive theme uses **Google Sans Flex** first, followed by
Google Sans, Inter, Cantarell and the generic sans-serif fallback. The font rule
is scoped to Material Expressive, so Noctalia and Frosted Glass keep their own
typography.

Install the official Google Fonts family for the current user with:

```bash
bash scripts/install-google-sans-flex.sh --user
```

Use `--system` or `--prefix PATH` when appropriate. The helper installs the
font files and the OFL license from the official family archive and refreshes
the fontconfig cache when available.

## Other migrated surfaces

- Home collection card surfaces are elevated Material cards because they sit on
  section containers and need separation.
- Collection-grid and compact artist card surfaces share the same card
  contract.
- Settings and YouTube stream-source surfaces reuse the Material card contract:
  hero entries are elevated, grouped sections are filled, and scannable rows
  are outlined while keeping their existing controls.
- Noctalia has explicit compatibility styling for shared Material button, icon
  button and chip semantics.

No transport controls, page switchers, queue rows or full-card click targets are
replaced by the card contract. Clickable wrappers remain buttons, while the
inner surface carries the card semantic.

## Validation

Automated validation includes `cargo fmt`, `cargo check`, `cargo test`, Clippy,
CSS manifest assertions and shell/release checks from the repository Quality
Gate.

Manual validation should cover:

- Featured Hero rails at narrow, normal and wide widths;
- Compact Multi-browse rails while dragging, wheel-scrolling and using kinetic
  touch scrolling;
- leading and trailing small previews;
- TrackRows remaining uniform;
- card play/overflow controls staying clipped and aligned correctly;
- theme switching between Material Expressive, Noctalia and Frosted Glass;
- Google Sans Flex selection and fallback behavior.

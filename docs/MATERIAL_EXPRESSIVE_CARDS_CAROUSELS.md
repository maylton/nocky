# Material Expressive Cards And Carousels

This document records the Material card and carousel implementation currently
used by Nocky.

References:

- Cards: <https://m3.material.io/components/cards>
- Motion: <https://m3.material.io/styles/motion/overview>

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

Nocky keeps horizontal collection rails as regular GTK scrollers with stable
card geometry. The experimental Hero/MultiBrowse keyline and mask implementation
was retired after review because it introduced unstable card density, control
clipping and layout complexity that did not improve the desktop Home.

The active carousel behavior is a bounded edge spring implemented in
`src/ui/widgets/material_card.rs`:

- card geometry remains unchanged while browsing normally;
- kinetic scrolling stays owned by `GtkScrolledWindow`;
- reaching the leading or trailing edge triggers a short 520 ms spring;
- the three cards closest to the edge receive decreasing strengths of `1.0`,
  `0.60`, and `0.32`;
- both the outer slot and the visible `.collection-card` surface participate in
  the animation;
- original width requests are restored when the animation finishes or the
  scroller is unmapped;
- the effect is scoped to Material Expressive and does not affect TrackRows;
- play/pause and overflow controls keep stable opacity while scrolling.

The semantic variant classes remain available for presentation metadata and CSS
compatibility:

- `material-carousel-multi-browse`;
- `material-carousel-hero`;
- `material-carousel-uncontained`.

They no longer imply Android-style keyline resizing.

### Nocky mapping

- Featured and Compact Home sections both use stable collection-card geometry
  and the same edge-spring interaction.
- TrackRows keep their horizontal row geometry and do not receive card-width
  spring animation.
- Chip rails are filter controls rather than visual collection carousels.

## Preserved Home hierarchy

The current production Home intentionally keeps Featured and Compact card
metrics uniform while the earlier responsive Hero experiment remains archived
outside the active path:

- outer width: `168 px`;
- card width: `152 px`;
- artwork width: `128 px`;
- TrackRows retain their dedicated horizontal row geometry.

The edge spring is temporary visual feedback and never becomes a persistent
layout allocation.

## Loading-state contract

Collection loading follows three levels:

1. inline Material Loading Indicator inside an action that initiated work;
2. skeleton treatment on an existing active collection card;
3. dedicated non-interactive placeholder rails when remote Home content has not
   produced its first real section yet.

Placeholder rails must:

- preserve the final section and card geometry;
- avoid fake clickable actions;
- keep source-aware loading copy outside the card skeletons;
- avoid layout shifts when real sections replace them;
- remain calm when reduced motion is requested.

The detailed action and loading audit lives in
`docs/CARD_ACTIONS_LOADING_AUDIT.md`.

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

- leading and trailing spring feedback on Featured and Compact rails;
- wheel, touchpad, kinetic and scrollbar-driven edge arrival;
- widths returning exactly to their original values;
- TrackRows remaining uniform;
- card play/pause and overflow controls remaining visible and aligned;
- initial remote Home placeholder rails transitioning to real content;
- theme switching between Material Expressive, Noctalia and Frosted Glass;
- Google Sans Flex selection and fallback behavior.

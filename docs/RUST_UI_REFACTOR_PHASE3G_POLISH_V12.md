# Compact footer vertical-air correction v12

## Problem

The Full fill behavior was also used as the footer card's construction-time
default. Compact mode therefore could briefly or persistently inherit a card
that occupied the complete footer height.

## Construction default

The now-playing card now starts in the Compact-safe state:

- `vexpand = false`;
- `valign = Center`.

## Full mode

`apply_footer_mode()` still applies:

- `vexpand = true`;
- `valign = Fill`;
- zero vertical margins.

The Full card continues to occupy the complete available height.

## Compact mode

`apply_footer_mode()` applies:

- `vexpand = false`;
- `valign = Center`;
- 4 px top margin;
- 4 px bottom margin;
- existing 52 px card request;
- existing 50 px artwork.

Marker: `nocky_footer_compact_restores_vertical_air_v12`.

# Footer Full metadata visual-density polish v7

## Why the previous geometry change looked unchanged

The v4 successfully enlarged the card and artwork, but the metadata hierarchy
remained controlled by the existing CSS:

- the metadata container had zero spacing;
- title and artist rows had no visual separation;
- the Full card retained the same typography as Compact mode.

## Full mode

- artwork: 64 × 64 px;
- metadata card request: 72 px high;
- title/artist row separation: 3 px;
- additional horizontal breathing room: 2 px;
- title typography: 0.97 rem in Material Expressive;
- artist typography: 0.79 rem in Material Expressive.

## Compact mode

Compact geometry and typography remain unchanged:

- artwork: 50 × 50 px;
- metadata card request: 52 px;
- existing dense hierarchy preserved.

## Themes

The row-spacing hierarchy applies to both Material Expressive and Noctalia.
Material Expressive additionally receives the slightly larger Full typography.

Marker: `nocky_footer_full_metadata_visual_density_v7`.

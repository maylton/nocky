# Footer artwork tracks metadata-card height v11

## Behavior

In Full mode, the artwork now follows the metadata card's actual allocated
height rather than using only the fixed fallback size from the footer plan.

The displayed square size is:

`max(card height - 6 px, 72 px)`

This preserves 3 px of vertical breathing room above and below the artwork.

## Image quality

Artwork is decoded at 96 × 96 px and then displayed at the current required
size, avoiding upscaling when the Full card receives a taller allocation.

## Compact mode

Compact mode continues to use the explicit 50 × 50 px size applied by
`apply_footer_mode()`. The adaptive tracker exits immediately while the footer
has the Compact class.

## Performance

`CoverView::set_display_size()` now ignores requests that match the current
size, preventing unnecessary allocation loops inside the tick callback.

Marker: `nocky_footer_artwork_tracks_card_height_v11`.

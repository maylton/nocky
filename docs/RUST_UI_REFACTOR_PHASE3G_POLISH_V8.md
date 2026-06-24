# Footer metadata fill-height correction v8

## Correct interpretation

The metadata card must use the complete vertical allocation available to the
start child of the footer, rather than merely requesting a larger fixed
height while remaining vertically centered.

## Full mode

- vertical expansion enabled;
- `GtkAlign::Fill`;
- zero top and bottom margins;
- 72 px artwork displayed inside the filled card;
- existing Full metadata hierarchy and CSS spacing preserved.

## Compact mode

- vertical expansion disabled;
- `GtkAlign::Center`;
- 50 px artwork;
- existing compact card height and density preserved.

## Why this changes the visible result

`set_size_request()` establishes a minimum size. It does not make a centered
widget consume the parent allocation. The mode-dependent `valign` and
`vexpand` settings now control the actual vertical use of the footer.

Marker: `nocky_footer_metadata_fill_available_height_v8`.

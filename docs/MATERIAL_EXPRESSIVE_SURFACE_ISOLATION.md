# Material Expressive surface isolation

This note records the GTK Inspector diagnosis and the final fix for the
Material Expressive and Frosted Glass dialog/player edge artifacts.

## Problem

Opening the YouTube Music settings dialog in Material Expressive made the
application behind the dialog look almost black. Lowering the libadwaita
`dimming` node opacity did not reveal the normal app content; it only exposed a
uniform Material surface. Later visual checks showed small dark corner halos on
the YouTube dialog, main Home player card and main footer.

The important Inspector findings were:

- `AdwDialogHost` creates an internal `dimming` node from GTK CSS.
- `floating-sheet` and its inner widget were transparent.
- `sheet` was opaque only for the central modal surface.
- `window.theme-material-expressive > toastoverlay` was painting a full-window
  Material surface above the app shell.
- Several rounded top-level surfaces still projected external `box-shadow`
  outside their border radius, which looked like leftover Noctalia corners.

## Fix

The YouTube Music dialog now has an explicit themed content surface:

- the AdwDialog wrapper nodes stay transparent for Material Expressive and
  Frosted Glass;
- `.youtube-dialog-surface` owns the visible modal background, border radius and
  border;
- dialog internals such as toolbar, viewport, clamp and host stay transparent;
- the dialog content size was increased to avoid compressing the lower
  controls.

The main app and player surfaces now avoid hidden full-window or off-radius
painting:

- `toastoverlay` stays transparent in static CSS, runtime blur CSS and dynamic
  palette CSS;
- the main player and footer keep their own rounded borders and gradients but
  do not project an external drop shadow;
- the footer root clips its own painting with `gtk::Overflow::Hidden`.

## Validation

Validated with:

```bash
GTK_DEBUG=interactive cargo run
cargo fmt
cargo test
```

The relevant regression tests are:

- `theme_css::tests::material_toast_overlay_stays_transparent`
- `material_palette::tests::dynamic_css_keeps_toast_overlay_transparent`

Marker: `nocky_material_expressive_surface_isolation_v1`.

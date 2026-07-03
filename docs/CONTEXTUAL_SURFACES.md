# Material Expressive contextual surfaces

## Scope

This checkpoint adds the first Material Expressive treatment for transient
contextual surfaces: menu popovers, contextual action menus and future action
sheets.

## Styling contract

- popovers use high tonal surfaces instead of flat GTK defaults;
- menu contents get rounded 24 px containers with outline and subtle elevation;
- menu rows use 16 px state-layer rounding;
- hover, active and focus-visible states follow Material 3 Expressive tonal
  layering;
- destructive actions receive error-color state layers;
- generic helper classes are available for future custom contextual surfaces:
  - `contextual-surface`;
  - `material-contextual-surface`;
  - `material-contextual-menu-item`;
  - `material-contextual-separator`.

## Non-goals

This does not rewrite menu behavior or change any queue, playlist, search or
collection action semantics. It is a visual-system checkpoint that lets existing
GTK popovers and modelbutton menus inherit coherent Material Expressive surfaces.

## Next checkpoints

Dialogs and confirmation surfaces can now reuse the same tonal/elevation logic
instead of reintroducing ad-hoc popover styling.

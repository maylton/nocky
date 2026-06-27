# UI modules

This directory contains Nocky's native UI module hierarchy.

The main groups are:

- `footer`: responsive footer layout, transport controls, progress, utilities,
  and now-playing surfaces.
- `player`: the home player view.
- `settings`: settings-page widgets.
- `widgets`: reusable application widgets such as cover art, animated page
  switching, compact volume motion, transport effects, and wave progress.

Application coordination stays in `app::controller`; these modules should keep
visual construction and local widget behavior focused.

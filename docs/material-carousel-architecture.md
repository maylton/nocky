# Isolated Material Carousel Architecture

This component will replace the current Home carousel motion with an isolated GTK 4 widget. It is intentionally developed outside the Home page first so geometry, scrolling, clipping, and strategy behavior can be verified before integration.

## Component Contract

`MaterialCarousel` will be a custom `gtk::Widget`. It owns carousel children and exposes only explicit state inputs from the surrounding scroll container.

`MaterialCarouselLayout` will be a custom `gtk::LayoutManager`. It will be responsible for measuring the carousel and allocating each visual item from pure geometry derived from the current scroll state.

`MaterialCarouselItem` will be a wrapper with exactly one child. The full card content is allocated inside this wrapper; text, subtitles, and details are not hidden by opacity. The wrapper is responsible for clipping during `snapshot()`, so edge treatments mask the item surface without fading card copy.

The outer `gtk::ScrolledWindow` continues to provide the `GtkAdjustment`. The carousel does not replace GTK scrolling; it consumes the adjustment value as input state.

`MaterialCarousel` receives these values explicitly:

- `adjustment`
- viewport width
- variant
- base item extent
- spacing

The layout has a stable logical extent. That extent is independent of which card is visually largest at a given moment, so the scroll range does not jump while the user scrolls.

During `allocate()`, the layout converts the scroll offset into visual positions and extents. Geometry is a continuous function of `scroll_offset`; there is no dynamic choice of a nearest card as an anchor.

The `GtkAdjustment` callback only stores the latest scroll state and requests a new layout. It does not call `set_width_request`, set margins, move children, compute bounds, or allocate widgets directly.

There will be no `GtkFixed` in the final implementation. There will also be no `compute_bounds` calls during carousel animation.

## Variants

Home presentation mapping:

- Featured maps to `Hero`.
- Compact maps to `MultiBrowse`.
- TrackRows stays `Uncontained`.

`Hero`, `MultiBrowse`, and `Uncontained` are separate layout strategies. `Uncontained` preserves the current TrackRows behavior and remains outside the contained Material carousel motion.

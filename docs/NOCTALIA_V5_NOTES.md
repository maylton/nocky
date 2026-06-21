# Nocky, Noctalia V5 and MPRIS notes

Nocky is an independent GTK4/libadwaita application. It does not reuse Noctalia's internal renderer or widgets. Visual integration is achieved through compatible Material color roles, while media integration uses the freedesktop MPRIS protocol.

## Media-card hierarchy

The interface follows the same broad hierarchy used by Noctalia's media surfaces:

1. surface-container card
2. “Now Playing” heading
3. centered square artwork
4. primary-colored title
5. on-surface-variant artist
6. secondary album text
7. progress slider
8. repeat, previous, primary play/pause, next and shuffle controls

Nocky keeps fixed artwork and metadata regions so long titles do not resize the card or footer.

## Stable integration boundary

Nocky communicates with the shell through:

- MPRIS over the user session D-Bus
- generated Noctalia color templates
- a standard desktop entry and hicolor icon

No undocumented Noctalia internal API is required.

## MPRIS contract

- Bus-name suffix: `Nocky`
- Desktop entry: `io.github.maylton.Nocky`
- Track IDs: stable object paths derived from local audio paths
- Track and artwork locations: local `file://` URIs
- Position and length: microseconds
- Commands: play, pause, play/pause, stop, next, previous, seek, volume, shuffle, loop, raise and quit

## Playback boundary

Only `src/playback.rs` communicates with GStreamer. The GTK controller consumes high-level events from the GStreamer bus. Track replacement first moves the pipeline to `NULL`, then assigns the next URI and returns to `PAUSED` or `PLAYING`.


## Audio visualizer alignment (Nocky 0.6)

The Now Playing visualizer follows the current Noctalia V5 media visualizer concepts without copying its renderer:

- 32 logical bands
- horizontal layout
- centered and mirrored frequency arrangement
- theme-derived two-color gradient
- approximately 60 ms exponential smoothing
- live values from GStreamer's `spectrum` analyzer

Nocky draws the result with GTK/Cairo because it is a separate GTK application, while Noctalia uses its own Wayland/OpenGL scene graph.

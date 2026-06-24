# Footer metadata polish v4

## Compact mode

- artwork remains 50 × 50 px;
- metadata spacing remains compact;
- utility card remains vertically centered;
- title and artist retain stable natural-width limits;
- the metadata card remains independent of title length.

## Full mode

- metadata card height: 68 px;
- artwork size: 62 × 62 px;
- 2 px breathing room after title and artist;
- Wide, Medium and Narrow Full layouts use the 68 px card;
- the total footer height remains unchanged.

## Image quality

The footer cover texture is loaded at 62 px and displayed at 50 px in Compact
mode, avoiding upscaling when switching to Full mode.

## Preserved

- responsive width breakpoints;
- transport and progress geometry;
- source and favorite behavior;
- compact-volume reveal and spring;
- callbacks, MPRIS and playback state;
- Material and Noctalia themes.

Marker: `nocky_footer_metadata_full_mode_breathing_room_v4`.

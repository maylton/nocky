# Frosted Glass theme

Frosted Glass is the third Nocky visual identity, introduced in **Nocky 0.5.0**.

It deliberately keeps the application behavior and expressive motion system while changing the composition from connected Material surfaces to floating translucent islands.

## Design identity

| Theme | Surface language | Artwork colors |
| --- | --- | --- |
| Noctalia | Shell-oriented surfaces that follow the desktop palette | Controlled by the Noctalia integration |
| Material Expressive | Solid tonal planes and expressive geometry | Used directly across controls and surfaces |
| Frosted Glass | Floating translucent islands and matte controls | Used as restrained ambient lighting |

Frosted Glass reuses the responsive layout, wavy progress and expressive transport behavior. It does **not** alter playback, Local Library and YouTube Music separation, queues, history, search or playlists.

## Dynamic artwork palette

Frosted Glass uses the same album-aware palette generation as Material Expressive. When artwork changes, Nocky transitions the Material color roles gradually.

The theme intentionally applies those roles more subtly:

- `primary` provides the main ambient tint;
- `tertiary` adds a secondary low-intensity aura;
- `primary_container` marks selected and active controls;
- neutral surface roles keep cards readable and less saturated.

When artwork is unavailable, the normal Material fallback palette is used.

## Blur and opacity

The visual theme and blur source are independent settings.

Available blur modes:

- **Custom blur** — use the compositor blur configured by the user;
- **Follow Noctalia blur** — use the Noctalia-managed blur state;
- **No blur** — use opaque fallback surfaces.

The glass-opacity slider remains independent and is clamped to the supported range. Nocky reads Niri or Hyprland state but does not write compositor configuration. The compositor supplies real background blur; Nocky controls GTK alpha and surface styling.

## Surface hierarchy

The theme distinguishes itself through:

- detached header and footer islands;
- rounded sidebar and content panels;
- a denser main player card;
- matte media, Settings and onboarding cards;
- neutral, darker queue popovers;
- low-saturation hover and active states;
- preserved but reduced ambient lighting when the window loses focus.

## Controls

Buttons avoid glossy top reflections. Active controls rely on tonal fills and borders rather than solid black or bright highlights.

Special handling includes:

- Settings active state;
- YouTube resynchronization action;
- Lyrics and Volume footer controls;
- transparent Home / Lyrics / Queue switcher segments;
- keyboard-only accessibility rings.

## Queue behavior

The queue popover uses a darker, denser neutral surface with only a restrained color cue for the current item.

**Clear upcoming** preserves the current track.  
**Clear all** stops playback, clears Local or YouTube playback context, persists an empty queue and removes the restorable playback session.

## Focus and backdrop behavior

Losing window focus no longer removes all gradients, borders and depth. Frosted Glass keeps its identity while reducing:

- ambient color intensity;
- shadow elevation;
- border contrast;
- active-control emphasis.

## Accessibility

The theme preserves readable foreground roles, keyboard focus cues and opaque fallbacks when compositor blur is disabled.

## Known issue

A tiny lower-border cusp can appear on the main player card with some GTK/compositor combinations. Two experimental contour overrides did not solve the renderer artifact and were removed before the 0.5.0 release. The issue is intentionally deferred for a focused fix that does not compromise the card geometry.

## Relevant files

- `assets/themes/frosted-glass.css`
- `src/config.rs`
- `src/theme_css.rs`
- `src/visual_theme.rs`
- `src/ui/settings/page.rs`
- `src/onboarding.rs`
- `src/app/controller/appearance.rs`

# Compositor blur compatibility

Nocky supports persistent window blur on:

- niri, including CachyOS layouts with separate included KDL rule files;
- Hyprland 0.53-0.54 using hyprlang configuration;
- Hyprland 0.55+ using Lua configuration.

The active compositor is detected from the Wayland session environment.
Noctalia mode combines `~/.config/noctalia/config.toml` tint information with
the active compositor's effective blur settings.

For niri setup, see `docs/NIRI_BLUR.md`.
For Hyprland setup, see `docs/HYPRLAND_BLUR.md`.

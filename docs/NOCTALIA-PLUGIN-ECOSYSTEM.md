# Noctalia plugin ecosystem

Nocky is part of an optional three-project integration. Each project remains
usable and configurable independently.

## Related repositories

- **[Nocky](https://github.com/maylton/nocky)** — Linux music player and
  publisher of MPRIS metadata plus the native Album Aura bridge.
- **[Album Aura](https://github.com/maylton/album-aura-plugin)** — generates and
  activates Noctalia palettes from album artwork.
- **[OpenRGB Noctalia](https://github.com/maylton/openrgb-noctalia)** — applies
  the active Noctalia primary color to physical RGB devices.

## Integration flow

```text
Nocky or another MPRIS player
             ↓
          Album Aura
             ↓
       Noctalia palette
             ↓
 OpenRGB Noctalia (optional)
```

## Nocky and Album Aura

Nocky publishes its versioned runtime bridge atomically to:

```text
$XDG_RUNTIME_DIR/nocky/album-aura.json
```

Album Aura accepts the bridge only while it is active and its player/track
identity still matches the active Nocky MPRIS track.

The resolution order is:

1. inline `palette`;
2. `palette_path`;
3. `artwork_path`;
4. `artwork_url` or `art_url`;
5. generic MPRIS `mpris:artUrl`.

When Nocky stops controlling the current album state, it should remove the
bridge or publish `"active": false`.

## Album Aura and OpenRGB Noctalia

Album Aura activates a regular Noctalia custom palette named `AlbumAura`.

OpenRGB Noctalia follows the active Noctalia primary color instead of depending
directly on Nocky. Therefore:

- while Album Aura is active, RGB devices follow the album palette;
- after Album Aura restores the previous palette, RGB devices follow the normal
  Noctalia theme again;
- pausing OpenRGB synchronization does not disable Album Aura;
- pausing Album Aura does not disable normal OpenRGB/Noctalia synchronization.

This separation avoids feedback loops and duplicated RGB-controller logic.

## Installation

Album Aura:

```bash
noctalia msg plugins source add album-aura git   https://github.com/maylton/album-aura-plugin
noctalia msg plugins update album-aura
noctalia msg plugins enable maylton/album-aura
```

OpenRGB Noctalia:

```bash
noctalia msg plugins source add openrgb-noctalia git   https://github.com/maylton/openrgb-noctalia
noctalia msg plugins update openrgb-noctalia
noctalia msg plugins enable maylton/openrgb-noctalia
```

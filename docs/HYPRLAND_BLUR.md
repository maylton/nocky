# Nocky window blur on Hyprland

Nocky controls the GTK surface transparency. Hyprland provides the real
background blur. The app detects Hyprland through
`HYPRLAND_INSTANCE_SIGNATURE` and reads the effective blur configuration from
the running compositor with `hyprctl getoption`.

This means split configurations work automatically:

- Hyprland 0.54 and earlier: `source = ...` files;
- Hyprland 0.55 and newer: Lua modules loaded with `require(...)`;
- distribution layouts that keep decoration and window rules in separate files.

Nocky listens for Hyprland's `configreloaded` IPC event and reapplies the
Noctalia-synchronized glass values after the compositor reloads.

## Global blur

Hyprland window blur is global. It must be enabled for translucent Nocky
surfaces to show real blur.

### Hyprland 0.53-0.54

```ini
decoration {
    blur {
        enabled = true
    }
}

source = ~/.config/hypr/rules/nocky-blur.conf
```

Copy `contrib/hyprland/nocky-blur.conf` to the sourced rules location.

### Hyprland 0.55+

```lua
hl.config({
  decoration = {
    blur = { enabled = true }
  }
})

require("rules/nocky_blur")
```

Copy `contrib/hyprland/nocky_blur.lua` to the required module location.

## Nocky modes

- **Blur**: uses the custom transparency saved by Nocky.
- **Noctalia blur**: combines Noctalia tint values with Hyprland's live
  `enabled`, `size`, `passes`, `noise`, `contrast`, `brightness`, and
  `vibrancy` values.
- **Off**: restores the opaque GTK surface, hiding compositor blur.

The supplied rules deliberately do not set Hyprland window opacity. Nocky
already controls its own alpha channel; an additional compositor opacity rule
would multiply the values and make the UI unnecessarily transparent.

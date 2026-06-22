# Nocky window blur on niri

Nocky's blur modes control the transparency of the GTK window. The compositor
provides the real background blur.

## Requirement

- niri 26.04 or newer

## CachyOS and separate rules files

CachyOS commonly stores niri window rules and the global `blur {}` section in
separate KDL files instead of placing everything directly in `config.kdl`.

Nocky does **not** assume that blur lives in one fixed file. In Noctalia blur
mode it starts from the active niri configuration and follows top-level
`include` directives, including nested includes:

1. `$NIRI_CONFIG`, when set;
2. `~/.config/niri/config.kdl`;
3. `/etc/niri/config.kdl` as a fallback.

Relative paths, absolute paths and `~/...` includes are supported. This means
an active `rules.kdl`, `rules/nocky.kdl`, or another CachyOS rules file is read
when it is included by the real niri configuration. Unused backup or disabled
files are not scanned.

The resolved files and their parent directories are watched, so changing the
CachyOS rules file updates the Nocky appearance without restarting the app.

## Install the Nocky rule

Use the organization already present in your CachyOS setup.

For a rules directory, one possible layout is:

```text
~/.config/niri/
├── config.kdl
└── rules/
    └── nocky-blur.kdl
```

Then ensure the active include chain contains:

```kdl
include "rules/nocky-blur.kdl"
```

If CachyOS already has a central `rules.kdl`, you can paste the
`window-rule` from `contrib/niri/nocky-blur.kdl` into that file instead.

Validate and reload:

```bash
niri validate
niri msg action load-config-file
```

## What is synchronized

Nocky reads the active niri global blur values:

- `off`;
- `passes`;
- `offset`;
- `noise`;
- `saturation`.

It also reads Noctalia's `[backdrop]` values from
`~/.config/noctalia/config.toml` when available. The active niri configuration
has priority because it represents the compositor settings actually used by
CachyOS. Noctalia's backdrop settings are used as complementary tint data and
as a fallback when no niri configuration can be read.

## Modes

- **Blur**: uses Nocky's saved glass transparency.
- **Noctalia blur**: follows the active niri include graph and Noctalia
  backdrop settings.
- **Off**: restores the opaque Nocky surface, hiding compositor blur.

Nocky only reads these files. It does not rewrite the user's niri or CachyOS
configuration.

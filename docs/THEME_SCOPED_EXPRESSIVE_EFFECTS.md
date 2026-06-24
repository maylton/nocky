# Theme-scoped Expressive Effects

## Changes

- removed the colored halo from the Home carousel edge spring;
- preserved the neutral depth shadow and spring geometry;
- restricted compact-volume overshoot/rebound to Material 3 Expressive;
- preserved the native `GtkRevealer` slide in Noctalia;
- made the Material spring respect the system animation preference;
- hid Expressive settings while Noctalia is selected;
- preserved stored Expressive preferences for later M3 use.

## CSS byte contract

- before: 108383
- after: 108381
- delta: -2

## Existing gates verified

- Expressive transport;
- Home carousel edge spring;
- Material wave progress.

## Validation

- source-state preflight;
- architecture audit;
- Rust format/check/test/clippy gate;
- visual comparison in both themes.

## Settings placement

The two Material Expressive effect controls are displayed in the Appearance
section, directly after the visual theme and Noctalia integration controls.
Their theme-dependent visibility and stored values are unchanged.

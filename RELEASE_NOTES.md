# Nocky 0.2.6 Beta — A Better First Experience

Nocky 0.2.6 focuses on the first experience, appearance discovery and clearer synchronized lyrics behavior.

## First-run onboarding

New installations now open a five-step setup wizard before entering the main interface:

1. Welcome
2. Music source
3. Appearance
4. Player and footer
5. Setup summary

The wizard allows users to choose:

- local files or YouTube Music as the initial Home source;
- custom blur, Noctalia blur or an opaque window;
- Noctalia palette synchronization when Noctalia Shell is running;
- the Material Design 3-inspired wavy progress bar;
- Automatic, Full, Compact or Hidden footer behavior.

Selecting the local library opens the folder chooser after setup when no directory has been configured.

## YouTube Music explanation

The onboarding clearly explains that YouTube Music support is experimental, uses unofficial interfaces, may require future compatibility updates and does not require connecting an account for public search.

## Noctalia-aware setup

Noctalia-specific palette and blur options are shown only when Noctalia Shell is detected. Other desktops continue receiving custom blur and opaque-window options without misleading settings.

## Lyrics

The focused inline lyric is now measured with Pango:

- short lines remain on one line;
- long lines wrap only when the available width is exceeded;
- wrapping is limited to two lines;
- embedded whitespace and unexpected line breaks are normalized.

## Branding

The official Nocky icon now appears:

- in the onboarding welcome page;
- above the application name in the About dialog.

## Existing users

Existing configuration files are migrated as already onboarded. Updating from Nocky 0.2.5 will not unexpectedly interrupt users with the setup wizard.

Developers can safely test the flow with:

```bash
NOCKY_FORCE_ONBOARDING=1 cargo run
```

## Suggested tag

```text
v0.2.6-beta
```

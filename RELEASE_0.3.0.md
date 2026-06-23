# Nocky 0.3.0 — Material Expressive

Nocky 0.3.0 is the largest visual and structural update of the project so far. It brings the desktop player closer to Material 3 Expressive while preserving native GTK4/libadwaita behavior and Noctalia integration.

## Highlights

- Complete Material 3 Expressive visual mode.
- Dynamic color palette generated from the current artwork.
- Refined Noctalia visual mode and automatic theme integration.
- Animated Home/Lyrics selector with a light bounce effect.
- Light bounce motion for the library sidebar and collapsible Home player.
- Collapsible main player with persistent state.
- Redesigned full and compact footer player.
- Wavy progress indicators for Material mode.
- History-first Home experience for local music and YouTube Music.
- Improved album, artist, playlist, liked-song and search surfaces.
- Navigable Settings page instead of a modal dialog.
- Dedicated themed About and Keyboard Shortcuts windows.
- Automatic and manual synchronized-lyrics workflows.
- YouTube Music synchronization, search and playback improvements.
- Portuguese, English and Spanish interface coverage.
- Zero-warning build, test and Clippy quality gate.

## Release validation

Run:

```bash
./scripts/check-release-0.3.0.sh
```

## Suggested release flow

```bash
git add -A
git commit -m "release: prepare Nocky 0.3.0"
git push -u origin review/material-3-expressive-v0.3.0

git tag -a v0.3.0 -m "Nocky 0.3.0"
git push origin v0.3.0

gh release create v0.3.0 \
  --title "Nocky 0.3.0 — Material Expressive" \
  --notes-file RELEASE_0.3.0.md
```

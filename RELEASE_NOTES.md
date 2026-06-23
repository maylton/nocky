# Nocky 0.2.5 Beta — A More Personal and Expressive Player

Nocky 0.2.5 is the largest interface and usability update in the 0.2 series. It introduces a personalized Home, a redesigned adaptive footer, synchronized online lyrics, faster collection browsing and a Material 3-inspired animated progress experience.

## Highlights

- Personalized Home sections built from listening history
- Clear separation between the local library and YouTube Music modes
- Categorized online search for songs, albums, artists and playlists
- Synchronized lyrics for local and streamed tracks
- Material 3-inspired wavy progress bar with smooth, continuous animation
- Footer 2.0 with large artwork, improved metadata and responsive behavior
- Four footer modes: Automatic, Full, Compact and Hidden
- Automatic mode avoids duplicated controls throughout every Home route
- Clickable local and YouTube playback queue
- Portuguese, English and Spanish localization
- Incremental album and artist loading for large libraries
- More responsive search and cached online collection browsing
- Automatic recovery when a temporary YouTube stream URL expires
- Numerous visual fixes for cards, metadata, favorites, controls and blur

## Footer modes

### Automatic

The footer stays compact while the Home player is visible, including album, discography, artist and playlist routes. Outside Home, such as the full Lyrics page, the complete footer returns automatically.

### Full

Displays the complete playback interface, including transport controls, progress, lyrics and volume.

### Compact

Keeps the track and queue card, lyrics button, mute and volume controls. Playback controls and the progress bar are hidden to avoid duplicating the Home player.

### Hidden

Removes the footer and restores the available vertical space.

## Local and YouTube Music modes

Local-library mode now stays focused on offline content and does not display online mixes, suggestions or synchronized playlists. YouTube Music mode keeps catalogue search, account synchronization, cached collections, playlists and streamed playback.

## Lyrics

Nocky can display synchronized lyrics for both local and YouTube Music tracks. Inline focused lines wrap only when they exceed the available card width.

## Performance

Albums and artists are rendered incrementally, online searches are categorized and processed in batches, and previously loaded YouTube Music data continues to use the local cache before background refreshes.

## Known limitation

A minor Home-player layout shift can still appear while inline lyrics move between the loading state and synchronized content. It does not affect playback or lyrics synchronization.

## Install

```bash
./install.sh --install-deps
```

For local-library-only use:

```bash
./install.sh --install-deps --without-youtube
```

For development:

```bash
./scripts/setup-youtube-runtime.sh
cargo run
```

## Suggested tag

```text
v0.2.5-beta
```

# Nocky 0.3.1 Beta — Reliability, Queue 2.0 and UI Architecture

Nocky 0.3.1 consolidates the large post-0.3.0 development cycle with a stronger playback model, more reliable navigation and lyrics, a modular UI architecture and a fully audited Material Expressive theme.

## Queue and playback

- Introduces Queue 2.0 with source-independent entries, persistent ordering and improved playback navigation.
- Improves queue interactions, context handling and restore behavior across local and YouTube Music playback.
- Stabilizes optional playback-session resume and YouTube stream recovery.

## Lyrics and listening continuity

- Improves synchronized-lyrics scrolling, recentering and clickable line seeking.
- Makes automatic following more stable while preserving manual navigation.
- Strengthens playback history and resume checkpoints.

## Home, artists and library

- Improves personalized Home ordering based on listening activity.
- Refines artist grouping, collaborative credits, directory profiles and artist-page refresh behavior.
- Keeps YouTube-specific Home sections hidden when the local library is selected.

## Material Expressive and interface architecture

- Splits the Material Expressive stylesheet into audited modules with size and selector validation.
- Extracts player and footer surfaces into focused Rust modules.
- Refines transport motion, compact volume, footer layouts and tonal surfaces.
- Fixes invalid GTK scrollbar slider geometry that produced repeated `min width -8` warnings.

## Localization and quality

- Reviews Portuguese, English and Spanish coverage across 87 interface messages.
- Audits Settings, Home, onboarding, lyrics and themed pop-up surfaces.
- Passes the complete Rust, translation, localization and shell/Python quality gates.

## Release metadata

- Version: `0.3.1`
- Date: `2026-06-25`
- License: GPL-3.0-or-later

## Suggested tag

```text
v0.3.1-beta
```

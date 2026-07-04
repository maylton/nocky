# Home V3 MetroList stack

This branch restarts the YouTube Music Home work from a clean main baseline.

## Direction

Home V3 is a new YouTube Music Home surface inspired by MetroList. It is separate from the previous Home implementation and should be built in small reviewable steps.

The goal is to replicate the MetroList Home behavior, not merely the visual style. The Home should be driven by YouTube Music feed data: chips, feed sections, section endpoints, item endpoints, continuation and refresh state.

## MetroList behavior contract

- The top of the Home exposes YouTube Music chips/filters.
- The main body is a vertical feed of sections.
- Each section has a title/header and usually renders as a horizontal carousel.
- Items preserve their YouTube endpoint behavior: songs play, albums/artists/ playlists open their destination, and unsupported items stay non-destructive.
- A selected chip changes the feed instead of applying local categories over stale data.
- Continuation is requested as the user approaches the end of the vertical feed.
- Loading states should preserve the feed structure with shimmer/empty states instead of falling back to the old Home.

## Plan

1. Add the clean Home V3 helper contract and parser coverage.
2. Add a small Rust bridge contract for Home V3.
3. Add an isolated Home V3 renderer.
4. Wire the YouTube source Home to the isolated renderer.
5. Add chips, continuation, loading and empty states.
6. Polish the MetroList-inspired visual hierarchy after the data and render path is stable.

## Validation checkpoints

Manual validation is needed when the first Home V3 data path reaches the UI, when chips and continuation are wired, and when the visual layout is ready to judge spacing, density and artwork behavior.

## Current integration bridge

The GTK renderer currently mounted in `src/browser.rs` is intentionally named
`youtube_home_v3_legacy_feed_shell`. It is the new Home V3 shell, but it still
receives the legacy `YouTubeHomePage` payload while the native Home V3
helper/parser is being introduced.

This bridge must remain visible in code review so we do not confuse:
- Home V3 renderer/shell readiness;
- native Home V3 source/parser readiness.

The next functional step is to replace this legacy bridge with a native Home V3
feed source that produces the `HomeV3Page` contract directly.\n\n## HomeV3Page renderer boundary

The mounted GTK shell now receives `HomeV3Page` instead of `YouTubeHomePage`
directly. While the feed still originates from the legacy YouTube Home payload,
that payload is first converted by `legacy_youtube_home_page_source` and then
adapted through `adapt_source_page`.

This keeps the next cut clear: replace only the legacy source bridge with the
native Home V3 helper/parser while preserving the renderer contract.\n
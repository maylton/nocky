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
native Home V3 helper/parser while preserving the renderer contract.\n\n\n## Home V3 source resolver

`home_v3_source::resolve_home_v3_source` is the boundary for choosing the feed
source that will be adapted into `HomeV3Page`.

Current runtime state:
- native Home V3 source: absent;
- legacy YouTube Home bridge: active fallback.

Important contract:
- when a native source exists, it wins even if it is empty;
- the old Home must not silently reappear as a fallback after the native source
  is wired.

This prepares the next phase: connect the native helper/parser to this resolver
and remove the legacy bridge from the normal runtime path.\n\n\n## Native Home V3 payload parser

`home_v3_native::parse_native_home_v3_payload` parses the JSON contract emitted
by `helpers/nocky_youtube_home_v3.py` into `HomeV3SourcePage`.

Current state:
- parser exists and is covered by synthetic payload tests;
- runtime still uses the legacy bridge through the source resolver;
- helper output can now evolve independently toward real V3 content.

The next stack step is to make the helper produce populated `chips`, `sections`,
items and continuation using YouTube's native Home response shape, then pass that
payload through this parser before the source resolver.\n

## Native helper extraction

`helpers/nocky_youtube_home_v3.py` now extracts the native Home V3 source
contract from YouTube Music browse responses:

- chip titles and params from chip renderers;
- carousel/list sections from Music shelf renderers;
- item title, subtitle, thumbnail, video id, browse id and params;
- continuation tokens.

The helper remains non-fallback: empty or unknown responses produce an empty
Home V3 payload instead of reusing Home V2 data.\n\n## Native helper CLI contract

`helpers/nocky_youtube_home_v3.py` can now run as a helper command. It reads a
raw YouTube Music Home browse response from stdin and emits the app helper
contract with `ok`, `result` and `error` fields.

Rust can parse this wrapper through
`home_v3_native::parse_native_home_v3_helper_response`. This prepares runtime
wiring without changing the mounted Home UI yet.\n\n\n## Backend Home V3 helper boundary

`YouTubeBridge::native_home_v3_source_from_raw_response` can execute the native
Home V3 helper with a raw YouTube Music Home browse response through stdin and
parse the helper stdout into `HomeV3SourcePage`.

This still does not change the mounted Home UI. The next runtime cut is to make
the existing Home request path preserve the raw browse response long enough to
feed this method, then pass the resulting native source into the source resolver.\n\n\n## Embedded native Home V3 source

The `home_v2` helper command can now embed a `native_v3_source` candidate in the
returned `YouTubeHomePage`. The Rust page model accepts this field as
`Option<HomeV3SourcePage>`.

Current runtime state:
- Home requests ask the Python helper to compute the native V3 source candidate;
- the renderer still resolves with `None` for native source, so the UI remains on
  the safe legacy bridge;
- the next cut is to pass `youtube_home_page.native_v3_source.clone()` into
  `resolve_home_v3_source`, which will require visual validation because native
  data will start winning over the legacy bridge.\n\n\n## Native Home V3 source activation

The browser now passes `youtube_home_page.native_v3_source` into
`resolve_home_v3_source`.

Runtime effect:
- if the helper embeds a native Home V3 source, it wins;
- if the helper does not provide a native source, the legacy bridge remains the
  fallback;
- if the native source exists but is empty, the old Home is intentionally not
  resurrected silently.

This is the first visual-validation cut of the stack.\n\n\n## Home source freshness trace

The Home request path now supports a forced live mode while Home V3 source
freshness is being validated.

Temporary validation behavior:
- Home requests include `force_live: true`;
- stale cached Home data is not used as fallback for the main Home request;
- requests send no-cache headers through the shared requests session;
- setting `NOCKY_HOME_SOURCE_TRACE=1` prints a source hash and section titles for
  each raw Home response.

This exists to prove whether the repeated Home is coming from Nocky's cache/fallback
or from YouTube's Web Home response itself.\n\n\n## Home root chip

The Home V3 feed now injects a stable root chip labelled `Início` with empty
params. YouTube Music filter chips do not always include a way back to the
unfiltered Home feed, so Nocky keeps this navigation affordance explicit.

Clicking `Início` requests `LoadYouTubeHome` with empty params and returns to
the root Home request.\n\n\n## Home V3 visual cards

The Home V3 renderer now paints YouTube Music-style visual cards with cached
artwork, horizontal section scrolling, title/subtitle metadata and a play overlay
for playable items.

This keeps the confirmed YouTube-provided Home data intact and changes only the
presentation layer.\n\n\n## Home V3 carousel hierarchy

Home V3 now restores the older Home hierarchy from the Material Expressive
carousel work:

- first/listen-again sections use Featured geometry;
- collection sections use Compact geometry;
- song-only rails can use TrackRows geometry;
- scroll containers and cards receive the same semantic classes used by the
  Material carousel/card foundation.\n\n\n## Home V3 card density

Home V3 cards now use stable scroller heights and density-aware title/subtitle
limits so long YouTube Music metadata does not stretch or destabilize carousel
rows. Featured, Compact and TrackRows keep separate vertical budgets.\n\n\n## Home V3 card actions and TrackRows

Home V3 restores the card action layer from the previous Home work:

- media cards expose a primary play affordance;
- cards expose an overflow menu with primary action and copy-title action;
- song-like TrackRows use a compact horizontal layout with small artwork,
  single-line metadata and aligned contextual controls.\n
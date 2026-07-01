# Home V2 direct InnerTube validation

PR #85 builds Home V2 card rows directly from raw `WEB_REMIX` InnerTube renderers
instead of using `ytmusicapi.parse_mixed_content` as the primary source. The
artwork follow-up also ensures that every Home section rendered by the GTK Home
is eligible for browser-cover caching, even when its layout is not `carousel`.

## Covered renderers

- `musicTwoRowItemRenderer`
- `musicResponsiveListItemRenderer`
- `musicMultiRowListItemRenderer`
- `reelItemRenderer`
- `shortsLockupViewModel`

The parser preserves item and section order and extracts playback identity, linked
artist/album metadata, duration and artwork from standard thumbnails,
`croppedSquareThumbnailRenderer`, animated-thumbnail static backups and Shorts
thumbnail sources. Root, chip-filtered and continuation responses use the same
parser. The Home cache contract is V4.

## First-paint behavior

Home and playlist loading should not wait for the complete cover-cache pass:

- Home V2 should render recommendations as soon as the structured page is
  available, reusing any cover files already present on disk.
- Fresh cover downloads should update the visible Home silently afterward.
- Opening a YouTube playlist should show tracks after the first visible block is
  prepared, while the rest of the track artwork continues caching in the
  background.

## Real-account validation

Verify artwork in:

- Álbuns para você
- Escolha a dedo
- Favoritos antigos
- Vídeos de música recomendados
- Em alta nos Shorts
- Lançamentos
- Mixes longos
- Apresentações ao vivo
- Covers e remixes
- Suas descobertas diárias

Afterward, switch chips and load at least two continuation pages. Play/pause and
navigation must retain the render-reuse behavior merged in PR #84. The helper logs
an explicit per-section missing-artwork summary when a raw item has neither an
image nor a valid video fallback.

For the artwork/performance follow-up, also clear the cover cache and verify:

- `Em alta nos Shorts`, `Apresentações ao vivo` and `Mixes longos` no longer stay
  on generic placeholders when their items expose usable thumbnails or video IDs;
- the first Home render appears before all covers have downloaded;
- opening a large playlist is usable before every track cover is cached;
- a second visit reuses cached covers immediately.

## Sanitized renderer diagnostics

When a production shelf still differs from the test fixtures, start Nocky with:

```bash
NOCKY_HOME_DEBUG_DUMP=/tmp/nocky-home-renderers.json cargo run
```

The helper writes a JSON file containing the raw renderer structure, a parsed-item
summary, renderer counts and every thumbnail-like path. Authentication headers,
cookies, visitor data, continuation tokens and tracking fields are not included;
URL query strings and fragments are removed. The resulting file can be attached to
the issue to add exact support for renderer experiments used by a real account.

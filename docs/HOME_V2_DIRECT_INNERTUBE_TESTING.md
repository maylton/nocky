# Home V2 direct InnerTube validation

PR #85 now builds Home V2 card rows directly from raw `WEB_REMIX` InnerTube
renderers instead of using `ytmusicapi.parse_mixed_content` as the primary source.

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

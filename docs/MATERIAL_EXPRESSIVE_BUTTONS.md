# Material Expressive Buttons

This document is the living audit and migration contract for Nocky's Material
Expressive button checkpoint.

References:

- Buttons: <https://m3.material.io/components/buttons>
- Icon buttons: <https://m3.material.io/components/icon-buttons>
- Chips: <https://m3.material.io/components/chips>

## Status

- Foundation CSS: implemented in `100-buttons.css`.
- Shared Rust class contract: implemented in `material_button.rs`.
- Existing controls migrated: Settings pilots, dialog/browser/app-shell labeled
  actions, the first icon-button batch and the first YouTube chip batch.
- Visual behavior changed only for explicitly audited controls in Material
  Expressive and Frosted Glass.
- The button CSS does not style `.theme-noctalia`; Noctalia keeps its own
  shell/theme cascade instead of acting as a Material variant.
- Local automated validation passes.
- Next step: continue chips and specialized popover buttons after review.

The inventory below records the currently identified button families. It will be
expanded to one row per construction site before the checkpoint is marked ready
for review.

## Design rules

1. Button hierarchy communicates action priority. Filled is reserved for the
   highest-priority action in a visual group.
2. Filled tonal is used for important supporting actions.
3. Elevated is used only when a button needs separation from a complex,
   translucent or artwork-backed surface.
4. Outlined represents an important alternative action.
5. Text represents low-emphasis navigation, disclosure or cancellation.
6. Destructive is a semantic modifier, not a separate structural variant.
7. Hover must never change widget allocation.
8. Focus remains visible for keyboard users.
9. Loading must preserve width and use the shared Material loading indicator.
10. Noctalia is not a Material variant; Material button, icon-button and chip
    CSS is scoped to Material Expressive and Frosted Glass only.
11. Frosted Glass retains translucency and surface separation.
12. Chips are compact selection/filter controls and must not inherit
    high-priority action classes.

## Preserved architectures

The following controls are excluded from replacement by the common button
helper unless a later audit documents a concrete accessibility defect:

- `ExpressiveTransport` and its play/previous/next buttons;
- the main-player and footer transport layouts;
- `new_mode_toggle`, repeat and shuffle;
- the moving indicator and layout engine inside `AnimatedPageSwitcher`;
- cards that use `GtkButton` as a full clickable surface;
- clickable lyric labels;
- `WaveProgress` and custom volume controls.

They may share semantic color roles, focus treatment and accessibility fixes,
but their existing motion and layout ownership must remain intact.

## Inventory by surface

| Surface | Source | Current family | Recommended treatment | States | Migration decision |
| --- | --- | --- | --- | --- | --- |
| Header sidebar toggle | `src/app/controller/construction.rs` | icon toggle | standard icon button | hover, focus, pressed, selected | migrated |
| Header player collapse | `src/app/controller/construction.rs` | flat icon button | standard icon button | hover, focus, pressed | migrated |
| Header search | `src/app/controller/construction.rs` | icon toggle | standard icon button | hover, focus, pressed, selected | migrated |
| Header sync | `src/app/controller/construction.rs` | flat icon button | standard icon button | hover, focus, pressed | migrated |
| Header folder picker | `src/app/controller/construction.rs` | icon button | standard icon button | hover, focus, pressed | migrated |
| Header settings navigation | `src/app/controller/construction.rs` | flat icon toggle | standard icon button | hover, focus, pressed, selected | migrated |
| Sidebar rows | `src/app/sidebar.rs` | flat full-width buttons | navigation rows, not common buttons | hover, focus, selected, disabled | preserve architecture |
| Top page switcher | `src/ui/widgets/animated_page_switcher.rs` | custom button group | segmented/button-group semantics | hover, focus, selected, reduced motion | preserve architecture |
| Empty-library action | `src/app/controller/construction.rs` | suggested pill | filled button | hover, focus, pressed, disabled | migrated |
| Queue clear upcoming | `src/app/controller/construction.rs` | pill | outlined button | hover, focus, pressed, disabled | migrated |
| Queue clear all | `src/app/controller/construction.rs` | destructive pill | destructive outlined button | hover, focus, pressed, confirmation | migrated |
| Player favorite | `src/ui/player/view.rs` | flat card icon | standard icon button | hover, focus, pressed | migrated |
| Player inline lyrics | `src/ui/player/view.rs` | flat toggle icon | standard icon button | hover, focus, pressed, selected | migrated |
| Player refresh lyrics | `src/ui/player/view.rs` | flat icon | standard icon button | hover, focus, pressed | migrated |
| Main transport | `src/ui/player/view.rs` | custom transport buttons | keep `ExpressiveTransport` | hover, focus, pressed, playing | preserve architecture |
| Repeat and shuffle | `src/mode_toggle.rs` | custom toggle buttons | keep `new_mode_toggle` | hover, focus, pressed, checked | preserve architecture |
| Footer transport | `src/ui/footer/transport.rs` | custom transport buttons | keep `ExpressiveTransport` | hover, focus, pressed, playing | preserve architecture |
| Footer lyrics | `src/ui/footer/utilities.rs` | toggle icon | standard icon button | hover, focus, pressed, selected | migrated |
| Footer mute | `src/ui/footer/utilities.rs` | flat icon | standard icon button | hover, focus, pressed | migrated |
| Settings clear history | `src/ui/settings/page.rs` | destructive labeled button | destructive outlined | hover, focus, pressed, confirmation | pilot migrated |
| Settings manage YouTube | `src/ui/settings/page.rs` | suggested primary action | filled button | hover, focus, pressed, loading | pilot migrated |
| Settings open offline folder | `src/ui/settings/page.rs` | generic labeled button | outlined button | hover, focus, pressed | pilot migrated |
| Settings clean partials | `src/ui/settings/page.rs` | generic labeled button | filled tonal button | hover, focus, pressed, disabled | migrated |
| Settings remove downloads | `src/ui/settings/page.rs` | destructive labeled button | destructive filled tonal | hover, focus, pressed, disabled, confirmation | migrated |
| Settings diagnostics disclosure | `src/ui/settings/page.rs` | row action | text button | hover, focus, pressed, selected/disclosed | pilot migrated |
| Settings diagnostics rerun | `src/ui/settings/page.rs` | row action | filled tonal button | hover, focus, pressed, disabled, loading class | migrated |
| Settings copy report | `src/ui/settings/page.rs` | primary row action | filled button | hover, focus, pressed | migrated |
| Settings about | `src/ui/settings/page.rs` | primary row action | filled tonal button | hover, focus, pressed | migrated |
| Settings shortcuts | `src/ui/settings/page.rs` | row action | outlined button | hover, focus, pressed | migrated |
| Startup local source | `src/dialogs.rs` | source-choice button | outlined button | hover, focus, pressed | migrated |
| Startup YouTube source | `src/dialogs.rs` | suggested source-choice | filled button | hover, focus, pressed | migrated |
| Startup cancel | `src/dialogs.rs` | low-emphasis action | text button | hover, focus, pressed | migrated |
| Onboarding back | `src/onboarding.rs` | generic labeled button | outlined button | hover, focus, pressed, disabled | migrated |
| Onboarding next | `src/onboarding.rs` | suggested action | filled tonal button | hover, focus, pressed | migrated |
| Onboarding finish | `src/onboarding.rs` | suggested action | filled button | hover, focus, pressed | migrated |
| Stream source configure | `src/ui/settings/stream_sources.rs` | suggested action | filled tonal button | hover, focus, pressed | migrated |
| Stream source move up/down | `src/ui/settings/stream_sources.rs` | flat icon buttons | standard icon buttons | hover, focus, pressed, disabled | migrated |
| Stream source reset | `src/ui/settings/stream_sources.rs` | flat labeled button | text button | hover, focus, pressed | migrated |
| Assisted-login cancel | `src/youtube/assisted_login.rs` | flat labeled button | text button | hover, focus, pressed | migrated |
| Home card surface | `src/browser.rs` | full-card button | clickable surface, not common button | hover, focus, pressed, playing | preserve architecture |
| Home card play/context | `src/browser.rs` | contextual icon action | filled/elevated icon button | hover, focus, pressed, selected | later icon checkpoint |
| Home card overflow | `src/browser.rs` | compact icon button | standard icon button with 48 px target | hover, focus, pressed | later icon checkpoint |
| Local playlist create | `src/browser.rs` | suggested labeled button | filled button | hover, focus, pressed | migrated |
| Local playlist add current | `src/browser.rs` | labeled button | filled tonal button | hover, focus, pressed | migrated |
| Local playlist remove current | `src/browser.rs` | labeled button | outlined button | hover, focus, pressed | migrated |
| Local playlist delete | `src/browser.rs` | destructive labeled button | destructive outlined button | hover, focus, pressed | migrated |
| Collection offline action | `src/browser.rs` | stateful labeled button | filled tonal or outlined by state | ready, loading, complete, retry, disabled | migrated |
| Search/load-more actions | `src/browser.rs` | labeled actions | filled tonal by hierarchy | hover, focus, pressed, loading | migrated |
| YouTube Home filter chips | `src/browser.rs` | pill/suggested action | filter chip | hover, focus, pressed, selected | migrated |
| YouTube result filter chips | `src/youtube/mod.rs` | pill/suggested action | filter chip | hover, focus, pressed, selected | migrated |
| YouTube account actions | `src/youtube/mod.rs` | suggested/flat actions | filled, outlined and text buttons | hover, focus, pressed, disabled | migrated |
| YouTube search and sync | `src/youtube/mod.rs` | suggested actions | filled and filled tonal buttons | hover, focus, pressed, disabled | migrated |
| YouTube private navigation | `src/youtube/mod.rs` | pill actions | assist/suggestion chips | hover, focus, pressed, disabled | migrated |
| YouTube create playlist dialog | `src/youtube/playlist_create.rs` | suggested/flat dialog actions | filled and text buttons | hover, focus, pressed, disabled | migrated |
| YouTube add current to playlist | `src/app/controller/youtube_playlist_add.rs` | suggested pill action | filled tonal button | hover, focus, pressed, disabled, pending | migrated |

## Foundation class contract

All common labeled buttons receive:

- `material-button`;
- one size class: `material-button-compact`, `material-button-standard`, or
  `material-button-large`;
- one variant class: `material-button-filled`,
  `material-button-filled-tonal`, `material-button-elevated`,
  `material-button-outlined`, or `material-button-text`;
- optional `material-button-destructive`;
- optional state classes `material-button-selected` and
  `material-button-loading`.

The class contract is deliberately separate from old generic GTK/libadwaita
classes such as `suggested-action`, `destructive-action`, `pill`, and `flat`.
During migration, old classes must be removed from each audited control to avoid
cascade conflicts.

All common icon buttons receive:

- `material-icon-button`;
- one variant class: `material-icon-button-standard`,
  `material-icon-button-filled`, `material-icon-button-filled-tonal`, or
  `material-icon-button-outlined`;
- optional state class `material-icon-button-selected`.

Icon buttons are for compact supplementary actions and keep a stable 40 px
target so hover, selected and pressed states do not resize toolbar rows.

All common chips receive:

- `material-chip`;
- one variant class: `material-chip-assist`, `material-chip-filter`,
  `material-chip-input`, or `material-chip-suggestion`;
- optional state class `material-chip-selected`.

Filter chips use the selected state instead of legacy `suggested-action`, so
they remain visually distinct from primary actions while preserving compact rail
geometry.

## Pilot migration result

The first GTK helper integration migrated four Settings controls:

1. Manage YouTube — filled.
2. Open offline folder — outlined.
3. Clear listening history — destructive outlined.
4. Diagnostics disclosure — text with selected state while expanded.

The second Settings batch migrated six more labeled controls:

1. Clean incomplete downloads — filled tonal.
2. Remove downloads — destructive filled tonal.
3. Diagnostics rerun — filled tonal with `material-button-loading` while
   checks refresh.
4. Copy diagnostics report — filled.
5. About details — filled tonal.
6. Keyboard shortcuts — outlined.

No transport control, card surface, page switcher or icon-only action is
changed by these checkpoints.

The first dialog/onboarding batch migrated:

1. Startup source choices — local outlined, YouTube filled.
2. Startup cancel — text.
3. Onboarding navigation — back outlined, next filled tonal, finish filled.
4. Stream-source configure — filled tonal.
5. Stream-source reset — text.
6. Assisted-login cancel — text.

The first browser labeled-action batch migrated:

1. Local playlist manager actions — create filled, add filled tonal, remove
   outlined and delete destructive outlined.
2. Search load-more and YouTube Home load-more — filled tonal.
3. Collection offline action — stateful filled tonal, with loading class while
   downloading and outlined retry state.

The app-shell labeled-action batch migrated:

1. Empty-library add-folder action — filled.
2. Queue clear upcoming — outlined.
3. Queue clear all — destructive outlined.

The first icon-button batch migrated:

1. Header navigation/search/sync/folder/settings/player-collapse actions —
   standard icon buttons.
2. Player favorite, inline lyrics and refresh lyrics — standard icon buttons
   with selected state on the lyrics toggle.
3. Footer lyrics and mute — standard icon buttons with selected state on the
   lyrics toggle.
4. Stream-source move up/down controls — standard icon buttons with disabled
   bounds preserved.

The first chip batch migrated:

1. YouTube Home filter rail — `Tudo` and remote-provided chips.
2. YouTube result filter rail — `Tudo` and remote-provided chips.

The second YouTube action batch migrated:

1. Account/authentication actions — browser login filled, manual import and
   browser-open outlined, cancel/disconnect text.
2. Search and sync actions — search filled, sync filled tonal.
3. Private YouTube navigation — assist chips plus create-playlist suggestion
   chip.
4. Create-playlist dialog actions — cancel text, create filled.
5. Add-current-to-playlist action — filled tonal compact with pending disabled
   state preserved.

## Loading contract for a later checkpoint

Loading buttons will use a stable internal stack containing normal content and
the shared `MaterialLoadingIndicator`. Loading must not be represented only by
changing the label. The implementation must preserve minimum width, expose a
stable accessible name, disable duplicate activation, and restore the previous
state on success, failure or cancellation.

The diagnostics rerun action is the first shared loading-content consumer. It
uses a homogeneous stack with the normal label and the shared
`MaterialLoadingIndicator`, disables duplicate activation and restores the label
once the refresh timeout updates the report.

The YouTube Home load-more action uses the same shared loading-content helper
while the next continuation request is dispatched, replacing the old label-only
loading feedback.

## Manual validation matrix

Validate the migrated Settings buttons in Material Expressive, Noctalia and
Frosted Glass, including keyboard focus, hover, pressed, disabled, selected,
confirmation and diagnostics loading states. Reduced-motion behavior and
remaining non-Settings controls stay in later checkpoints.

Automated validation for this checkpoint was run with `cargo fmt` and
`cargo test`.

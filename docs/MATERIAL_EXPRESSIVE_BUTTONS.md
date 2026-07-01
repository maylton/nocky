# Material Expressive Buttons

This document is the living audit and migration contract for Nocky's Material
Expressive button checkpoint.

Reference: <https://m3.material.io/components/all-buttons>

## Status

- Foundation CSS: implemented in `100-buttons.css`.
- Existing controls migrated: none yet.
- Visual behavior changed: none yet; the new classes are opt-in.
- Next step: introduce the GTK helper API and migrate four Settings pilots.

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
10. Noctalia retains ownership of Shell palette roles.
11. Frosted Glass retains translucency and surface separation.

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
| Header sidebar toggle | `src/app/controller/construction.rs` | icon toggle | toggle icon button | hover, focus, pressed, selected | later icon checkpoint |
| Header player collapse | `src/app/controller/construction.rs` | flat icon button | standard icon button | hover, focus, pressed | later icon checkpoint |
| Header search | `src/app/controller/construction.rs` | icon toggle | toggle icon button | hover, focus, pressed, selected | later icon checkpoint |
| Header sync | `src/app/controller/construction.rs` | flat icon button | icon button, tonal only while pending | hover, focus, pressed, loading | later icon/loading checkpoint |
| Header folder picker | `src/app/controller/construction.rs` | icon button | standard icon button | hover, focus, pressed | later icon checkpoint |
| Header settings navigation | `src/app/controller/construction.rs` | flat icon toggle | toggle icon button | hover, focus, pressed, selected | later icon checkpoint |
| Sidebar rows | `src/app/sidebar.rs` | flat full-width buttons | navigation rows, not common buttons | hover, focus, selected, disabled | preserve architecture |
| Top page switcher | `src/ui/widgets/animated_page_switcher.rs` | custom button group | segmented/button-group semantics | hover, focus, selected, reduced motion | preserve architecture |
| Empty-library action | `src/app/controller/construction.rs` | suggested pill | filled button | hover, focus, pressed, disabled | labeled checkpoint |
| Queue clear upcoming | `src/app/controller/construction.rs` | pill | outlined button | hover, focus, pressed, disabled | labeled checkpoint |
| Queue clear all | `src/app/controller/construction.rs` | destructive pill | destructive tonal/outlined | hover, focus, pressed, confirmation | labeled checkpoint |
| Player favorite | `src/ui/player/view.rs` | flat card icon | toggle icon button | hover, focus, pressed, selected | later icon checkpoint |
| Player inline lyrics | `src/ui/player/view.rs` | flat toggle icon | toggle icon button | hover, focus, pressed, selected | later icon checkpoint |
| Player refresh lyrics | `src/ui/player/view.rs` | flat icon | standard icon button | hover, focus, pressed, loading | later icon/loading checkpoint |
| Main transport | `src/ui/player/view.rs` | custom transport buttons | keep `ExpressiveTransport` | hover, focus, pressed, playing | preserve architecture |
| Repeat and shuffle | `src/mode_toggle.rs` | custom toggle buttons | keep `new_mode_toggle` | hover, focus, pressed, checked | preserve architecture |
| Footer transport | `src/ui/footer/transport.rs` | custom transport buttons | keep `ExpressiveTransport` | hover, focus, pressed, playing | preserve architecture |
| Footer lyrics | `src/ui/footer/utilities.rs` | toggle icon | toggle icon button | hover, focus, pressed, selected | later icon checkpoint |
| Footer mute | `src/ui/footer/utilities.rs` | flat icon | toggle-like icon button | hover, focus, pressed, muted | later icon checkpoint |
| Settings clear history | `src/ui/settings/page.rs` | destructive labeled button | destructive outlined | hover, focus, pressed, confirmation | pilot candidate |
| Settings manage YouTube | `src/ui/settings/page.rs` | suggested primary action | filled button | hover, focus, pressed, loading | pilot candidate |
| Settings open offline folder | `src/ui/settings/page.rs` | generic labeled button | outlined button | hover, focus, pressed | pilot candidate |
| Settings clean partials | `src/ui/settings/page.rs` | generic labeled button | filled tonal button | hover, focus, pressed, disabled, loading | labeled/loading checkpoint |
| Settings remove downloads | `src/ui/settings/page.rs` | destructive labeled button | destructive filled tonal | hover, focus, pressed, disabled, confirmation | labeled checkpoint |
| Settings diagnostics disclosure | `src/ui/settings/page.rs` | row action | text button | hover, focus, pressed, selected/disclosed | pilot candidate |
| Settings diagnostics rerun | `src/ui/settings/page.rs` | row action | filled tonal button | hover, focus, pressed, disabled, loading | loading checkpoint |
| Settings copy report | `src/ui/settings/page.rs` | primary row action | filled button | hover, focus, pressed, success feedback | labeled checkpoint |
| Settings about | `src/ui/settings/page.rs` | primary row action | filled tonal button | hover, focus, pressed | labeled checkpoint |
| Settings shortcuts | `src/ui/settings/page.rs` | row action | outlined or text button | hover, focus, pressed | labeled checkpoint |
| Startup local source | `src/dialogs.rs` | source-choice button | outlined button | hover, focus, pressed | labeled checkpoint |
| Startup YouTube source | `src/dialogs.rs` | suggested source-choice | filled button | hover, focus, pressed | labeled checkpoint |
| Startup cancel | `src/dialogs.rs` | low-emphasis action | text button | hover, focus, pressed | labeled checkpoint |
| Onboarding back | `src/onboarding.rs` | generic labeled button | outlined button | hover, focus, pressed, disabled | labeled checkpoint |
| Onboarding next | `src/onboarding.rs` | suggested action | filled button | hover, focus, pressed | labeled checkpoint |
| Onboarding finish | `src/onboarding.rs` | suggested action | filled button | hover, focus, pressed | labeled checkpoint |
| Stream source configure | `src/ui/settings/stream_sources.rs` | suggested action | filled tonal button | hover, focus, pressed | labeled checkpoint |
| Stream source move up/down | `src/ui/settings/stream_sources.rs` | flat icon buttons | standard icon buttons | hover, focus, pressed, disabled | later icon checkpoint |
| Assisted-login cancel | `src/youtube/assisted_login.rs` | flat labeled button | text button | hover, focus, pressed | labeled checkpoint |
| Home card surface | `src/browser.rs` | full-card button | clickable surface, not common button | hover, focus, pressed, playing | preserve architecture |
| Home card play/context | `src/browser.rs` | contextual icon action | filled/elevated icon button | hover, focus, pressed, selected | later icon checkpoint |
| Home card overflow | `src/browser.rs` | compact icon button | standard icon button with 48 px target | hover, focus, pressed | later icon checkpoint |
| Collection offline action | `src/browser.rs` | stateful labeled button | filled/tonal stateful button | ready, loading, complete, retry, disabled | loading checkpoint |
| Search/load-more actions | `src/browser.rs` | labeled actions | filled tonal or text by hierarchy | hover, focus, pressed, loading | loading checkpoint |

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

## Pilot migration plan

The first GTK helper integration will migrate exactly four Settings controls:

1. Manage YouTube — filled.
2. Open offline folder — outlined.
3. Clear listening history — destructive outlined.
4. Diagnostics disclosure — text.

No other control will be changed in that commit.

## Loading contract for a later checkpoint

Loading buttons will use a stable internal stack containing normal content and
the shared `MaterialLoadingIndicator`. Loading must not be represented only by
changing the label. The implementation must preserve minimum width, expose a
stable accessible name, disable duplicate activation, and restore the previous
state on success, failure or cancellation.

## Manual validation matrix

Manual validation is deferred until the first pilot controls are wired. At that
point validate Material Expressive, Noctalia and Frosted Glass in light/dark
contexts, keyboard focus, hover, pressed, disabled, selected, loading, reduced
motion, narrow layouts and all supported interface languages.

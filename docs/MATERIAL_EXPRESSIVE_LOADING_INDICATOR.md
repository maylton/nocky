# Material Expressive loading indicator checkpoint

This checkpoint introduces one reusable native GTK loading indicator and
rebaselines visual-system work as the active product priority.

## Loading-state inventory

| Surface | Current treatment | Classification | Checkpoint action |
| --- | --- | --- | --- |
| YouTube page header during search, sync, account validation and playlist creation | Shared header indicator plus status text | Indeterminate page loading | Migrated to `MaterialLoadingIndicator` |
| YouTube Home chip/filter refresh banner | Inline row with indicator and localized message | Indeterminate inline loading | Migrated |
| YouTube playlist, album and artist uncached routes | Centered list row with indicator and text | Indeterminate page/section loading | Migrated |
| YouTube collection card play action while loading | Icon button swaps to loading indicator | Indeterminate inline/action loading | Migrated |
| Assisted browser login window | `GtkSpinner` plus localized status | Indeterminate page/dialog loading | Migrated; remaining generic spinner removed |
| Search results while remote search is pending and cached results exist | Status banner text | Background activity over valid cache | Not migrated; cached content remains visible |
| YouTube Home continuation/load-more button | Button label changes to “Loading…” | Indeterminate inline/action loading | Not migrated in this checkpoint to avoid button-width churn |
| Playlist creation dialog submit button | Button disabled while background operation runs through page header loading | Indeterminate action loading | Covered by YouTube page header; button layout unchanged |
| Add-current-to-playlist button | Label changes to pending state | Indeterminate action loading | Not migrated; follow-up should reserve stable button width first |
| Offline collection download | Text shows completed/total counts | Determinate operation progress | Not migrated; backend exposes counts, but this checkpoint avoids offline-download UI churn |
| Onboarding import progress | `GtkProgressBar` | Determinate operation progress | Not migrated; onboarding flow is not a loading indicator target yet |
| Queue accent line/progress | `GtkProgressBar` decorative/progress accent | Non-loading progress | Not migrated |
| Playback progress, WaveSeekBar, volume and duration | Dedicated playback controls | Non-loading progress | Explicitly out of scope |
| Cover placeholders and collection skeleton cards | Placeholder/skeleton content | Skeleton/placeholder loading | Not migrated; skeletons are the correct treatment |
| Debounced search/save timers and queue pending entry | Internal async state | Background activity without indicator | Not migrated |

## Widget architecture

`MaterialLoadingIndicator` lives in `src/ui/widgets/expressive_loading.rs` to
avoid a broader module reorganization. It is a small `gtk::DrawingArea` wrapper
with pure geometry helpers and a lifecycle-managed GTK tick callback.

The public API supports:

- `LoadingIndicatorMode::Indeterminate`;
- `LoadingIndicatorMode::Determinate(f64)`;
- `LoadingIndicatorPresentation::{Uncontained, Contained}`;
- `LoadingIndicatorSize::{Compact, Standard, Large}`.

Indeterminate mode rotates continuously and morphs through a deterministic
sequence of normalized rounded shapes. Determinate mode clamps progress to
`0.0..=1.0`, maps progress to the same shape sequence and exposes accessibility
progress values as percentages.

The shape sequence is generated from normalized geometry, so the visual center
and bounds stay stable across sizes and fractional scaling. The contained
variant draws a stable tonal container while only the active shape morphs.

## Lifecycle and reduced motion

The widget starts a single frame-clock tick only when visible, mapped,
indeterminate and animations are enabled. It removes that callback when hidden,
unmapped or reduced motion is active. Repeated show/hide cycles do not register
duplicate callbacks.

When GTK/libadwaita animations are disabled, the indicator uses one stable
rounded Material shape and does not continuously rotate, morph or pulse.

## Theme roles

The widget uses CSS classes rather than hardcoded theme colors:

- `.material-loading-indicator`;
- `.material-loading-indicator.compact`;
- `.material-loading-indicator.standard`;
- `.material-loading-indicator.large`;
- `.material-loading-indicator.contained`;
- `.material-loading-indicator.uncontained`.

Base, Material Expressive and dynamic-palette CSS map the indicator to semantic
accent/primary roles. Button-hosted indicators inherit the button foreground so
inline pending states remain visible on tonal buttons.

## Accessibility

The drawing area uses `ProgressBar` as its GTK accessible role. Indeterminate
instances expose a stable loading label and busy state without updating that
label every frame. Determinate instances expose min, max and current values.

## Migrated call sites

- `src/youtube/mod.rs`: YouTube page header loading.
- `src/browser.rs`: Home refresh banner, uncached playlist/collection loading
  rows and inline collection play loading.
- `src/youtube/assisted_login.rs`: assisted-login `GtkSpinner` replacement.

## Manual visual validation checklist

- [ ] Material light theme.
- [ ] Material dark theme.
- [ ] Noctalia theme.
- [ ] Frosted Glass theme.
- [ ] Dynamic artwork palette active.
- [ ] Compact inline indicator.
- [ ] Standard page indicator.
- [ ] Large contained indicator.
- [ ] Contained and uncontained variants.
- [ ] Determinate values at 0%, 25%, 50%, 75% and 100%.
- [ ] Indeterminate animation for several complete shape cycles.
- [ ] Narrow window.
- [ ] Wide window.
- [ ] HiDPI or fractional scaling when available.
- [ ] System animations enabled.
- [ ] System animations disabled/reduced motion.
- [ ] Keyboard navigation around pending buttons.
- [ ] Screen-reader/accessibility inspection where available.
- [ ] Repeated show/hide and navigation to detect leaked callbacks.
- [ ] Long-running loading state for CPU and animation stability.
- [ ] Runtime logs checked for GTK/GLib criticals, allocation warnings and
  repeated callback leaks.

## Validation status

- Automated validation passed on 2026-07-01: formatting, whitespace diff check,
  focused loading-indicator tests, full all-target test suite, all-features
  clippy, no-default-features test suite, no-default-features clippy and
  `scripts/quality-gate.sh`.
- Runtime smoke used `timeout 25s cargo run`. The application started and stayed
  alive until the timeout. The log still showed environment/rendering messages
  already seen in this workspace: Vulkan `VK_SUBOPTIMAL_KHR` swapchain warnings
  and pixman invalid-rectangle diagnostics. No loading-indicator-specific
  callback or lifecycle warnings were observed in that short run.
- Visual checklist items above remain open until they are inspected interactively
  and screenshots or recordings are attached to the draft PR.

## Known follow-up work

- Migrate load-more buttons after reserving stable inline indicator space.
- Migrate playlist add/edit mutation buttons after their pending layouts are
  width-stable.
- Add a small developer/demo surface for determinate visual inspection if no
  production determinate loading operation is ready.
- Continue the remaining visual-system checkpoints listed in `ROADMAP.md`.

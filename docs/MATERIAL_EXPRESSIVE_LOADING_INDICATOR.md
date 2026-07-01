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
| Assisted browser login window | Generic spinner plus localized status | Indeterminate page/dialog loading | Migrated; remaining generic spinner removed |
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
with pure geometry helpers and one lifecycle-managed GTK frame-clock callback.

The API supports:

- `LoadingIndicatorMode::Indeterminate`;
- `LoadingIndicatorMode::Determinate(f64)`;
- `LoadingIndicatorPresentation::{Uncontained, Contained}`;
- `LoadingIndicatorSize::{Compact, Standard, Large}`.

Indeterminate mode rotates and morphs through seven compatible rounded shapes.
Its phase is derived from elapsed frame-clock time, so a complete cycle keeps the
same duration at 30, 60, 120 or 144 Hz.

Determinate mode is intentionally separate: progress between `0.0` and `1.0`
morphs a circle into a soft burst while rotating through half a turn. Progress
updates interpolate toward the newest target and the tick stops automatically
when the displayed value converges.

All shapes use normalized compatible point topology and cubic curves. Their
bounds and visual center remain stable across sizes and fractional scaling.

## Lifecycle and reduced motion

The widget owns at most one frame callback. It starts only while the widget is
visible, mapped, animations are enabled and animation work remains.

The callback stops when:

- the widget is hidden or unmapped;
- the owning view is destroyed;
- system animations are disabled;
- determinate progress reaches its target.

The widget observes live changes to `gtk-enable-animations` and also checks the
setting inside the frame callback. Reduced-motion indeterminate state uses one
stable rounded shape without rotation, morphing or pulsing. Determinate state
settles directly on the current target.

## Theme roles

The widget uses semantic CSS classes rather than hardcoded colors:

- `.material-loading-indicator`;
- `.material-loading-indicator.compact`;
- `.material-loading-indicator.standard`;
- `.material-loading-indicator.large`;
- `.material-loading-indicator.contained`;
- `.material-loading-indicator.uncontained`.

`099-loading-indicator.css` provides contained roles for all three visual
identities:

- Material Expressive: `PrimaryContainer` and `OnPrimaryContainer`;
- Noctalia: system accent and accent foreground;
- Frosted Glass: translucent album-toned container with a subtle outline.

The dynamic-palette provider appends the same semantic Material/Frosted rules
after every album-palette update, so the higher-priority provider cannot replace
the contained foreground with the plain primary role. Button-hosted indicators
continue to inherit the button foreground and keep a transparent container.

## Accessibility

The drawing area uses `ProgressBar` as its GTK accessible role. Call sites supply
localized accessible labels in Portuguese, English or Spanish.

Indeterminate instances expose one stable busy state without announcing every
frame. Determinate instances expose minimum, maximum and the currently displayed
percentage. Busy state is cleared when the determinate indicator completes or
the widget becomes hidden.

## Migrated call sites

- `src/youtube/mod.rs`: YouTube page header loading.
- `src/browser.rs`: Home refresh banner, uncached playlist/collection loading
  rows and inline collection play loading.
- `src/youtube/assisted_login.rs`: assisted-login spinner replacement.

## Automated coverage

Regression tests cover:

- progress clamping;
- circle and soft-burst determinate endpoints;
- continuity between adjacent indeterminate shapes;
- normalized bounds and center stability;
- one closed cubic path for rounded rendering;
- frame-rate-independent phase progression;
- determinate target convergence;
- reduced-motion settlement;
- style-class mapping;
- accessibility percentage conversion;
- shared-component use at migrated call sites.

Quality Gate #543 passed the hardened animation and accessibility core. Quality
Gate #546 passed the semantic theme module and dynamic-palette integration.
Quality Gate #547 passed on the final documented head, including the complete
repository test and strict Clippy matrix.

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

The original short runtime smoke remained alive until its timeout and did not
show loading-indicator-specific lifecycle warnings. The visual checklist remains
open until the branch is inspected interactively and screenshots or a recording
are attached to the draft PR.

## Known follow-up work

- Migrate load-more buttons after reserving stable inline indicator space.
- Migrate playlist add/edit mutation buttons after their pending layouts are
  width-stable.
- Add a small developer/demo surface for determinate visual inspection if no
  production determinate loading operation is ready.
- Continue the remaining visual-system checkpoints listed in `ROADMAP.md`.

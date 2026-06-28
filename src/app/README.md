# Application architecture

The `app` module owns application startup, high-level GTK state, and the
controller that coordinates UI events with playback, persistence, YouTube, and
offline work.

`application.rs` exposes the process-level entry point used by `main.rs`.
`controller/mod.rs` defines `AppController` and declares focused controller
modules:

- `construction.rs`: application/window construction and initial widget wiring.
- `callbacks.rs`: periodic GTK callbacks and player event polling.
- `actions.rs`: application actions and shortcuts.
- `navigation.rs`: browser routes, source-aware navigation, and browser events.
- `queue.rs`: queue state, source switching, persistence, enqueue, and shuffle.
- `queue_presentation.rs`: queue page and popover rendering.
- `playback.rs`: local playback, transport controls, progress, resume, and MPRIS.
- `youtube.rs`: YouTube library, search, collection playback, status, and likes.
- `offline.rs`: offline downloads, followed collections, and sync status.
- `lyrics.rs`: lyrics requests, refresh, and timed-line updates.
- `appearance.rs`: visual theme, footer, translations, toasts, and dialogs.
- `settings.rs`: settings, onboarding, and startup source events.
- `local_library.rs`: local library loading, scanning, and folder selection.
- `persistence.rs`: config, playback session, and listening history persistence.
- `background.rs`: application of `BackgroundMessage` results.
- `favorites.rs` and `feedback.rs`: small favorite-state and user-feedback helpers.

`AppController` keeps GTK handles directly, while non-visual coordination state
lives in `ControllerRuntime`. A temporary `Deref` implementation preserves
existing field access while the controller modules migrate toward explicit
context accessors. Future passes can split that runtime into playback, queue,
YouTube and persistence contexts without another large mechanical rewrite.

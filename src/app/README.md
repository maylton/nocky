# Application lifecycle

Phase 4 begins the gradual reduction of `main.rs`.

The process entry point now delegates to `app::run()`, while the original
startup body lives in `application.rs`. The module currently imports the crate
root so existing private application structures remain available without
changing behavior.

Future steps can move window construction and `AppController` into focused
modules after this entry-point extraction is stable.

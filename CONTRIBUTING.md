# Contributing to Nocky

Thank you for helping improve Nocky.

## Development setup

Install the GTK4, libadwaita and GStreamer development packages for your distribution, then run:

```bash
cargo fmt --check
cargo check
python3 -m py_compile helpers/nocky_youtube.py
cargo run
```

The universal installer can install common build dependencies without copying files:

```bash
./install.sh --install-deps --build-only
```

## Before opening a pull request

- Keep the interface consistent with GTK4/libadwaita conventions.
- Do not copy Noctalia artwork or internal code; Nocky only follows compatible design concepts and color roles.
- Run `cargo fmt --check` and `cargo check`.
- Run `./scripts/verify-release.sh` when changing packaging or metadata.
- Keep user-visible strings clear and concise.
- Never commit browser cookies, copied cURL requests, `.env` files, YouTube session files or cached stream URLs.
- Run `./scripts/check-youtube.sh` when changing the optional YouTube runtime.

## Bug reports

Include the distribution, desktop/compositor, GTK/libadwaita/GStreamer versions, terminal output and exact reproduction steps.

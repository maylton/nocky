# Native playlist metadata integration

This document tracks Phase 12D2, the native integration of the read-only
playlist editability contract.

## Current checkpoint

- The Python metadata helper and normalizer are present on `main`.
- The Rust ownership, privacy and occurrence-identity model is present on
  `main`.
- The complete Quality Gate explicitly compiles both Python metadata modules.
- No native edit control or remote mutation is exposed.

## Remaining work

1. Add the metadata helper and normalizer to installed runtime packaging.
2. Register the Rust metadata model in the YouTube domain.
3. Add a read-only bridge call that invokes the sanitized helper.
4. Load metadata asynchronously when an authenticated playlist detail opens.
5. Surface only a non-interactive ownership/privacy/editability diagnostic.
6. Keep browsing and playback available when metadata loading fails.
7. Add integration tests for unavailable, unowned, incomplete and duplicate
   occurrence responses.

## Safety boundary

This checkpoint does not authorize adding, removing, reordering, renaming or
deleting playlist content. A later mutation may only proceed when authenticated
ownership and the operation-specific identity requirements are satisfied.

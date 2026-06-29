# Read-only YouTube Music playlist editability contract

This document defines Phase 12D1. The slice preserves authenticated playlist
metadata required by future operations, but it does not expose or execute any
playlist mutation.

## Purpose

The existing Nocky item model is sufficient for browsing and playback, but it
does not prove whether a playlist can be edited safely. Phase 12D1 introduces a
separate read-only contract rather than overloading every search and feed item
with mutation-specific fields.

## Python allowlist

The normalized playlist detail contains only:

- `playlist_id`;
- `title`;
- `owned`;
- `privacy`;
- `editable`;
- `tracks`.

Each track occurrence contains only:

- `video_id`;
- `set_video_id`;
- `title`.

Duplicate `video_id` values are preserved because one track may occur multiple
times in the same playlist. Each occurrence can have a different
`set_video_id`.

## Editability semantics

The Python normalizer emits `editable: true` only when:

- the authenticated response explicitly contains `owned: true`; and
- a non-empty playlist ID is available.

The Rust model validates the same condition again. An incoming `editable: true`
value cannot override missing ownership or a missing playlist ID.

Unknown privacy values degrade to `Unknown`; the application must not invent a
privacy state.

## Read-only helper

`nocky_youtube_playlist.py`:

1. requires the existing connected browser session;
2. accepts a playlist ID, not a URL;
3. strips an optional `VL` browse prefix;
4. restricts the request limit to 1–500;
5. calls `ytmusicapi.get_playlist()`;
6. immediately applies the allowlisted normalizer;
7. emits the normalized result as JSON.

The helper does not:

- create, rename or delete a playlist;
- add, remove or reorder tracks;
- write playlist metadata to disk;
- change authentication or profile state;
- follow URLs returned by the service;
- return complete upstream response objects.

## Rust compatibility

The Rust structures use Serde defaults. Payloads produced before Phase 12D1
remain readable and default to a non-editable state.

The model exposes read-only checks for:

- effective editability;
- known privacy classification;
- number of track occurrences with complete removal identity;
- duplicate `set_video_id` detection.

These checks do not authorize a network request. The separate Phase 12D safety
contract remains mandatory for future operations.

## Current delivery boundary

This slice includes normalization, helper access, Rust models, tests and Quality
Gate coverage.

It does not wire the helper into GTK, add buttons, cache editability metadata or
execute a remote mutation. Native integration will be a later checkpoint after
the live response shape is validated against a connected owned playlist.

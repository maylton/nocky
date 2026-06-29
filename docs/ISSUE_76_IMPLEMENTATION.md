# Phase 12D4: add the current YouTube track to an owned playlist

This checkpoint exposes one native playlist-membership action with a deliberately
small scope.

## Native eligibility

The action is rendered only while all of these conditions hold:

- the open route is a YouTube Music playlist;
- authenticated read-only metadata confirms effective editability, which includes
  confirmed ownership;
- the current playback source is YouTube Music;
- the current item has a valid 11-character video ID;
- the same playlist/video request is not already pending.

Shared, incomplete and unconfirmed playlists remain read-only.

## Mutation boundary

The packaged helper accepts one playlist ID and one video ID, re-reads up to 500
remote occurrences to confirm ownership and editability, rejects an existing
matching item, and calls `add_playlist_items` exactly once with
`duplicates=False`. Ambiguous failures are never retried automatically.

## Reconciliation

A successful mutation response does not update native state. Nocky fetches the
playlist again from the server, confirms the requested video ID in the fresh
response, caches the refreshed items, and only then replaces the native playlist
contents. A failed or ambiguous mutation leaves the existing native playlist
unchanged.

## Validation

The automated contract tests cover request validation, remote ownership checks,
duplicate suppression, one-item submission and response sanitization. The Rust
controller tests cover video/playlist identity, while the full Quality Gate and
one reversible real-account smoke test remain required before merge.

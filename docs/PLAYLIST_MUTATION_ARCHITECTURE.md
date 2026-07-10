# Remote YouTube Music playlist mutation architecture

This document is the Phase 12D architecture gate for remote YouTube Music
playlist mutation. It records what the pinned runtime can do, what Nocky can
prove safely today, and which checkpoints must stay blocked before destructive
playlist operations are exposed.

This gate does not authorize delete, remove or reorder UI, and it does not add
remote destructive calls.

## Status atual

Nocky already has:

- authenticated YouTube Music search, Home, library, liked songs and playlist
  loading through the Python helper runtime;
- cache-first playlist restoration, Home snapshots and account-library cache
  persistence;
- Phase 12B read-only profile discovery diagnostics;
- Phase 12C reversible favorite-state handling with pending, confirmed and
  rollback states;
- read-only playlist metadata normalization for `playlist_id`, ownership,
  privacy, editability and playlist item `set_video_id`;
- a native diagnostic surface that keeps unowned playlists read-only;
- empty-playlist creation, private by default, with playlist-list cache update;
- single-track addition to confirmed-owned playlists, duplicate-submit
  protection and fresh playlist reconciliation;
- a helper-level metadata edit contract that revalidates current metadata before
  calling upstream, with native UI still gated as a separate checkpoint.

Nocky still lacks:

- `YouTubeItem` fields for ownership, privacy, editability and `set_video_id`;
- persisted freshness metadata for ownership/editability decisions;
- native remove, reorder, append-source-playlist or delete surfaces;
- queue-aware reconciliation for destructive playlist membership changes;
- a user-facing destructive confirmation framework for remote playlists.

Destructive playlist actions remain blocked because ownership cannot be inferred
from library membership, playback, title, author text or cached route identity.
Removing a track also requires the exact playlist occurrence identity:
`videoId` plus `setVideoId`. A plain `videoId` can refer to multiple occurrences
inside the same playlist and is not enough to delete safely.

## Runtime baseline

The project pins `ytmusicapi==1.12.1` in `requirements-youtube.txt`, installed by
`scripts/setup-youtube-runtime.sh`.

The pinned wheel exposes these relevant public methods:

- `create_playlist(title, description, privacy_status="PRIVATE", video_ids=None, source_playlist=None)`
- `edit_playlist(playlistId, title=None, description=None, privacyStatus=None, collaboration=None, moveItem=None, addPlaylistId=None, sortOrder=None, addToTop=None, voteOption=None)`
- `delete_playlist(playlistId)`
- `add_playlist_items(playlistId, videoIds=None, source_playlist=None, duplicates=False)`
- `remove_playlist_items(playlistId, videos)`
- `get_playlist(playlistId, limit=...)`

Nocky's packaged helper currently allows only `get_playlist`, empty
`create_playlist`, single-item `add_playlist_items` and metadata-only
`edit_playlist`. It rejects source-playlist cloning, duplicate insertion and
unsupported destructive operations before the YouTube Music client is created.

## Upstream operation matrix

| Operation | Upstream API | Required identifiers | Ownership required | Risk level | Reversible / non-destructive / destructive | Cache reconciliation needed | UI confirmation needed | Safe to expose now? |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Create empty private playlist | `create_playlist(title, description, privacy_status="PRIVATE")` | title; optional description; privacy default | No existing playlist ownership; authenticated account required | Low | Non-destructive | Add/refetch account playlist list; persist library cache; refresh visible playlists | Submit confirmation only | Yes; shipped as an empty-create checkpoint |
| Edit metadata | `edit_playlist(playlistId, title=..., description=..., privacyStatus=...)` | playlistId; current title/privacy for stale-state check; changed fields | Yes, confirmed on fresh read | Medium | Reversible when previous values are known | Refetch target playlist metadata; refresh playlist list if visible metadata changed; persist cache | Explicit save with current values visible | No native UI yet; helper contract only |
| Add tracks | `add_playlist_items(playlistId, videoIds=[...], duplicates=False)` | playlistId; one or more videoId values | Yes, confirmed on fresh read | Medium | Reversible in principle, but not an undo unless matching `setVideoId` is later available | Refetch target playlist; update opened-playlist cache; invalidate playlist metadata; refresh visible playlist | Action-specific submit confirmation; no destructive dialog | Yes for current single-track addition only |
| Remove tracks | `remove_playlist_items(playlistId, videos=[{"videoId": ..., "setVideoId": ...}])` | playlistId; videoId; setVideoId per occurrence | Yes, confirmed on fresh read | High | Destructive item mutation | Refetch target playlist; update opened-playlist cache; invalidate metadata; repair queue/offline state | Required destructive confirmation | No |
| Reorder tracks | `edit_playlist(playlistId, moveItem=...)` | playlistId; setVideoId/move identity for affected occurrence; target position identity | Yes, confirmed on fresh read | High | Reversible only if the complete previous order is fresh | Refetch target playlist; update opened-playlist cache; refresh player queue if same playlist is active | Required confirmation or explicit drag/drop commit | No |
| Delete playlist | `delete_playlist(playlistId)` | playlistId; current title; typed confirmation text | Yes, confirmed on fresh read | Critical | Destructive collection mutation | Remove/refetch account playlist list; clear opened-playlist cache; invalidate metadata; repair queue/offline state | Required typed-title destructive confirmation | No |
| Append playlist | `edit_playlist(playlistId, addPlaylistId=...)`; upstream also offers source-playlist inputs on creation/addition | target playlistId; source/addPlaylistId | Yes for target; source readability must be confirmed | High | Partially reversible only after fresh occurrence identities are known | Refetch target playlist; refresh account playlist list if metadata changes; update opened-playlist cache | Required bulk-change confirmation | No |

## Required model additions

Before any broader remote playlist action can be exposed, the application model
must distinguish:

- playlist ownership: explicit `owned == true` from an authenticated playlist
  response, never inferred;
- playlist editability: effective app-level capability derived from ownership,
  playlist ID validity and operation-specific requirements;
- privacy: `PRIVATE`, `UNLISTED`, `PUBLIC` or unknown, preserving unknown values
  as non-authoritative;
- playlistId: the normalized stable playlist identifier, with `VL` route aliases
  normalized before mutation;
- videoId: the YouTube video identity for additions and track matching;
- setVideoId: the occurrence identity required for remove and reorder;
- stable item identity: duplicate tracks must remain distinguishable by
  occurrence, not collapsed by `videoId`;
- source of truth and freshness timestamp: ownership/editability must come from
  a fresh authenticated read or a bounded-validity cache;
- optional owner/channel metadata: useful for diagnostics, but never sufficient
  to prove ownership on its own.

Current read-only structures cover part of this contract in
`YouTubePlaylistMetadata`, `YouTubePlaylistTrackMetadata` and
`playlist_mutation_contract.rs`. The general `YouTubeItem` route/playback model
does not carry enough metadata for destructive actions and must not be used as a
mutation authority.

## Safe delivery order

1. Read-only ownership/editability metadata contract.
2. Create empty private playlist.
3. Add tracks with duplicate handling and reconciliation.
4. Edit metadata only for confirmed-owned playlists.
5. Remove/reorder only when `setVideoId` is present and ownership is confirmed.
6. Delete playlist only after explicit confirmation, ownership verification and
   cache invalidation.

In this repository state, the architecture gate, read-only contract, empty
creation, single-track addition and helper-level metadata edit contract already
exist. Remove, reorder, append-source-playlist and delete remain future
checkpoints.

## Safety rules

- Never show delete, remove or reorder controls unless ownership is confirmed.
- Never remove a playlist item without `setVideoId`.
- Never assume a playlist is owned by the user because it appears in the
  library.
- Always separate non-destructive, reversible and destructive actions.
- Every destructive action requires confirmation.
- Every failure must be categorized in a privacy-safe way.
- Every successful action must reconcile cache before native state is treated as
  confirmed.
- Do not log tokens, headers, cookies, sensitive URLs or private payloads.
- Generated radio/mix aliases must stay read-only even when upstream returns a
  canonical playlist response.
- Collaborative playlist mutation is deferred until editability can be proven
  per operation.

## UI/UX requirements

- User-facing strings need Portuguese, English and Spanish parity.
- Destructive operations need explicit confirmation copy that identifies the
  remote playlist and action.
- Delete needs typed-title confirmation.
- Remove and reorder need occurrence-aware labels when duplicate tracks are
  present.
- Loading state must disable only the pending operation, not the entire YouTube
  surface when avoidable.
- Recoverable errors must explain whether the user can retry, reconnect or
  refresh metadata.
- Undo may be offered only when the previous remote state and required
  identifiers are complete enough to restore it.
- Diagnostics must be privacy-safe and avoid account identifiers, raw endpoint
  payloads, cookies, headers, tokens and sensitive URLs.

## Cache reconciliation

Playlist mutation reconciliation must update every affected native source of
truth:

- playlist cache: refetch the target playlist, replace `playlist_tracks`, persist
  opened-playlist cache and invalidate read-only metadata derived from the old
  track list;
- YouTube library cache: refetch or update the account playlist list when a
  playlist is created, deleted, renamed or its visible privacy/description state
  changes;
- Home V3/Home feed: refresh or invalidate visible Home sections when they
  display the mutated playlist or stale playlist-derived tracks;
- player queue: if the active queue came from the mutated playlist, preserve
  currently playing media when possible, remove deleted occurrences explicitly
  after reconciliation and avoid silent queue reshuffles;
- offline/local cache: do not delete local/offline files solely because a remote
  occurrence changed; mark stale remote membership and reconcile through the
  existing offline cache policies;
- search cache: stale cached search results must not be used as mutation
  authority and should be refreshed after visible playlist metadata changes.

Remote success followed by reconciliation failure is a partial-success state.
The UI must report that the server may have changed while Nocky's local view was
left unchanged or stale.

## Testing plan

- Unit tests for model mapping and version-tolerant deserialization.
- Runtime contract tests using mocked `ytmusicapi` clients only.
- No live destructive tests by default.
- Fixture tests for `setVideoId` presence, absence and duplicate occurrences.
- Ownership/editability matrix tests for owned, shared, generated and missing-ID
  playlists.
- UI gating tests proving delete/remove/reorder controls are absent without
  confirmed ownership and occurrence identity.
- Cache reconciliation tests for playlist cache, account library cache, Home
  cache, active queue and offline/local cache interactions.
- Privacy-safe diagnostics tests proving raw headers, cookies, tokens, account
  identifiers and opaque payloads do not cross helper or log boundaries.
- Failure tests for expired session, permission denial, stale metadata, network
  timeout, unexpected upstream response and reconciliation failure.

## Open questions

- What freshness window is acceptable for cached ownership/editability metadata?
- Should metadata edit UI ship before any membership mutation beyond add-current?
- How should duplicate track occurrences be presented when title/artist metadata
  is identical?
- What is the exact undo policy for add, metadata edit and reorder?
- Should reorder be implemented as explicit save after local drag/drop rather
  than immediate remote mutation?
- How should active queue repair behave when a removed occurrence is currently
  playing?
- Should deleted remote playlists keep local/offline remnants as detached local
  history or be hidden from playlist views immediately after confirmation?
- Can collaborative playlists expose a safe subset of operations, or should they
  remain read-only until upstream returns explicit per-operation capabilities?
- How should profile selection interact with playlist mutation if multiple Brand
  Account identities become deterministic later?

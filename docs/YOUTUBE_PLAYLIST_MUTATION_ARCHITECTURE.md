# Remote YouTube Music playlist mutation architecture

This document defines the Phase 12D review boundary for remote playlist
operations. It authorizes only model and validation work. It does not authorize
network mutations, UI controls, or persistence changes.

## Runtime baseline

Nocky currently pins `ytmusicapi==1.12.1`.

The pinned public API exposes:

| Operation | Upstream method | Initial Nocky classification |
| --- | --- | --- |
| Read playlist and ownership metadata | `get_playlist` | Read-only |
| Create playlist | `create_playlist` | Non-destructive remote mutation |
| Rename, edit description or privacy | `edit_playlist` | Reversible remote mutation |
| Reorder items | `edit_playlist(moveItem=...)` | Reversible remote mutation |
| Append another playlist | `edit_playlist(addPlaylistId=...)` | Reversible remote mutation |
| Add tracks | `add_playlist_items` | Reversible remote mutation |
| Remove tracks | `remove_playlist_items` | Destructive item mutation |
| Delete playlist | `delete_playlist` | Destructive collection mutation |

`create_playlist` rejects titles containing `<` or `>`. The Nocky contract
blocks those values before any request is sent.

`remove_playlist_items` requires both `videoId` and `setVideoId`. The latter is
the unique identity of a track occurrence inside a playlist. A plain YouTube
video ID is insufficient because the same track can appear more than once.

## Current Nocky model gaps

The current `YouTubeItem` model carries playback and routing data but does not
carry:

- playlist ownership;
- playlist privacy;
- stable playlist-item `setVideoId` values;
- collaborative editability;
- remote mutation capabilities;
- a mutation revision or reconciliation state.

Therefore, the application cannot safely expose remove, reorder, rename or
delete controls yet.

## Ownership policy

The first implementation must require `owned == true` for every operation on an
existing playlist.

Collaborative playlist editing is intentionally deferred. Being able to view a
collaborative playlist does not prove that every edit operation is allowed.

The application must not infer ownership from:

- playlist title;
- author display name;
- presence in the user's library;
- browse ID prefix;
- successful playback;
- cached metadata alone.

Ownership must come from a fresh authenticated playlist response or a cache that
was produced from the same explicit field and is still within its validity
window.

## Risk classes

### Non-destructive

- Create an empty private playlist.

The default privacy must be `PRIVATE`. Public or unlisted creation requires an
explicit user choice in the creation dialog.

### Reversible

- Add tracks.
- Rename a playlist.
- Edit description.
- Change privacy.
- Reorder tracks.
- Append another playlist.

These operations still require remote reconciliation. The UI may use optimistic
feedback only when the previous state is known and can be restored locally.

### Destructive

- Remove tracks.
- Delete a playlist.

Track removal requires ownership plus complete `videoId` and `setVideoId`
identity. Playlist deletion requires a separate checkpoint and exact typed-title
confirmation. Neither operation may use silent optimistic disappearance.

## Safety contract

The pure Rust contract in `playlist_mutation_contract.rs` blocks requests when:

- an existing playlist ID is missing;
- ownership is not confirmed;
- a title is empty or contains unsupported characters;
- an add/remove request contains no video ID;
- duplicate video IDs are present in one operation;
- removal lacks `setVideoId`;
- a metadata edit contains no actual change;
- deletion confirmation does not exactly match the current playlist title.

Passing validation means only that the operation is eligible for a later
implementation. It does not execute or authorize a network request.

## Reconciliation requirements

Every successful remote mutation must be followed by:

1. refetching the target playlist;
2. refetching the account playlist list when collection membership may change;
3. updating the native in-memory model;
4. atomically updating the cache;
5. comparing the confirmed remote state with the requested state;
6. showing explicit partial-success feedback if the mutation succeeded but
   reconciliation failed.

A failed request must never leave an optimistic state as if it were confirmed.

## Error categories

User-facing failures must distinguish:

- expired or disconnected session;
- ownership or permission denial;
- missing playlist-item identity;
- duplicate track rejection;
- unavailable or removed video;
- network or timeout failure;
- remote success followed by reconciliation failure;
- unexpected response contract.

Logs must not include cookies, authorization values, raw headers, account
identifiers, or complete endpoint payloads.

## Localization and accessibility

All controls and feedback require Portuguese, English and Spanish parity.
Confirmation dialogs must:

- identify the remote playlist clearly;
- describe whether the action is reversible;
- keep destructive actions visually separated from routine edits;
- support keyboard-only operation;
- remain usable at the minimum supported window width.

## Delivery order

### Slice 12D1 — read-only editability metadata

- Extend the playlist-detail contract with ownership, privacy and stable item
  identity.
- Keep all mutation controls hidden.
- Add fixture-based parser and Rust compatibility tests.

### Slice 12D2 — create empty private playlist

- Add a native creation dialog.
- Default to private.
- Reconcile the account playlist list after success.
- No initial track batch in the first slice.

### Slice 12D3 — add tracks

- Add one or more unique video IDs.
- Keep duplicates disabled by default.
- Reconcile the target playlist after success.

### Slice 12D4 — metadata edits

- Rename, description and privacy changes.
- Display current values before submission.
- Keep rollback data until reconciliation completes.

### Slice 12D5 — remove tracks

- Require ownership and `setVideoId`.
- Require an explicit confirmation dialog.
- Reconcile the exact target playlist.

### Slice 12D6 — delete playlist

- Separate explicit approval checkpoint.
- Exact typed-title confirmation.
- No optimistic removal.
- Refetch the account playlist list after success.

## Explicitly deferred

- collaborative playlist mutation;
- bulk destructive actions;
- playlist deletion in the architecture PR;
- cross-account or profile-specific mutation routing;
- persisting broader authentication material;
- Local Home or local-playlist changes.

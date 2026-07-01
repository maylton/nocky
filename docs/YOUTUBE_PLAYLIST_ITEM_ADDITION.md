# YouTube Music playlist item addition foundation

This document defines Phase 12D3's single-track addition contract and native GTK
exposure. The user-facing action is available only on an authenticated YouTube
playlist page after the read-only metadata contract confirms that the connected
account owns and can edit that playlist.

## Supported request

The first checkpoint accepts exactly:

- one caller-confirmed playlist ID;
- one caller-confirmed editable state from the read-only metadata contract;
- one 11-character YouTube video ID;
- duplicate insertion disabled.

An optional `VL` browse prefix is removed before the request is sent. URLs,
source-playlist cloning, batches and explicit duplicate insertion are rejected
before session or client access.

Caller-provided ownership is only the first gate. After authentication, the
helper fetches the same playlist through the read-only metadata contract and
requires the returned playlist ID, `owned` state and effective editability to
match before any remote change can be attempted.

## Native exposure

The playlist header shows an "add current track" action only when all of these
conditions are true:

- the current route is a YouTube playlist;
- read-only playlist metadata has confirmed `owned == true` and
  `editable == true`;
- playback is currently sourced from YouTube Music;
- the current item has a valid 11-character YouTube video ID;
- the same playlist/video addition is not already pending.

Clicking the action disables that specific playlist/video request while a worker
thread performs the remote mutation. The native model is not changed from the
initial mutation response.

## Runtime call

After the authenticated ownership check, a valid request maps to the pinned
runtime method with:

- the normalized playlist ID;
- one `videoIds` entry;
- `duplicates=False`.

The helper calls the mutation method once. It does not retry automatically
because an ambiguous network failure could otherwise insert the same item twice.

## Sanitized result

The helper returns only:

```json
{
  "playlist_id": "PL...",
  "video_id": "abcdefghijk",
  "added_count": 1,
  "reconciliation_required": true
}
```

Raw responses, edit tokens and newly returned `setVideoId` values do not cross
the helper boundary.

## Reconciliation rule

A successful mutation response is not sufficient to finalize native state. The
native controller fetches the playlist again through the read-only metadata
contract and confirms the new occurrence before updating in-memory state or the
cache. The refreshed response becomes the source of truth for ownership,
privacy, duplicate occurrences and `setVideoId` identity.

If the fresh read cannot confirm the item, Nocky leaves the native playlist
unchanged and reports an ambiguous reconciliation failure instead of presenting
an optimistic success.

## Failure behavior

- Invalid caller ownership, editability or identifiers fail before authentication.
- Missing authentication fails before client creation.
- Missing or mismatched remote ownership metadata blocks the mutation.
- Unsupported runtimes fail without attempting another method.
- Only `STATUS_SUCCEEDED` is accepted as confirmation.
- Unexpected or partial responses become a generic sanitized error.

## Out of scope

- automatic real-account smoke tests;
- batch addition;
- duplicate insertion;
- source-playlist append;
- removal, reordering, rename, privacy changes or deletion;
- profile or authentication changes;
- Local Home and local-library changes.

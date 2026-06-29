# YouTube Music playlist item addition foundation

This document defines Phase 12D3's first network-isolated contract. It does not
expose a GTK action and it is not installed as a user-facing capability yet.

## Supported request

The first checkpoint accepts exactly:

- one confirmed-owned playlist ID;
- one confirmed-editable state from the read-only metadata contract;
- one 11-character YouTube video ID;
- duplicate insertion disabled.

An optional `VL` browse prefix is removed before the request is sent. URLs,
source-playlist cloning, batches and explicit duplicate insertion are rejected
before session or client access.

## Runtime call

A valid request maps to the pinned runtime method with:

- the normalized playlist ID;
- one `videoIds` entry;
- `duplicates=False`.

The helper calls the method once. It does not retry automatically because an
ambiguous network failure could otherwise insert the same item twice.

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
caller must fetch the playlist again through the read-only metadata contract and
confirm the new occurrence. The refreshed response becomes the source of truth
for ownership, privacy, duplicate occurrences and `setVideoId` identity.

## Failure behavior

- Invalid ownership, editability or identifiers fail before authentication.
- Missing authentication fails before client creation.
- Unsupported runtimes fail without attempting another method.
- Only `STATUS_SUCCEEDED` is accepted as confirmation.
- Unexpected or partial responses become a generic sanitized error.

## Out of scope

- GTK controls;
- automatic real-account smoke tests;
- batch addition;
- duplicate insertion;
- source-playlist append;
- removal, reordering, rename, privacy changes or deletion;
- profile or authentication changes;
- Local Home and local-library changes.

Native exposure remains blocked until the playlist metadata integration tracked
by issue #71 is complete.

# YouTube Music playlist metadata edit foundation

This document defines Phase 12D4's first reversible metadata-edit contract. It
does not expose a GTK action yet; it prepares the installed helper and native
bridge for a later dialog that can show the current values before submission.

## Supported request

The first checkpoint accepts exactly:

- one caller-confirmed playlist ID;
- caller-confirmed ownership and editability from the read-only metadata
  contract;
- the current title, description and privacy shown to the user;
- at least one changed title, description or privacy value.

An optional `VL` browse prefix is removed before the request is sent. Empty
titles, unsupported title characters, unsupported privacy values and no-op edits
are rejected before session or client access.

## Runtime call

After authentication, the helper fetches the same playlist and requires
ownership and editability to still be true. If the submitted current title or
privacy no longer matches the fresh playlist metadata, the edit is blocked as a
stale request.

A valid request maps to the pinned `ytmusicapi 1.12.1` method with:

- the normalized playlist ID;
- `title` only when the title changed;
- `description` only when the description changed;
- `privacyStatus` only when privacy changed.

Collaboration, reordering, source-playlist append, removal and deletion are not
called by this checkpoint.

## Sanitized result

The helper returns only the requested changed fields:

```json
{
  "playlist_id": "PL...",
  "title": "Deep Focus",
  "privacy": "UNLISTED",
  "reconciliation_required": true
}
```

Raw responses, collaboration tokens and endpoint payloads do not cross the
helper boundary.

## Reconciliation rule

The returned success contract is not enough to finalize native state. The future
GTK slice must refetch the playlist, confirm the edited values, refresh the
playlist list when title or privacy changes affect visible library metadata, and
only then update the native cache.

## Out of scope

- GTK controls;
- automatic real-account smoke tests;
- collaborative settings;
- track reordering;
- source-playlist append;
- track removal or playlist deletion;
- profile or authentication changes;
- Local Home and local-library changes.

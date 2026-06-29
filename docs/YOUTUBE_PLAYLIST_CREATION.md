# YouTube Music playlist creation

This document defines Phase 12D's first non-destructive playlist mutation.

## Supported operation

The initial checkpoint creates one empty playlist. It accepts:

- a required non-empty title;
- an optional plain-text description;
- one privacy value: `PRIVATE`, `UNLISTED`, or `PUBLIC`.

Privacy defaults to `PRIVATE`. Titles containing `<` or `>` are rejected before
network access because the pinned `ytmusicapi 1.12.1` contract documents those
characters as unsupported.

The first checkpoint rejects initial track lists and source-playlist cloning.
Those capabilities will be reviewed separately after empty-playlist creation is
validated against a real account.

## Native boundary

The helper returns only:

```json
{
  "playlist_id": "PL...",
  "title": "Playlist title",
  "privacy": "PRIVATE"
}
```

Raw service responses, request context, headers, cookies, account information,
and internal edit metadata never cross the helper boundary.

## Failure behavior

- Invalid title, privacy, or non-empty creation requests fail before session or
  client access.
- A disconnected session fails before the YouTube client is created.
- A response without a valid playlist identifier becomes a generic sanitized
  error; the raw response is not returned or logged.

## Out of scope

- playlist deletion;
- track removal;
- playlist rename;
- batch creation;
- collaborative playlist settings;
- profile switching;
- Local Home and local-library changes.

Destructive playlist operations require a separate design and security review.

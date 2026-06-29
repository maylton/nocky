# YouTube Music account profile discovery

This document defines the read-only discovery boundary for Phase 12, Slice 12B.
It does not authorize account switching, new authentication storage, or remote
library mutations.

## Upstream behavior

The current `ytmusicapi` public contract exposes two separate capabilities:

1. `get_account_info()` calls `account/account_menu` and parses only the active
   account header. It returns the active account name, channel handle and photo.
2. `YTMusic(..., user=<id>)` places the supplied value in
   `context.user.onBehalfOfUser`, allowing requests for a known Brand Account.

The public library does not currently provide a method that discovers every
channel attached to the authenticated Google account.

The internal `account/accounts_list` endpoint and its renderer structure have
been observed by independent YouTube Music clients, but are not part of the
stable `ytmusicapi` public API. Nocky must therefore treat discovery as an
experimental, read-only capability with strict fallback behavior.

## Sanitized contract

The parser accepts a raw account-list response and returns:

```json
{
  "state": "unavailable | single | multiple | ambiguous",
  "deterministic": true,
  "profiles": [
    {
      "profile_id": "primary or numeric Brand Account id",
      "name": "Display name",
      "channel_handle": "@handle",
      "photo_url": "https://...",
      "kind": "primary | brand | unknown",
      "is_selected": true,
      "switchable": true
    }
  ]
}
```

Only explicit `pageIdToken.pageId` values consisting of 10–30 decimal digits are
accepted as Brand Account identifiers. The single profile without such an ID is
classified as the primary profile. Multiple profiles without stable IDs make the
result ambiguous and non-switchable.

## Data that must never leave the parser

- cookies or authorization values;
- raw request headers;
- Google account email addresses;
- sign-in URLs;
- click-tracking parameters;
- visitor data;
- continuation tokens;
- complete service endpoints or opaque payloads.

Photo URLs must use HTTPS. Profiles without a display name are ignored.
Duplicate Brand Account IDs are collapsed.

## State semantics

- `unavailable`: no recognized profile renderer was found;
- `single`: exactly one deterministic profile was found;
- `multiple`: multiple profiles have unique stable IDs and exactly one is active;
- `ambiguous`: IDs or selected-state information are insufficient for safe use.

The application must keep the already validated active-profile presentation for
`unavailable` and `ambiguous` results.

## Delivery sequence

1. Pure parser and fixture-based tests.
2. Read-only helper call to `account/accounts_list` with sanitized output only.
3. Real-account diagnostic that reports counts, state and display labels without
   printing authentication material.
4. Review of response stability across primary and Brand Account configurations.
5. Only after deterministic validation: a separate selection design and security
   review.

## Explicitly out of scope

- switching the active profile;
- persisting a selected Brand Account ID;
- adding `X-Goog-PageId` to the saved authentication contract;
- changing `X-Goog-AuthUser` automatically;
- opening or following sign-in URLs returned by account-list responses;
- remote likes or playlist mutations.

## Primary references

- `ytmusicapi/ytmusicapi/mixins/library.py`: `get_account_info()` parses
  `activeAccountHeaderRenderer` only.
- `ytmusicapi/ytmusicapi/ytmusic.py`: the `user` constructor parameter maps to
  `context.user.onBehalfOfUser`.
- `ytmusicapi` issue #177: maintainer guidance for using a known Brand Account ID
  through `YTMusic(user=...)`.
- `ytmusicapi` issue #283: `X-Goog-PageId` is not established as a universally
  required selector and must not be added to Nocky's persisted contract without
  new evidence and review.

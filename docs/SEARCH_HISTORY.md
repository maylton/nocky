# Recent search history

## Scope

Nocky keeps a small local MRU list of completed search text. It is shared by
local-library and YouTube Music modes because it stores only the text entered by
the user, not account identifiers, remote result metadata or continuation
tokens.

## Behavior

- a stable query is recorded after 800 ms;
- pressing Enter records it immediately;
- selecting a recent query moves it to the front;
- matching is case-insensitive and whitespace-normalized;
- single-character fragments are ignored;
- at most 20 entries are retained;
- entries can be removed individually or cleared together;
- focusing the empty global search field opens the recent-query dropdown;
- typing non-empty text closes the dropdown and keeps the existing live search
  behavior.

## Storage

The list is stored atomically in the user data directory as
`nocky/search-history.json`. Corrupt or incompatible files are ignored. Search
cache entries remain session-scoped and separate from this file.

## Deferred

Mixed local/remote ranking, route-aware cancellation and result-update
announcements remain later search checkpoints.

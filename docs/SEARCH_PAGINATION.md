# Real remote search pagination

## Scope

Nocky now requests one real YouTube Music search page per category and keeps the
opaque continuation returned by the remote service. Songs, albums, artists and
playlists paginate independently.

## Request flow

1. The initial categorized search requests the first remote page for each
   category in parallel.
2. The helper parses the initial `musicShelfRenderer` and returns both its items
   and the next continuation.
3. After all fetched items in a category are visible, **Load more** sends only
   that category's continuation.
4. The response is appended with stable identity deduplication and its next
   continuation replaces the previous token.
5. An empty continuation removes the remote load-more control for that category.

The helper supports both the classic `continuationContents` response and the
newer append-action response shape. Continuations remain opaque outside the
helper.

## Safety and state

- one pagination request per category can be active at a time;
- changing the query invalidates late page responses through the existing
  request generation;
- a pagination failure preserves all previously displayed results;
- successful pages update the expiring query cache without mixing synchronized
  local-library matches into the remote snapshot;
- account transitions continue to clear the complete query cache.

## Deferred

Search history, recent queries, mixed local/remote ranking, route-aware request
cancellation and optional accessibility announcements remain separate
checkpoints.

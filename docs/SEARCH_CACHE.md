# Expiring YouTube Search Cache

## Scope

This checkpoint adds a bounded, session-scoped cache for categorized YouTube
Music search results. It deliberately does not persist account search history to
disk.

## Policy

- cache key: normalized query text;
- fresh lifetime: 10 minutes;
- stale-while-revalidate window: up to 60 minutes;
- capacity: 32 queries;
- eviction: least recently used entry;
- account disconnect or reconnect: immediate cache clear.

## Behavior

A fresh hit paints immediately and skips the remote request. A stale hit paints
immediately with the existing searching banner while the four remote categories
are refreshed in parallel. Entries older than the stale window are discarded.

Only remote results are stored. Current synchronized library matches are merged
when a cache entry is displayed, preventing removed local or synchronized items
from becoming permanently embedded in a query snapshot.

Request generation remains authoritative: using a fresh cache entry increments
the request ID and invalidates older in-flight responses, so a late response can
never replace the active query.

## Deferred

True remote pagination and continuation tokens remain the next search
checkpoint. The current cache stores the initial categorized batches and is
structured so paginated batches can update the same query entry later.

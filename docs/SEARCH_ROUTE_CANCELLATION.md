# Route-aware YouTube search cancellation

## Scope

Global YouTube Music searches are now tied to the visible global-search route.
Remote work remains useful for the search screen, but route changes should not
allow late responses to repaint or keep loading state alive after the user has
moved elsewhere.

## Behavior

- the initial remote search only starts while the browser route is `All` and the
  global search query is non-empty;
- continuation pagination uses the same route gate;
- leaving the global search route increments the search generation, clears
  transient loading flags and makes in-flight responses stale;
- returning to the global search route with the same non-empty query starts a new
  request generation;
- background global-search and page-loaded messages are ignored unless the route,
  query, source and generation still match;
- cached results remain query-scoped and are not cleared by route changes.

## Visual follow-up

The recent-search dropdown keeps the exact width alignment from the inline
surface checkpoint while restoring a tonal card background, visible outline and a
subtle elevation shadow.

## Deferred

Result-update announcements and deeper remote worker cancellation remain future
accessibility/performance polish. The current checkpoint prevents stale UI state
and stale response application without trying to interrupt Python helper calls
already running in worker threads.

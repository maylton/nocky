# Search result accessibility announcements

## Scope

The categorized search page now updates its accessible label every time the
search result tree is rebuilt. This gives assistive technologies one compact
summary of the current query, loading state and category counts without adding
extra visible chrome to the interface.

## Announcement contract

For each rebuild, Nocky summarizes:

- the active query;
- whether YouTube Music is still updating results;
- total result count;
- visible category totals for tracks, albums, artists and playlists.

The message is localized in Portuguese, English and Spanish and is exposed
through GTK's accessible property API on the search results container.

## Behavior

- Local-only search announces local counts only.
- YouTube Music search announces synchronized/remote counts for the active
  source without mixing in the local library.
- Loading state changes update the announcement even when the visible result
  counts are unchanged.
- Existing keyboard row activation and source-specific actions are preserved.

## Deferred

Future polish can use toolkit-level live-region APIs when the project moves to a
GTK/gtk-rs version that exposes reliable cross-screen-reader announcement
priorities. This checkpoint keeps the implementation dependency-free and stable
with the current GTK bindings.

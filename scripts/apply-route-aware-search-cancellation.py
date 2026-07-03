#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path.cwd()
CONSTRUCTION = ROOT / "src/app/controller/construction.rs"
NAVIGATION = ROOT / "src/app/controller/navigation.rs"
YOUTUBE_CONTROLLER = ROOT / "src/app/controller/youtube.rs"
BACKGROUND_CONTROLLER = ROOT / "src/app/controller/background.rs"
SEARCH_STYLE = ROOT / "assets/themes/material-expressive/102-search-history.css"
ROADMAP = ROOT / "ROADMAP.md"
DOC = ROOT / "docs/SEARCH_ROUTE_CANCELLATION.md"


class PatchError(RuntimeError):
    pass


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count == 0 and new in text:
        print(f"[already applied] {label}")
        return text
    if count != 1:
        raise PatchError(f"{label}: expected one match, found {count}")
    print(f"[changed] {label}")
    return text.replace(old, new, 1)


DOC_SOURCE = r'''# Route-aware YouTube search cancellation

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
'''


def patch_construction(text: str) -> str:
    return replace_once(
        text,
        '''                let source = glib::timeout_add_local_once(Duration::from_millis(350), move || {
                    delayed_pending.borrow_mut().take();
                    if let Some(controller) = delayed_controller.upgrade() {
                        controller.request_global_youtube_search(query);
                    }
                });
''',
        '''                let source = glib::timeout_add_local_once(Duration::from_millis(350), move || {
                    delayed_pending.borrow_mut().take();
                    if let Some(controller) = delayed_controller.upgrade() {
                        controller.request_global_youtube_search(query);
                    }
                });
''',
        "Keep debounced search request routed through guarded controller",
    )


ROUTE_METHODS = r'''    fn global_youtube_search_visible(&self, query: &str) -> bool {
        self.config.borrow().startup_source == Some(StartupSource::YouTube)
            && matches!(self.browser.route(), BrowserRoute::All)
            && !query.trim().is_empty()
            && self.search_query.borrow().trim() == query.trim()
    }

    fn cancel_global_youtube_search_for_route_change(&self) {
        self.youtube_search_request_id
            .set(self.youtube_search_request_id.get().wrapping_add(1));
        let mut library = self.youtube_library.borrow_mut();
        library.search.clear_transient_state();
    }

    fn restart_global_youtube_search_after_route_return(&self) {
        let query = self.search_query.borrow().trim().to_string();
        if self.global_youtube_search_visible(&query) {
            self.request_global_youtube_search(query);
        }
    }

'''


def patch_navigation(text: str) -> str:
    text = replace_once(
        text,
        "impl AppController {\n",
        "impl AppController {\n" + ROUTE_METHODS,
        "Add route-aware search helpers",
    )
    text = replace_once(
        text,
        '''        if previous_route != route {
            self.browser.reset_queue_scroll_position();
        }
''',
        '''        if previous_route != route {
            self.browser.reset_queue_scroll_position();
            if !matches!(route, BrowserRoute::All) {
                self.cancel_global_youtube_search_for_route_change();
            } else {
                self.restart_global_youtube_search_after_route_return();
            }
        }
''',
        "Invalidate remote search generation on route transitions",
    )
    return text


def patch_youtube_controller(text: str) -> str:
    text = replace_once(
        text,
        '''        if query.is_empty()
            || self.config.borrow().startup_source != Some(StartupSource::YouTube)
            || self.search_query.borrow().trim() != query.as_str()
        {
            return;
        }
''',
        '''        if !self.global_youtube_search_visible(&query) {
            return;
        }
''',
        "Gate initial remote search by visible route",
    )
    text = replace_once(
        text,
        '''        if query.is_empty() || self.config.borrow().startup_source != Some(StartupSource::YouTube) {
            return;
        }
''',
        '''        if !self.global_youtube_search_visible(&query) {
            return;
        }
''',
        "Gate paginated remote search by visible route",
    )
    return text


def patch_background_controller(text: str) -> str:
    text = replace_once(
        text,
        '''                    if request_id != self.youtube_search_request_id.get()
                        || self.search_query.borrow().trim() != query.as_str()
                        || self.config.borrow().startup_source != Some(StartupSource::YouTube)
                    {
                        continue;
                    }
''',
        '''                    if request_id != self.youtube_search_request_id.get()
                        || !self.global_youtube_search_visible(&query)
                    {
                        continue;
                    }
''',
        "Ignore stale initial search responses after route changes",
    )
    text = replace_once(
        text,
        '''                    if request_id != self.youtube_search_request_id.get()
                        || self.search_query.borrow().trim() != query.as_str()
                        || self.config.borrow().startup_source != Some(StartupSource::YouTube)
                    {
                        continue;
                    }
''',
        '''                    if request_id != self.youtube_search_request_id.get()
                        || !self.global_youtube_search_visible(&query)
                    {
                        continue;
                    }
''',
        "Ignore stale paginated search responses after route changes",
    )
    return text


def patch_search_style(text: str) -> str:
    text = replace_once(
        text,
        '''  background-color: alpha(@m3_surface_container_high, 0.94);
  border: 1px solid alpha(@m3_outline_variant, 0.62);
  box-shadow: none;
''',
        '''  background-color: @m3_surface_container_high;
  border: 1px solid alpha(@m3_outline, 0.34);
  box-shadow:
    0 8px 22px alpha(black, 0.18),
    inset 0 0 0 1px alpha(@m3_primary, 0.04);
''',
        "Restore recent-search card surface",
    )
    return text


def patch_roadmap(text: str) -> str:
    active_candidates = [
        "- 🟡 Cancellation of unnecessary remote requests after route changes.\n",
        "- 🟡 Better ranking across mixed local and remote results.\n",
        "- 🟡 Route-aware remote search cancellation.\n",
    ]
    active_matches = [candidate for candidate in active_candidates if candidate in text]
    if len(active_matches) != 1:
        raise PatchError(
            f"Advance active search checkpoint: expected one active marker, found {len(active_matches)}"
        )
    text = text.replace(
        active_matches[0],
        "- 🟡 Search result update announcements and accessibility polish.\n",
        1,
    )
    print("[changed] Advance active search checkpoint")

    anchors = [
        "- ✅ Local recent-query history with MRU ordering, individual removal and clear-all controls.\n",
        "- ✅ Relevance-ranked mixed local and remote results while YouTube Music is active.\n",
        "- ✅ Real per-category remote pagination backed by opaque YouTube Music continuations.\n",
    ]
    anchor = next((candidate for candidate in anchors if candidate in text), None)
    if anchor is None:
        raise PatchError("Document completed route-aware cancellation: anchor not found")
    if "- ✅ Route-aware cancellation for stale YouTube Music search responses.\n" not in text:
        text = text.replace(
            anchor,
            anchor + "- ✅ Route-aware cancellation for stale YouTube Music search responses.\n",
            1,
        )
        print("[changed] Document completed route-aware cancellation")
    else:
        print("[already applied] Document completed route-aware cancellation")

    for completed in [
        "- Cancellation of unnecessary remote requests after route changes.\n",
        "- Route-aware remote search cancellation.\n",
    ]:
        if completed in text:
            text = text.replace(completed, "", 1)
            print("[changed] Remove completed route cancellation item")
            break

    order_candidates = [
        "8. Add route-aware remote search cancellation.\n",
        "8. Improve mixed-source ranking and route-aware cancellation.\n",
    ]
    order_matches = [candidate for candidate in order_candidates if candidate in text]
    if order_matches:
        text = text.replace(
            order_matches[0],
            "8. Add search result announcements and release polish.\n",
            1,
        )
        print("[changed] Advance recommended search order")
    else:
        print("[already applied] Advance recommended search order")
    return text


def main() -> int:
    required = [NAVIGATION, YOUTUBE_CONTROLLER, BACKGROUND_CONTROLLER, SEARCH_STYLE, ROADMAP]
    missing = [path for path in required if not path.is_file()]
    if missing:
        print("Run this script from the Nocky repository root after applying the search-history checkpoint.", file=sys.stderr)
        for path in missing:
            print(f"missing: {path}", file=sys.stderr)
        return 1

    original = {path: path.read_text(encoding="utf-8") for path in required}
    construction = CONSTRUCTION.read_text(encoding="utf-8") if CONSTRUCTION.is_file() else ""
    if "search_history_revealer" not in construction:
        print("ERROR: apply and validate the recent-search dropdown checkpoint first.", file=sys.stderr)
        return 1
    if "YouTubeSearchPageLoaded" not in original[BACKGROUND_CONTROLLER]:
        print("ERROR: apply and validate real remote search pagination first.", file=sys.stderr)
        return 1
    if "clear_transient_state" not in (ROOT / "src/youtube/mod.rs").read_text(encoding="utf-8"):
        print("ERROR: apply and validate the expiring search cache checkpoint first.", file=sys.stderr)
        return 1

    if DOC.exists() and DOC.read_text(encoding="utf-8") != DOC_SOURCE:
        print(f"ERROR: {DOC} already exists with different content.", file=sys.stderr)
        print("No files were written.", file=sys.stderr)
        return 1

    updated = dict(original)
    try:
        updated[NAVIGATION] = patch_navigation(updated[NAVIGATION])
        updated[YOUTUBE_CONTROLLER] = patch_youtube_controller(updated[YOUTUBE_CONTROLLER])
        updated[BACKGROUND_CONTROLLER] = patch_background_controller(updated[BACKGROUND_CONTROLLER])
        updated[SEARCH_STYLE] = patch_search_style(updated[SEARCH_STYLE])
        updated[ROADMAP] = patch_roadmap(updated[ROADMAP])
    except PatchError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        print("No files were written.", file=sys.stderr)
        return 1

    changed: list[Path] = []
    for path in required:
        if updated[path] != original[path]:
            path.write_text(updated[path], encoding="utf-8")
            changed.append(path.relative_to(ROOT))

    if not DOC.exists():
        DOC.write_text(DOC_SOURCE, encoding="utf-8")
        changed.append(DOC.relative_to(ROOT))

    print("Route-aware YouTube search cancellation patch applied successfully.")
    for path in changed:
        print(f"  {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path

SOURCE = Path(__file__).with_name("apply-route-aware-search-cancellation.py")
namespace = {
    "__name__": "apply_route_aware_search_cancellation",
    "__file__": str(SOURCE),
}
exec(compile(SOURCE.read_text(encoding="utf-8"), str(SOURCE), "exec"), namespace)

PatchError = namespace["PatchError"]
replace_once = namespace["replace_once"]


def patch_background_controller_v3(text: str) -> str:
    old = '''                    if request_id != self.youtube_search_request_id.get()
                        || self.search_query.borrow().trim() != query.as_str()
                        || self.config.borrow().startup_source != Some(StartupSource::YouTube)
                    {
                        continue;
                    }
'''
    new = '''                    if request_id != self.youtube_search_request_id.get()
                        || !self.global_youtube_search_visible(&query)
                    {
                        continue;
                    }
'''
    count = text.count(old)
    if count == 0 and text.count(new) >= 2:
        print("[already applied] Ignore stale search responses after route changes")
        return text
    if count != 2:
        raise PatchError(
            f"Ignore stale search responses after route changes: expected 2 matches, found {count}"
        )
    print("[changed] Ignore stale initial search responses after route changes")
    print("[changed] Ignore stale paginated search responses after route changes")
    return text.replace(old, new, 2)


def patch_roadmap_v3(text: str) -> str:
    active_candidates = [
        "- 🟡 Better ranking across mixed local and remote results.\n",
        "- 🟡 Cancellation of unnecessary remote requests after route changes.\n",
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
        "- ✅ Real per-category remote pagination backed by opaque YouTube Music continuations.\n",
        "- ✅ Expiring search cache with a 10-minute fresh TTL, one-hour stale-while-revalidate window and bounded LRU eviction.\n",
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

    for removed in [
        "- Better ranking across mixed local and remote results.\n",
        "- Cancellation of unnecessary remote requests after route changes.\n",
        "- Route-aware remote search cancellation.\n",
    ]:
        if removed in text:
            text = text.replace(removed, "", 1)
            print(f"[changed] Remove roadmap item: {removed.strip()[2:]}")

    order_candidates = [
        "8. Improve mixed-source ranking and route-aware cancellation.\n",
        "8. Add search history, mixed-source ranking and route-aware cancellation.\n",
        "8. Add route-aware remote search cancellation.\n",
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


namespace["patch_background_controller"] = patch_background_controller_v3
namespace["patch_roadmap"] = patch_roadmap_v3
raise SystemExit(namespace["main"]())

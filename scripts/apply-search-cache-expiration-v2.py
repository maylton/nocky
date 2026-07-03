#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path

SOURCE = Path(__file__).with_name("apply-search-cache-expiration.py")
namespace = {
    "__name__": "apply_search_cache_expiration",
    "__file__": str(SOURCE),
}
exec(compile(SOURCE.read_text(encoding="utf-8"), str(SOURCE), "exec"), namespace)

replace_once = namespace["replace_once"]
replace_between = namespace["replace_between"]
global_search_handler = namespace["GLOBAL_SEARCH_HANDLER"]


def patch_background_v2(text: str) -> str:
    text = replace_once(
        text,
        """                        } else {
                            self.youtube_library.borrow_mut().clear();
                            clear_library_cache();
""",
        """                        } else {
                            self.youtube_search_cache.borrow_mut().clear();
                            self.youtube_library.borrow_mut().clear();
                            clear_library_cache();
""",
        "Clear search cache when status becomes disconnected",
    )
    text = replace_once(
        text,
        """                        self.youtube_page
                            .set_loading(false, "YouTube Music connected");
                        {
""",
        """                        self.youtube_page
                            .set_loading(false, "YouTube Music connected");
                        self.youtube_search_cache.borrow_mut().clear();
                        {
""",
        "Clear search cache on account reconnect",
    )
    text = replace_once(
        text,
        """                        self.youtube_page
                            .show_empty("Search for music or connect your account.");
                        self.youtube_library.borrow_mut().clear();
                        clear_library_cache();
""",
        """                        self.youtube_page
                            .show_empty("Search for music or connect your account.");
                        self.youtube_search_cache.borrow_mut().clear();
                        self.youtube_library.borrow_mut().clear();
                        clear_library_cache();
""",
        "Clear search cache after explicit disconnect",
    )
    return replace_between(
        text,
        "                BackgroundMessage::YouTubeGlobalSearch {\n",
        "                BackgroundMessage::YouTubeItems { title, result } => match result {\n",
        global_search_handler,
        "Store successful remote search responses",
    )


namespace["patch_background"] = patch_background_v2
raise SystemExit(namespace["main"]())

#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path

SOURCE = Path(__file__).with_name("apply-search-keyboard-a11y-polish.py")
namespace = {
    "__name__": "apply_search_keyboard_a11y_polish",
    "__file__": str(SOURCE),
}
exec(compile(SOURCE.read_text(encoding="utf-8"), str(SOURCE), "exec"), namespace)

namespace["TESTS"] = r'''
#[cfg(test)]
mod search_keyboard_a11y_polish_tests {
    use super::*;

    #[test]
    fn row_accessible_label_mentions_source_and_quick_action_in_portuguese() {
        let label = search_result_row_accessible_label(
            AppLanguage::Portuguese,
            "Absolution",
            "Muse",
            "Álbum • YouTube Music",
            true,
            true,
        );
        assert!(label.contains("Absolution"));
        assert!(label.contains("Fonte: YouTube Music"));
        assert!(label.contains("Ação rápida disponível"));
    }

    #[test]
    fn row_accessible_label_keeps_local_source_in_english() {
        let label = search_result_row_accessible_label(
            AppLanguage::English,
            "The Bends",
            "Radiohead",
            "Local • 12 tracks",
            false,
            false,
        );
        assert!(label.contains("Source: local library"));
        assert!(label.contains("Press Enter to open"));
    }

    #[test]
    fn row_accessible_label_uses_one_metadata_sentence_when_secondary_and_detail_match() {
        let label = search_result_row_accessible_label(
            AppLanguage::Spanish,
            "Playlist",
            "YouTube Music",
            "YouTube Music",
            true,
            false,
        );
        assert!(label.contains("Playlist. YouTube Music. Fuente: YouTube Music."));
        assert!(label.contains("Presiona Enter para abrir"));
    }
}
'''

raise SystemExit(namespace["main"]())

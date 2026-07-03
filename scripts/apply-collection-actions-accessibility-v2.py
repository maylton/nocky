#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

SOURCE = Path(__file__).with_name("apply-collection-actions-accessibility.py")
spec = importlib.util.spec_from_file_location("nocky_collection_accessibility_patch", SOURCE)
if spec is None or spec.loader is None:
    raise SystemExit(f"Could not load {SOURCE}")

module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
base_patch_browser = module.patch_browser


def scoped_patch_browser(text: str) -> str:
    start_marker = "#[derive(Clone)]\nstruct CollectionActionSpec"
    end_marker = "fn collection_button(\n"

    start = text.find(start_marker)
    if start < 0:
        raise module.PatchError("Reusable collection action component was not found")

    end = text.find(end_marker, start)
    if end < 0:
        raise module.PatchError("Collection action component boundary was not found")

    # The Home already contains similar play and overflow code. Restrict the
    # applicator to the extracted collection-page component so repeated GTK
    # snippets cannot match the older Home implementation.
    segment = text[start:end] + end_marker
    patched_segment = base_patch_browser(segment)
    if not patched_segment.endswith(end_marker):
        raise module.PatchError("Accessibility patch changed the component boundary")

    patched_body = patched_segment[: -len(end_marker)]
    return text[:start] + patched_body + text[end:]


module.patch_browser = scoped_patch_browser
raise SystemExit(module.main())

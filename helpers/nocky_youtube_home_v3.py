from __future__ import annotations

from typing import Any


def build(response: dict[str, Any], *, selected_chip_params: str = "", section_limit: int = 6) -> dict[str, Any]:
    return {
        "version": 3,
        "selected_chip_params": selected_chip_params,
        "sections": [],
        "chips": [],
        "continuation": "",
    }

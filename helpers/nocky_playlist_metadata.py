"""Compatibility module for the read-only playlist metadata contract.

Installed builds carry the normalizer in ``nocky_playlist_mutations`` because
that module is already part of the packaged YouTube helper surface. Keeping this
module as a re-export preserves source checkouts, tests and standalone tools.
"""

from nocky_playlist_mutations import normalize_playlist_detail

__all__ = ["normalize_playlist_detail"]

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "helpers"))

from nocky_youtube_home_v3 import build


def test_build_extracts_native_home_v3_contract() -> None:
    response = {
        "contents": [
            {
                "chipCloudChipRenderer": {
                    "text": {"runs": [{"text": "Energize"}]},
                    "navigationEndpoint": {
                        "browseEndpoint": {
                            "browseId": "FEmusic_home",
                            "params": "chip-params",
                        }
                    },
                }
            },
            {
                "musicCarouselShelfRenderer": {
                    "header": {
                        "musicCarouselShelfBasicHeaderRenderer": {
                            "title": {"runs": [{"text": "Quick picks"}]}
                        }
                    },
                    "contents": [
                        {
                            "musicTwoRowItemRenderer": {
                                "title": {"runs": [{"text": "Song"}]},
                                "subtitle": {"runs": [{"text": "Artist"}]},
                                "thumbnailRenderer": {
                                    "musicThumbnailRenderer": {
                                        "thumbnail": {
                                            "thumbnails": [
                                                {"url": "https://example.invalid/small.jpg"},
                                                {"url": "https://example.invalid/large.jpg"},
                                            ]
                                        }
                                    }
                                },
                                "navigationEndpoint": {
                                    "watchEndpoint": {"videoId": "video-id"}
                                },
                            }
                        }
                    ],
                }
            },
        ],
        "continuationContents": {
            "musicShelfContinuation": {
                "continuations": [
                    {"nextContinuationData": {"continuation": "next-token"}}
                ]
            }
        },
    }

    page = build(response, selected_chip_params="chip-params")

    assert page["version"] == 3
    assert page["selected_chip_params"] == "chip-params"
    assert page["chips"] == [{"title": "Energize", "params": "chip-params"}]
    assert page["continuation"] == "next-token"

    assert len(page["sections"]) == 1
    section = page["sections"][0]
    assert section["title"] == "Quick picks"
    assert section["layout"] == "carousel"

    item = section["items"][0]
    assert item["result_type"] == "song"
    assert item["title"] == "Song"
    assert item["subtitle"] == "Artist"
    assert item["video_id"] == "video-id"
    assert item["thumbnail_url"] == "https://example.invalid/large.jpg"


def test_build_does_not_invent_fallback_content() -> None:
    page = build({}, selected_chip_params="")

    assert page == {
        "version": 3,
        "selected_chip_params": "",
        "sections": [],
        "chips": [],
        "continuation": "",
    }


if __name__ == "__main__":
    test_build_extracts_native_home_v3_contract()
    test_build_does_not_invent_fallback_content()
    print("home_v3_helper_contract: ok")

from __future__ import annotations

import sys
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "helpers"))

import nocky_youtube as helper


class FakeClient:
    def __init__(self, responses):
        self.responses = responses if isinstance(responses, list) else [responses]
        self.calls = []

    def _send_request(self, endpoint, body, additional_params=None):
        self.calls.append((endpoint, dict(body), additional_params))
        index = min(len(self.calls) - 1, len(self.responses) - 1)
        return self.responses[index]


class YouTubeHomeChipTests(unittest.TestCase):
    def test_filtered_home_uses_web_browse_params_and_parser(self):
        response = {
            "contents": {
                "singleColumnBrowseResultsRenderer": {
                    "tabs": [
                        {
                            "tabRenderer": {
                                "content": {
                                    "sectionListRenderer": {
                                        "contents": [{"musicCarouselShelfRenderer": {}}]
                                    }
                                }
                            }
                        }
                    ]
                }
            }
        }
        client = FakeClient(response)
        parsed = [{"title": "Energy", "contents": [{"title": "Song"}]}]
        with (
            mock.patch.object(helper, "ytmusic_parse_mixed_content", return_value=parsed),
            mock.patch.object(helper, "ytmusic_get_continuation_params", None),
        ):
            rows, raw = helper._inner_tube_home_rows(client, "mood-energy", 6)

        self.assertEqual(rows, parsed)
        self.assertIs(raw, response)
        self.assertEqual(client.calls[0][0], "browse")
        self.assertEqual(
            client.calls[0][1],
            {"browseId": "FEmusic_home", "params": "mood-energy"},
        )

    def test_filtered_home_continuation_reuses_params_and_raw_artwork(self):
        response = {
            "contents": {
                "singleColumnBrowseResultsRenderer": {
                    "tabs": [
                        {
                            "tabRenderer": {
                                "content": {
                                    "sectionListRenderer": {
                                        "contents": [{"musicCarouselShelfRenderer": {}}],
                                        "continuations": [
                                            {"nextContinuationData": {"continuation": "next"}}
                                        ],
                                    }
                                }
                            }
                        }
                    ]
                }
            }
        }
        continuation = {
            "continuationContents": {
                "sectionListContinuation": {
                    "contents": [
                        {
                            "musicCarouselShelfRenderer": {
                                "header": {
                                    "musicCarouselShelfBasicHeaderRenderer": {
                                        "title": {"runs": [{"text": "More"}]}
                                    }
                                },
                                "contents": [
                                    {
                                        "musicTwoRowItemRenderer": {
                                            "title": {"runs": [{"text": "More song"}]},
                                            "navigationEndpoint": {
                                                "watchEndpoint": {"videoId": "abcdefghijk"}
                                            },
                                            "thumbnailRenderer": {
                                                "musicThumbnailRenderer": {
                                                    "thumbnail": {
                                                        "thumbnails": [
                                                            {
                                                                "url": "https://lh3.googleusercontent.com/more=s240",
                                                                "width": 240,
                                                                "height": 240,
                                                            }
                                                        ]
                                                    }
                                                }
                                            },
                                        }
                                    }
                                ],
                            }
                        }
                    ]
                }
            }
        }
        client = FakeClient([response, continuation])
        parsed_pages = [
            [{"title": "First", "contents": []}],
            [
                {
                    "title": "More",
                    "contents": [
                        {"title": "More song", "videoId": "abcdefghijk"}
                    ],
                }
            ],
        ]
        continuation_params = "&ctoken=next&continuation=next"

        with (
            mock.patch.object(
                helper,
                "ytmusic_parse_mixed_content",
                side_effect=parsed_pages,
            ),
            mock.patch.object(
                helper,
                "ytmusic_get_continuation_params",
                return_value=continuation_params,
            ),
        ):
            rows, _raw = helper._inner_tube_home_rows(client, "mood-relax", 6)

        self.assertEqual([row["title"] for row in rows], ["First", "More"])
        self.assertEqual(client.calls[1][1]["params"], "mood-relax")
        self.assertEqual(client.calls[1][2], continuation_params)
        thumbnails = rows[1]["contents"][0]["rawRendererThumbnails"]
        self.assertEqual(thumbnails[0]["width"], 240)
        self.assertIn("more=s240", thumbnails[0]["url"])



if __name__ == "__main__":
    unittest.main()

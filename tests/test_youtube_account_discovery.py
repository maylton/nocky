from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "helpers"))

from nocky_account_discovery import discover_account_profiles  # noqa: E402


def runs(text: str) -> dict[str, object]:
    return {"runs": [{"text": text}]}


def account(
    name: str,
    *,
    handle: str = "",
    profile_id: str = "",
    selected: bool = False,
    photo: str = "https://example.invalid/avatar.jpg",
) -> dict[str, object]:
    item: dict[str, object] = {
        "accountName": runs(name),
        "isSelected": selected,
        "accountPhoto": {"thumbnails": [{"url": photo}]},
    }
    if handle:
        item["channelHandle"] = runs(handle)
    if profile_id:
        item["serviceEndpoint"] = {
            "selectActiveIdentityEndpoint": {
                "supportedTokens": [{"pageIdToken": {"pageId": profile_id}}],
                "clickTrackingParams": "not-part-of-contract",
            }
        }
    return {"accountItem": item}


def payload(*items: dict[str, object]) -> dict[str, object]:
    return {
        "actions": [
            {
                "getMultiPageMenuAction": {
                    "menu": {
                        "multiPageMenuRenderer": {
                            "sections": [
                                {
                                    "accountSectionListRenderer": {
                                        "header": {"privateMetadata": "ignored"},
                                        "contents": [
                                            {
                                                "accountItemSectionRenderer": {
                                                    "contents": list(items)
                                                }
                                            }
                                        ],
                                    }
                                }
                            ]
                        }
                    }
                }
            }
        ],
        "responseContext": {"privateMetadata": "ignored-too"},
    }


class AccountDiscoveryTests(unittest.TestCase):
    def test_multiple_profiles_are_deterministic(self) -> None:
        result = discover_account_profiles(
            payload(
                account("Primary", handle="@primary", selected=True),
                account(
                    "Brand One",
                    handle="@brandone",
                    profile_id="111111111111111111111",
                ),
                account(
                    "Brand Two",
                    handle="@brandtwo",
                    profile_id="222222222222222222222",
                ),
            )
        )

        self.assertEqual(result["state"], "multiple")
        self.assertTrue(result["deterministic"])
        self.assertEqual(result["profiles"][0]["profile_id"], "primary")
        self.assertEqual(result["profiles"][1]["kind"], "brand")

    def test_empty_response_is_unavailable(self) -> None:
        self.assertEqual(
            discover_account_profiles({}),
            {"state": "unavailable", "deterministic": False, "profiles": []},
        )

    def test_missing_stable_ids_are_ambiguous(self) -> None:
        result = discover_account_profiles(
            payload(
                account("First", selected=True),
                account("Second"),
            )
        )

        self.assertEqual(result["state"], "ambiguous")
        self.assertFalse(result["deterministic"])
        self.assertTrue(all(not item["switchable"] for item in result["profiles"]))

    def test_invalid_id_is_not_returned(self) -> None:
        result = discover_account_profiles(
            payload(account("Invalid", profile_id="invalid-id", selected=True))
        )

        self.assertNotIn("invalid-id", json.dumps(result))
        self.assertEqual(result["profiles"][0]["profile_id"], "primary")

    def test_non_https_photo_is_rejected(self) -> None:
        result = discover_account_profiles(
            payload(account("Unsafe", selected=True, photo="data:text/plain,bad"))
        )
        self.assertEqual(result["profiles"][0]["photo_url"], "")

    def test_duplicate_brand_id_is_removed(self) -> None:
        result = discover_account_profiles(
            payload(
                account("Primary", selected=True),
                account("Brand", profile_id="111111111111111111111"),
                account("Duplicate", profile_id="111111111111111111111"),
            )
        )
        self.assertEqual(len(result["profiles"]), 2)

    def test_contract_is_allowlisted(self) -> None:
        result = discover_account_profiles(
            payload(
                account(
                    "Safe",
                    handle="@safe",
                    profile_id="111111111111111111111",
                    selected=True,
                )
            )
        )
        serialized = json.dumps(result)
        self.assertNotIn("privateMetadata", serialized)
        self.assertNotIn("clickTrackingParams", serialized)
        self.assertEqual(
            set(result["profiles"][0]),
            {
                "profile_id",
                "name",
                "channel_handle",
                "photo_url",
                "kind",
                "is_selected",
                "switchable",
            },
        )


if __name__ == "__main__":
    unittest.main()

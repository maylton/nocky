#!/usr/bin/env python3
from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
HELPER = Path(os.environ.get("NOCKY_YOUTUBE_HELPER", ROOT / "helpers" / "nocky_youtube.py"))
PYTHON = Path(os.environ.get("NOCKY_PYTHON", sys.executable))


def invoke(config_path: Path, override: str | None = None) -> list[str]:
    environment = os.environ.copy()
    environment["NOCKY_CONFIG_FILE"] = str(config_path)
    if override is None:
        environment.pop("NOCKY_YOUTUBE_STREAM_CLIENTS", None)
    else:
        environment["NOCKY_YOUTUBE_STREAM_CLIENTS"] = override

    process = subprocess.run(
        [str(PYTHON), str(HELPER), "stream_clients"],
        input="{}\n",
        text=True,
        capture_output=True,
        env=environment,
        check=False,
    )
    if process.returncode != 0:
        raise SystemExit(process.stderr.strip() or "stream_clients failed")

    payload = json.loads(process.stdout)
    if not payload.get("ok"):
        raise SystemExit(payload.get("error") or "stream_clients failed")
    return list((payload.get("result") or {}).get("order") or [])


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="nocky-stream-preferences-") as temporary:
        config_path = Path(temporary) / "config.json"
        config_path.write_text(
            json.dumps(
                {
                    "youtube_stream_sources": {
                        "order": [
                            "ios",
                            "tv",
                            "web_music",
                            "android_vr",
                            "web",
                            "web_creator",
                        ],
                        "disabled": ["web_music", "web_creator"],
                    }
                }
            ),
            encoding="utf-8",
        )

        persisted = invoke(config_path)
        expected_persisted = ["ios", "tv", "android_vr", "web"]
        if persisted != expected_persisted:
            raise SystemExit(
                f"persisted order mismatch: {persisted!r} != {expected_persisted!r}"
            )
        print("Persisted preference reached helper:", " -> ".join(persisted))

        overridden = invoke(config_path, "android_vr,web")
        expected_override = ["android_vr", "web"]
        if overridden != expected_override:
            raise SystemExit(
                f"override order mismatch: {overridden!r} != {expected_override!r}"
            )
        print("Environment override has priority:", " -> ".join(overridden))

        sys.path.insert(0, str(ROOT / "helpers"))
        os.environ["NOCKY_CONFIG_FILE"] = str(config_path)
        os.environ.pop("NOCKY_YOUTUBE_STREAM_CLIENTS", None)
        from nocky_stream_clients import ordered_profiles

        recovery = [
            profile.key
            for profile in ordered_profiles(has_auth=True, failed_client="ios")
        ]
        expected_recovery = ["tv", "android_vr", "web", "ios"]
        if recovery != expected_recovery:
            raise SystemExit(
                f"recovery order mismatch: {recovery!r} != {expected_recovery!r}"
            )
        print("Rejected source rotates to the end:", " -> ".join(recovery))

    print("YouTube stream preference smoke test passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

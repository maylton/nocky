#!/usr/bin/env python3
from pathlib import Path


def replace_once(path: str, old: str, new: str, label: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one match, found {count}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


replace_once(
    "helpers/nocky_youtube.py",
    '''from nocky_youtube_innertube_home import (
    missing_artwork_by_section,
    parse_inner_tube_home_sections,
)

from nocky_stream_clients import (
''',
    '''from nocky_youtube_home_debug import write_home_debug_dump
from nocky_youtube_innertube_home import (
    missing_artwork_by_section,
    parse_inner_tube_home_sections,
)

from nocky_stream_clients import (
''',
    "debug helper import",
)

replace_once(
    "helpers/nocky_youtube.py",
    '''    body, response = _inner_tube_home_response(client, params)
    section_list = find_inner_tube_home_section_list(response)
''',
    '''    body, response = _inner_tube_home_response(client, params)
    debug_pages: list[dict[str, Any]] = [{"kind": "root", "response": response}]
    section_list = find_inner_tube_home_section_list(response)
''',
    "root debug page",
)

replace_once(
    "helpers/nocky_youtube.py",
    '''        continuation_response = sender("browse", body, additional_params)
        continuation_contents = continuation_response.get("continuationContents")
''',
    '''        continuation_response = sender("browse", body, additional_params)
        if isinstance(continuation_response, dict):
            debug_pages.append({"kind": "continuation", "response": continuation_response})
        continuation_contents = continuation_response.get("continuationContents")
''',
    "continuation debug page",
)

replace_once(
    "helpers/nocky_youtube.py",
    '''    missing = missing_artwork_by_section(rows)
    if missing:
        summary = ", ".join(
            f"{title}: {count}/{total}"
            for title, count, total in missing[:12]
        )
        print(
            f"Nocky YouTube raw Home items still missing artwork: {summary}",
            file=sys.stderr,
        )
    return rows, response
''',
    '''    missing = missing_artwork_by_section(rows)
    if missing:
        summary = ", ".join(
            f"{title}: {count}/{total}"
            for title, count, total in missing[:12]
        )
        print(
            f"Nocky YouTube raw Home items still missing artwork: {summary}",
            file=sys.stderr,
        )

    debug_destination = str(os.environ.get("NOCKY_HOME_DEBUG_DUMP") or "").strip()
    if debug_destination:
        try:
            debug_path = write_home_debug_dump(
                debug_destination,
                pages=debug_pages,
                rows=rows,
                selected_params=params,
            )
            print(f"Nocky YouTube Home renderer diagnostics: {debug_path}", file=sys.stderr)
        except Exception as debug_error:
            print(
                f"Nocky YouTube Home renderer diagnostics failed: {debug_error}",
                file=sys.stderr,
            )
    return rows, response
''',
    "debug dump write",
)

print("Home renderer debug dump integrated")

#!/usr/bin/env python3
"""Summarize Nocky's opt-in `[perf]` trace logs.

Usage:
    scripts/perf-log-summary.py /tmp/nocky-perf.log
    NOCKY_PERF_TRACE=1 cargo run 2>&1 | scripts/perf-log-summary.py
"""

from __future__ import annotations

import argparse
import re
import statistics
import sys
from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path
from typing import Iterable, TextIO

PERF_PREFIX = "[perf] "
TOKEN_RE = re.compile(r'''(\w+)=((?:"(?:\\.|[^"])*")|\S+)''')


@dataclass
class EventStats:
    count: int = 0
    durations: list[int] = field(default_factory=list)

    def add(self, duration_ms: int | None) -> None:
        self.count += 1
        if duration_ms is not None:
            self.durations.append(duration_ms)

    @property
    def timed_count(self) -> int:
        return len(self.durations)

    @property
    def min_ms(self) -> int | None:
        return min(self.durations) if self.durations else None

    @property
    def max_ms(self) -> int | None:
        return max(self.durations) if self.durations else None

    @property
    def avg_ms(self) -> float | None:
        return statistics.fmean(self.durations) if self.durations else None


def parse_value(value: str) -> str:
    if len(value) >= 2 and value[0] == '"' and value[-1] == '"':
        return bytes(value[1:-1], "utf-8").decode("unicode_escape")
    return value


def parse_perf_line(line: str) -> tuple[str, int | None] | None:
    if not line.startswith(PERF_PREFIX):
        return None

    fields = {key: parse_value(value) for key, value in TOKEN_RE.findall(line)}
    event = fields.get("event")
    if not event:
        return None

    duration_raw = fields.get("duration_ms")
    duration_ms = None
    if duration_raw is not None:
        try:
            duration_ms = int(duration_raw)
        except ValueError:
            duration_ms = None

    return event, duration_ms


def read_lines(path: str | None) -> Iterable[str]:
    if path is None or path == "-":
        yield from sys.stdin
        return

    with Path(path).open("r", encoding="utf-8", errors="replace") as handle:
        yield from handle


def summarize(lines: Iterable[str]) -> dict[str, EventStats]:
    stats: dict[str, EventStats] = defaultdict(EventStats)
    for line in lines:
        parsed = parse_perf_line(line.strip())
        if parsed is None:
            continue
        event, duration_ms = parsed
        stats[event].add(duration_ms)
    return dict(stats)


def print_summary(stats: dict[str, EventStats], output: TextIO) -> None:
    if not stats:
        print("No [perf] events found.", file=output)
        return

    print(
        f"{'event':40} {'count':>7} {'timed':>7} {'min_ms':>8} {'avg_ms':>8} {'max_ms':>8}",
        file=output,
    )
    print("-" * 84, file=output)

    for event in sorted(stats):
        event_stats = stats[event]
        min_ms = "-" if event_stats.min_ms is None else str(event_stats.min_ms)
        avg_ms = "-" if event_stats.avg_ms is None else f"{event_stats.avg_ms:.1f}"
        max_ms = "-" if event_stats.max_ms is None else str(event_stats.max_ms)
        print(
            f"{event:40} {event_stats.count:7d} {event_stats.timed_count:7d} "
            f"{min_ms:>8} {avg_ms:>8} {max_ms:>8}",
            file=output,
        )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "logfile",
        nargs="?",
        help="Path to a Nocky perf log. Reads stdin when omitted or set to '-'.",
    )
    args = parser.parse_args()

    stats = summarize(read_lines(args.logfile))
    print_summary(stats, sys.stdout)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

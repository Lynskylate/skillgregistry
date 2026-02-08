#!/usr/bin/env python3
"""Convert cargo llvm-cov JSON output to coverage/coverage-summary.json format.

Usage:
  python3 scripts/coverage_summary.py backend/llvm-cov.json coverage/coverage-summary.json
"""

from __future__ import annotations

import json
import os
import sys
from pathlib import Path
from typing import Any, Dict, List


def percent(covered: int, total: int) -> float:
    if total <= 0:
        return 100.0
    return round((covered * 100.0) / total, 2)


def metric_block(summary: Dict[str, Any], key: str) -> Dict[str, Any]:
    data = summary.get(key, {})
    covered = int(data.get("covered", 0) or 0)
    total = int(data.get("count", 0) or 0)
    return {
        "total": total,
        "covered": covered,
        "skipped": 0,
        "pct": percent(covered, total),
    }


def normalize_path(path: str) -> str:
    marker = "/backend/"
    if marker in path:
        return "backend/" + path.split(marker, 1)[1]
    return path


def read_llvm_cov(path: Path) -> Dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def collect_files(data: Dict[str, Any]) -> List[Dict[str, Any]]:
    rows: List[Dict[str, Any]] = []
    for unit in data.get("data", []):
        rows.extend(unit.get("files", []))
    return rows


def build_summary(data: Dict[str, Any]) -> Dict[str, Any]:
    files_summary: Dict[str, Any] = {}

    for file_entry in collect_files(data):
        filename = file_entry.get("filename", "")
        if not isinstance(filename, str) or not filename.endswith(".rs"):
            continue

        summary = file_entry.get("summary", {})
        files_summary[normalize_path(filename)] = {
            "lines": metric_block(summary, "lines"),
            "functions": metric_block(summary, "functions"),
            "branches": metric_block(summary, "branches"),
            "statements": metric_block(summary, "regions"),
        }

    totals = {}
    if data.get("data"):
        totals = data["data"][0].get("totals", {})

    return {
        "total": {
            "lines": metric_block(totals, "lines"),
            "functions": metric_block(totals, "functions"),
            "branches": metric_block(totals, "branches"),
            "statements": metric_block(totals, "regions"),
        },
        "files": dict(sorted(files_summary.items())),
    }


def main() -> int:
    if len(sys.argv) != 3:
        print(
            "Usage: python3 scripts/coverage_summary.py <llvm-cov.json> <coverage-summary.json>",
            file=sys.stderr,
        )
        return 2

    src = Path(sys.argv[1])
    dst = Path(sys.argv[2])

    if not src.exists():
        print(f"Input file not found: {src}", file=sys.stderr)
        return 2

    data = read_llvm_cov(src)
    summary = build_summary(data)

    dst.parent.mkdir(parents=True, exist_ok=True)
    with dst.open("w", encoding="utf-8") as handle:
        json.dump(summary, handle, indent=2, sort_keys=True)
        handle.write("\n")

    lines = summary["total"]["lines"]
    print(
        f"Wrote {dst} (lines: {lines['covered']}/{lines['total']} = {lines['pct']:.2f}%)"
    )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())

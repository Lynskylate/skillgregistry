#!/usr/bin/env python3
"""Extract and normalize Claude CLI result payloads."""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

FENCE_RE = re.compile(r"^```(?P<label>[a-zA-Z0-9_-]*)\n(?P<body>[\s\S]*?)\n```$", re.MULTILINE)


def strip_fences(text: str) -> str:
    trimmed = text.strip()
    match = FENCE_RE.match(trimmed)
    if not match:
        return text
    return match.group("body")


def main() -> int:
    parser = argparse.ArgumentParser(description="Extract .result from Claude JSON output")
    parser.add_argument("input", help="Path to Claude output JSON file")
    parser.add_argument(
        "--expect",
        choices=["any", "diff", "json"],
        default="any",
        help="Optional output validation",
    )
    parser.add_argument(
        "--strip-fences",
        action="store_true",
        help="Strip one outer markdown code fence if present",
    )
    args = parser.parse_args()

    raw = Path(args.input).read_text()
    try:
        parsed = json.loads(raw)
    except json.JSONDecodeError as exc:
        print(f"Invalid JSON file: {exc}", file=sys.stderr)
        return 1

    payload = parsed.get("result")
    if not isinstance(payload, str):
        print("Missing string field result", file=sys.stderr)
        return 1

    output = strip_fences(payload) if args.strip_fences else payload

    if args.expect == "json":
        try:
            json.loads(output)
        except json.JSONDecodeError as exc:
            print(f"Result is not valid JSON: {exc}", file=sys.stderr)
            return 1
    elif args.expect == "diff":
        normalized = output.lstrip()
        if not (normalized.startswith("--- ") or normalized.startswith("diff --git ")):
            print("Result does not look like a unified diff", file=sys.stderr)
            return 1

    sys.stdout.write(output)
    if not output.endswith("\n"):
        sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

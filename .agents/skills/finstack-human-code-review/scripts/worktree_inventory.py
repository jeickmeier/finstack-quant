#!/usr/bin/env python3
"""Read-only Git working-tree scope inventory for human-code-review.

Uses NUL-delimited porcelain status to handle renames and unusual file names.
It enumerates paths/statuses; it does not read or modify source files.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path
from typing import Any

CONFLICT_CODES = frozenset({"DD", "AU", "UD", "UA", "DU", "AA", "UU"})


class InventoryError(Exception):
    """Git could not supply a reliable inventory."""


def git(repo: Path, *arguments: str, check: bool = True) -> subprocess.CompletedProcess[bytes]:
    try:
        result = subprocess.run(
            ["git", "-C", str(repo), *arguments],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
    except OSError as exc:
        raise InventoryError(f"Could not run git: {exc}") from exc
    if check and result.returncode:
        error = result.stderr.decode("utf-8", "replace").strip()
        raise InventoryError(error or f"git {' '.join(arguments)} failed")
    return result


def decode_path(data: bytes) -> str:
    # Git exposes filesystem paths as raw bytes. Preserve odd bytes without loss.
    return data.decode("utf-8", "surrogateescape")


def parse_porcelain_z(raw: bytes) -> list[dict[str, Any]]:
    """Parse `git status --porcelain=v1 -z` entries, including rename sources.

    In -z mode rename/copy records are `XY new_path NUL old_path NUL`.
    """
    if raw and not raw.endswith(b"\0"):
        raise InventoryError("Malformed Git status: missing terminating NUL")

    fields = raw.split(b"\0")
    entries: list[dict[str, Any]] = []
    cursor = 0
    while cursor < len(fields):
        record = fields[cursor]
        cursor += 1
        if not record:
            continue
        if len(record) < 4 or record[2:3] != b" ":
            raise InventoryError("Malformed Git status entry")

        try:
            status = record[:2].decode("ascii")
        except UnicodeDecodeError as exc:
            raise InventoryError("Non-ASCII Git status code") from exc
        path = decode_path(record[3:])
        if not path:
            raise InventoryError("Empty path in Git status")
        old_path: str | None = None
        if "R" in status or "C" in status:
            if cursor >= len(fields) or not fields[cursor]:
                raise InventoryError("Malformed Git rename/copy status")
            old_path = decode_path(fields[cursor])
            cursor += 1

        untracked = status == "??"
        index_status, worktree_status = status
        entry: dict[str, Any] = {
            "path": path,
            "status": status,
            "staged": not untracked and index_status not in (" ", "?", "!"),
            "unstaged": not untracked and worktree_status not in (" ", "?", "!"),
            "untracked": untracked,
            "conflicted": status in CONFLICT_CODES,
            "deleted": "D" in status,
            "renamed_or_copied": old_path is not None,
        }
        if old_path is not None:
            entry["old_path"] = old_path
        entries.append(entry)

    return entries


def collect(repo: Path) -> dict[str, Any]:
    root_bytes = git(repo, "rev-parse", "--show-toplevel").stdout
    root = Path(decode_path(root_bytes.rstrip(b"\n")))
    has_head = git(root, "rev-parse", "--verify", "HEAD", check=False).returncode == 0
    # --untracked-files=all includes individual untracked files, not only folders.
    # --ignore-submodules=none surfaces a changed submodule but not its internal files.
    raw = git(
        root, "status", "--porcelain=v1", "-z", "--untracked-files=all",
        "--ignore-submodules=none",
    ).stdout
    entries = parse_porcelain_z(raw)
    return {
        "repository_root": str(root),
        "has_head_commit": has_head,
        "counts": {
            "changed_paths": len(entries),
            "staged": sum(item["staged"] for item in entries),
            "unstaged": sum(item["unstaged"] for item in entries),
            "untracked": sum(item["untracked"] for item in entries),
            "conflicted": sum(item["conflicted"] for item in entries),
            "deleted": sum(item["deleted"] for item in entries),
            "renamed_or_copied": sum(item["renamed_or_copied"] for item in entries),
        },
        "entries": entries,
        "notes": [
            "Paths are repository-relative; the status codes describe index and working-tree state.",
            "Ignored files are excluded. Untracked files must be opened separately; git diff does not show them.",
            "Submodules appear as entries; internal submodule changes are not enumerated.",
        ],
    }


def print_text(inventory: dict[str, Any], limit: int) -> None:
    print(f"Repository: {inventory['repository_root']}")
    print(f"HEAD commit exists: {inventory['has_head_commit']}")
    print("Counts: " + ", ".join(f"{key}={value}" for key, value in inventory["counts"].items()))
    entries = inventory["entries"]
    for item in entries[:limit]:
        labels = []
        for key in ("staged", "unstaged", "untracked", "conflicted"):
            if item[key]:
                labels.append(key)
        origin = f" (from {json.dumps(item['old_path'], ensure_ascii=True)})" if "old_path" in item else ""
        print(f"  {item['status']} [{', '.join(labels)}] {json.dumps(item['path'], ensure_ascii=True)}{origin}")
    if len(entries) > limit:
        print(f"... {len(entries) - limit} additional paths NOT DISPLAYED (use --format json or a larger --limit)")
    for note in inventory["notes"]:
        print("Note: " + note)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path("."), help="Repository path; defaults to cwd")
    parser.add_argument("--format", choices=("text", "json"), default="text")
    parser.add_argument("--limit", type=int, default=200, help="Maximum paths shown in text output (default: 200)")
    args = parser.parse_args()
    if args.limit < 0:
        parser.error("--limit must be nonnegative")
    try:
        inventory = collect(args.repo)
    except InventoryError as exc:
        print(f"Inventory failed: {exc}", file=sys.stderr)
        return 2
    if args.format == "json":
        print(json.dumps(inventory, indent=2, ensure_ascii=True))
    else:
        print_text(inventory, args.limit)
    return 0


if __name__ == "__main__":
    sys.exit(main())

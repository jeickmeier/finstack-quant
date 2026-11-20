"""Check tracked text formatting without modifying files."""

from __future__ import annotations

import argparse
from pathlib import Path


def check_end_of_file(content: bytes) -> bool:
    """Allow an empty file or one terminal line ending, including CRLF."""
    if not content:
        return True
    body = content.rstrip(b"\r\n")
    return bool(body) and content[len(body) :] in (b"\n", b"\r\n", b"\r")


def main() -> int:
    """Report paths failing the requested text check."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("check", choices=("trailing-whitespace", "end-of-file", "lf"))
    parser.add_argument("paths", nargs="+", type=Path)
    args = parser.parse_args()
    status = 0
    for path in args.paths:
        content = path.read_bytes()
        match args.check:
            case "trailing-whitespace":
                valid = all(line == line.rstrip() for line in content.splitlines())
            case "end-of-file":
                valid = check_end_of_file(content)
            case _:
                valid = b"\r" not in content
        if not valid:
            print(f"{path}: failed {args.check} check; run mise run all-fmt")
            status = 1
    return status


if __name__ == "__main__":
    raise SystemExit(main())

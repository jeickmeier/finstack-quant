"""Update or verify the benchmark result SHA-256 recorded in Markdown."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import re
import sys

_DIGEST_PATTERN = re.compile(
    r"(Its\s+SHA-256 is `)[0-9a-f]{64}(`\.)",
    flags=re.MULTILINE,
)


def synchronized_text(document: str, digest: str) -> str:
    """Return documentation containing exactly one current artifact digest."""
    updated, replacements = _DIGEST_PATTERN.subn(rf"\g<1>{digest}\g<2>", document)
    if replacements != 1:
        raise ValueError(f"expected exactly one benchmark digest, found {replacements}")
    return updated


def main() -> int:
    """Write or check the documented benchmark artifact digest."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifact", type=Path, required=True)
    parser.add_argument("--document", type=Path, required=True)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--write", action="store_true")
    mode.add_argument("--check", action="store_true")
    args = parser.parse_args()

    digest = hashlib.sha256(args.artifact.read_bytes()).hexdigest()
    original = args.document.read_text(encoding="utf-8")
    expected = synchronized_text(original, digest)
    if args.check:
        if original != expected:
            print(
                f"{args.document} does not record artifact SHA-256 {digest}",
                file=sys.stderr,
            )
            return 1
        print(f"Materialization benchmark artifact digest verified: {digest}")
        return 0
    args.document.write_text(expected, encoding="utf-8")
    print(f"Materialization benchmark artifact digest updated: {digest}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

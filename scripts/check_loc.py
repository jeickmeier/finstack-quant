"""Enforce the Rust source-file size limit.

Counts the non-test lines of every hand-written Rust source file under a
crate's ``src/`` directory and fails when one exceeds the hard limit (2000 by
default). Files over the soft limit (1000) are summarised on one line;
``--full`` lists them.

Excluded from the count, because the rule in
``.agents/rules/rust/code-standards.md`` measures source, not tests:

- items under ``#[test]``-style attributes and ``#[cfg(test)]`` (also
  ``#[cfg(all(test, ...))]``), but not ``#[cfg(not(test))]`` or
  ``#[cfg(any(test, ...))]``, which also compile outside tests;
- sibling test files (``tests.rs``, ``*_tests.rs``) and anything under a
  ``tests/`` directory;
- generated sources (any path segment named ``generated``).
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import os
from pathlib import Path
import re
import sys

HARD_LIMIT = 2000
SOFT_LIMIT = 1000
# Directories that hold hand-written Rust crates, relative to the repository root.
SOURCE_ROOTS = ("finstack-quant", "finstack-quant-py/src", "finstack-quant-wasm/src")
SKIPPED_DIRS = {"target", "node_modules", "pkg", "pkg-node", "tests", "generated", "__pycache__"}
TEST_ATTRIBUTE_RE = re.compile(r"^\s*#\[\s*(?:[A-Za-z_]\w*::)*test\b[^\]]*\]\s*$")
CFG_RE = re.compile(r"^\s*#\[\s*cfg\s*\((?P<predicate>.*)\)\s*\]\s*$")


@dataclass(frozen=True, slots=True)
class Args:
    """Parsed command-line arguments."""

    limit: int
    report_at: int
    ci: bool
    full: bool


def parse_args(argv: list[str] | None = None) -> Args:
    """Parse the command line."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("limit", nargs="?", type=int, default=HARD_LIMIT, help="hard limit (fail above it)")
    parser.add_argument("--report-at", type=int, default=SOFT_LIMIT, help="soft limit (summarised above it)")
    parser.add_argument("--ci", action="store_true", help="exit 1 when a file exceeds the hard limit")
    parser.add_argument("--full", action="store_true", help="list every file above the soft limit")
    ns = parser.parse_args(argv)
    return Args(limit=ns.limit, report_at=ns.report_at, ci=ns.ci, full=ns.full)


def is_test_file(path: Path) -> bool:
    """Return whether a Rust file is a sibling test module."""
    return path.name == "tests.rs" or path.name.endswith("_tests.rs")


def iter_rust_sources(root: Path) -> list[Path]:
    """Collect hand-written Rust files under each crate's ``src/`` directory."""
    files: list[Path] = []
    for source_root in SOURCE_ROOTS:
        base = root / source_root
        if not base.is_dir():
            continue
        for current, dirnames, filenames in os.walk(base):
            dirnames[:] = sorted(d for d in dirnames if d not in SKIPPED_DIRS and not d.startswith("."))
            current_path = Path(current)
            if "src" not in current_path.relative_to(root).parts:
                continue
            for filename in sorted(filenames):
                path = current_path / filename
                if path.suffix == ".rs" and not is_test_file(path):
                    files.append(path)
    return files


def is_test_only_attribute(line: str) -> bool:
    """Return whether a line is an attribute that makes the next item test-only."""
    if TEST_ATTRIBUTE_RE.match(line):
        return True
    match = CFG_RE.match(line)
    if match is None:
        return False
    predicate = re.sub(r"\s+", "", match.group("predicate"))
    if predicate == "test":
        return True
    # `all(test, ...)` compiles only under test; `any(test, ...)` and `not(test)` do not.
    return (
        predicate.startswith("all(")
        and re.search(r"(?<![\w(])test(?!\w)", predicate[4:]) is not None
        and "not(test" not in predicate
    )


def count_braces(line: str) -> int:
    """Return the net change in brace depth for a line."""
    return line.count("{") - line.count("}")


def count_rust_lines(path: Path) -> int:
    """Count a Rust file's lines, excluding items under test-only attributes."""
    effective = 0
    skipping = False
    started = False
    depth = 0
    with path.open("r", encoding="utf-8", errors="ignore") as handle:
        for line in handle:
            if skipping:
                if not started:
                    if "{" in line:
                        started = True
                        depth = count_braces(line)
                        if depth <= 0:
                            skipping = started = False
                            depth = 0
                    elif ";" in line:
                        skipping = False
                else:
                    depth += count_braces(line)
                    if depth <= 0:
                        skipping = started = False
                        depth = 0
                continue
            if is_test_only_attribute(line):
                skipping = True
                started = False
                depth = 0
                continue
            effective += 1
    return effective


def measure(root: Path) -> list[tuple[str, int]]:
    """Return every Rust source file with its effective line count, largest first."""
    counts = [(p.relative_to(root).as_posix(), count_rust_lines(p)) for p in iter_rust_sources(root)]
    counts.sort(key=lambda item: (-item[1], item[0]))
    return counts


def main(argv: list[str] | None = None) -> int:
    """Run the check and print a report."""
    args = parse_args(argv)
    root = Path(__file__).resolve().parent.parent
    counts = measure(root)
    hard = [(p, n) for p, n in counts if n > args.limit]
    soft = [(p, n) for p, n in counts if args.report_at < n <= args.limit]
    for path, lines in hard:
        print(f"{lines:6d}  {path}  (over the {args.limit}-line limit)")
    if args.full:
        for path, lines in soft:
            print(f"{lines:6d}  {path}")
    if soft:
        hint = "" if args.full else "; --full lists them"
        print(f"{len(soft)} Rust source file(s) between {args.report_at} and {args.limit} lines{hint}.")
    if hard:
        print(f"{len(hard)} Rust source file(s) exceed the {args.limit}-line limit; split them.")
        return 1 if args.ci else 0
    print(f"All {len(counts)} Rust source files are within the {args.limit}-line limit.")
    return 0


if __name__ == "__main__":
    sys.exit(main())

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
- a module's sibling test file (``tests.rs``) and anything under a ``tests/``
  directory;
- generated sources (any path segment named ``generated``).
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))

from rust_source import iter_rust_sources, production_lines

HARD_LIMIT = 2000
SOFT_LIMIT = 1000


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


def count_rust_lines(path: Path) -> int:
    """Count a Rust file's lines, excluding items under test-only attributes."""
    return sum(1 for _ in production_lines(path))


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

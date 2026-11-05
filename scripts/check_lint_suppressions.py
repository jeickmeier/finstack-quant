"""Fail on lint suppressions and test-file names the Rust rules forbid.

`AGENTS.md`, `.agents/rules/rust/code-standards.md` and
`.agents/rules/rust/testing-standards.md` allow, in production Rust:

- no `#[allow(...)]` or `#[expect(...)]` of `clippy::too_many_arguments`,
  `clippy::expect_used`, `clippy::unwrap_used`, `dead_code`,
  `unused_imports` or `missing_docs`: a function with more than seven inputs
  takes a params struct, unused code and imports are deleted (or `cfg`-gated
  when platform-specific), fallible calls propagate errors, and public items
  are documented;
- no sibling test file named other than `tests.rs` (`*_tests.rs`,
  `tests_*.rs`): test-only module files are a module's `tests.rs` or live
  under a `tests/` directory.

Test-only items (`#[cfg(test)]`, `#[test]`), test files and the dev-only
fixture crates (`DEV_ONLY_CRATES`, dev-dependencies of the tests only) are
exempt.
`scripts/lint_suppressions_backlog.txt` lists the suppressions not yet
removed, by `path: lint`; a finding it lists is tolerated, and an entry that no
longer matches a finding fails, so the list only shrinks. `--report` lists the
findings without failing.
"""

from __future__ import annotations

import argparse
from pathlib import Path
import re
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))

from rust_source import SOURCE_ROOTS, iter_rust_sources, production_lines

FORBIDDEN = (
    "clippy::too_many_arguments",
    "clippy::expect_used",
    "clippy::unwrap_used",
    "dead_code",
    "unused_imports",
    "missing_docs",
)
# Dev-dependency-only crates: their helpers panic on malformed fixtures by design.
DEV_ONLY_CRATES = ("finstack-quant/valuations/test-support/", "finstack-quant/test-utils/")
SUPPRESSION_RE = re.compile(r"#!?\[\s*(?:allow|expect)\s*\((?P<lints>[^\]]*)\)\s*\]")


def suppression_findings(root: Path) -> list[str]:
    """Every forbidden suppression in production Rust, as `path:line: message`."""
    found: list[str] = []
    for path in iter_rust_sources(root):
        rel = path.relative_to(root).as_posix()
        if rel.startswith(DEV_ONLY_CRATES):
            continue
        for number, line in production_lines(path):
            for match in SUPPRESSION_RE.finditer(line):
                lints = {lint.strip() for lint in match.group("lints").split(",")}
                found.extend(f"{rel}:{number}: suppresses {lint}" for lint in FORBIDDEN if lint in lints)
    return found


def test_file_findings(root: Path) -> list[str]:
    """Every `*_tests.rs` or `tests_*.rs` under a crate's `src/`, outside `tests/`."""
    found: list[str] = []
    for source_root in SOURCE_ROOTS:
        base = root / source_root
        if not base.is_dir():
            continue
        for path in sorted({*base.rglob("*_tests.rs"), *base.rglob("tests_*.rs")}):
            parts = path.relative_to(root).parts
            if "src" in parts and "target" not in parts and "tests" not in parts:
                found.append(
                    f"{path.relative_to(root).as_posix()}: rename to the module's tests.rs or move under tests/"
                )
    return found


def load_backlog(root: Path) -> set[tuple[str, str]]:
    """The `(path, lint)` pairs `scripts/lint_suppressions_backlog.txt` tolerates."""
    path = root / "scripts" / "lint_suppressions_backlog.txt"
    if not path.is_file():
        return set()
    entries = set()
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        file, lint, _reason = (part.strip() for part in line.split(": ", 2))
        entries.add((file, lint))
    return entries


def apply_backlog(found: list[str], backlog: set[tuple[str, str]]) -> list[str]:
    """Drop findings the backlog tolerates; report backlog entries that match none."""
    remaining = []
    matched = set()
    for finding in found:
        match = re.match(r"^(?P<path>[^:]+):\d+: suppresses (?P<lint>\S+)$", finding)
        key = (match["path"], match["lint"]) if match else None
        if key in backlog:
            matched.add(key)
        else:
            remaining.append(finding)
    remaining.extend(
        f"{path}: backlog entry for {lint} matches no suppression; delete it from scripts/lint_suppressions_backlog.txt"
        for path, lint in sorted(backlog - matched)
    )
    return remaining


def main(argv: list[str] | None = None) -> int:
    """Print the findings; exit 1 on any unless `--report`."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--report", action="store_true", help="list findings without failing")
    args = parser.parse_args(argv)
    root = Path(__file__).resolve().parent.parent
    found = apply_backlog(suppression_findings(root), load_backlog(root)) + test_file_findings(root)
    for finding in found:
        print(finding)
    if found:
        print(f"{len(found)} forbidden lint suppression(s) or test-file name(s) in production Rust.")
        return 0 if args.report else 1
    print("No forbidden lint suppressions or test-file names in production Rust.")
    return 0


if __name__ == "__main__":
    sys.exit(main())

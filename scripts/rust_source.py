"""Shared reading of hand-written Rust sources for the repository checkers.

`check_loc.py` and `check_lint_suppressions.py` both measure production Rust:
the files under each crate's `src/`, minus test-only files (a module's
`tests.rs` and anything under a `tests/` directory), generated code, and the
items under test-only attributes.
"""

from __future__ import annotations

from collections.abc import Iterator
import os
from pathlib import Path
import re

# Directories that hold hand-written Rust crates, relative to the repository root.
SOURCE_ROOTS = ("finstack-quant", "finstack-quant-py/src", "finstack-quant-wasm/src")
# `tests` covers both a crate's integration tests and test-only module files
# under `src/` (`#[cfg(test)] #[path = "tests/x.rs"] mod x_tests;`).
SKIPPED_DIRS = {"target", "node_modules", "pkg", "pkg-node", "tests", "generated", "__pycache__"}
TEST_ATTRIBUTE_RE = re.compile(r"^\s*#\[\s*(?:[A-Za-z_]\w*::)*test\b[^\]]*\]\s*$")
CFG_RE = re.compile(r"^\s*#\[\s*cfg\s*\((?P<predicate>.*)\)\s*\]\s*$")


def is_test_file(path: Path) -> bool:
    """Return whether a Rust file is a module's sibling test file (`tests.rs`)."""
    return path.name == "tests.rs"


def iter_rust_sources(root: Path) -> list[Path]:
    """Hand-written, non-test Rust files under each crate's `src/` directory, sorted."""
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
    """Return whether a line is an attribute that makes the next item test-only.

    `#[test]`-style attributes, `#[cfg(test)]` and `#[cfg(all(test, ...))]`
    qualify; `#[cfg(not(test))]` and `#[cfg(any(test, ...))]` do not, because
    that code also compiles outside tests.
    """
    if TEST_ATTRIBUTE_RE.match(line):
        return True
    match = CFG_RE.match(line)
    if match is None:
        return False
    predicate = re.sub(r"\s+", "", match.group("predicate"))
    if predicate == "test":
        return True
    return (
        predicate.startswith("all(")
        and "not(test" not in predicate
        and re.search(r"(?<![\w(])test(?!\w)", predicate[4:]) is not None
    )


def production_lines(path: Path) -> Iterator[tuple[int, str]]:
    """Yield `(line_number, line)` for a file's lines outside test-only items.

    Items are delimited by brace depth; braces in strings and comments are not
    special-cased, which the formatted sources in this workspace do not need.
    """
    skipping = False
    started = False
    depth = 0
    with path.open("r", encoding="utf-8", errors="ignore") as handle:
        for number, line in enumerate(handle, start=1):
            if skipping:
                if not started:
                    if "{" in line:
                        started = True
                        depth = line.count("{") - line.count("}")
                        if depth <= 0:
                            skipping = started = False
                    elif ";" in line:
                        skipping = False
                else:
                    depth += line.count("{") - line.count("}")
                    if depth <= 0:
                        skipping = started = False
                continue
            if is_test_only_attribute(line):
                skipping = True
                started = False
                depth = 0
                continue
            yield number, line

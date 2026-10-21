"""Reject legacy placeholder prose in public PyO3 runtime documentation.

Only Rust ``///`` blocks are scanned. Owners whose docstring is compiled from the
``.pyi`` stubs (``#[doc = pydoc!(...)]``) carry no Rust prose and are checked by
``check_python_api_input_docs.py`` on the stub instead. Retire this script once
every binding module takes its docstrings from the stubs.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path
import re
import sys

REPO_ROOT = Path(__file__).resolve().parents[1]
BINDINGS_ROOT = REPO_ROOT / "finstack-quant-py" / "src" / "bindings"

PLACEHOLDER_PATTERNS: tuple[tuple[str, re.Pattern[str]], ...] = (
    (
        "callability-only doctest",
        re.compile(r"(?m)^\s*>>>\s*callable\s*\("),
    ),
    (
        "name-only doctest",
        re.compile(r"(?m)^\s*>>>\s*[A-Za-z_][A-Za-z0-9_.]*\.__name__\s*$"),
    ),
    (
        "skipped doctest",
        re.compile(r"#\s*doctest:\s*\+SKIP\b", re.IGNORECASE),
    ),
    (
        "fabricated parameter description",
        re.compile(r"Value supplied for\s+`{1,2}[^`\n]+`{1,2}\s+to the documented binding operation\."),
    ),
    (
        "fabricated return description",
        re.compile(r"Result of [^\n]+ in the annotated representation\."),
    ),
    (
        "fabricated reconstruction description",
        re.compile(r"Validated\s+`{1,2}[^`\n]+`{1,2}\s+instance reconstructed from the canonical JSON payload\."),
    ),
    (
        "fabricated error description",
        re.compile(r"If supplied inputs violate the documented type, shape, finite-value, or domain constraints\."),
    ),
    (
        "generic-nan-sentinel boilerplate",
        re.compile(r"undefined results use ``None``, ``NaN``, or ``inf``"),
    ),
)


@dataclass(frozen=True)
class PublicDocBlock:
    """One Rust doc-comment block attached to a public PyO3 owner."""

    path: Path
    line: int
    owner: str
    text: str


@dataclass(frozen=True)
class PlaceholderError:
    """One placeholder diagnostic in a public PyO3 doc block."""

    path: Path
    line: int
    owner: str
    message: str

    def format(self) -> str:
        """Render the diagnostic in compiler-style form."""
        try:
            display_path = self.path.relative_to(REPO_ROOT)
        except ValueError:
            display_path = self.path
        return f"{display_path}:{self.line}: {self.owner}: {self.message}"


def _pymethod_ranges(lines: list[str]) -> list[tuple[int, int]]:
    """Locate rustfmt-formatted ``#[pymethods]`` implementation bodies."""
    ranges: list[tuple[int, int]] = []
    for marker_index, line in enumerate(lines):
        if line.strip() != "#[pymethods]":
            continue
        impl_index = next(
            (
                index
                for index in range(marker_index + 1, min(marker_index + 20, len(lines)))
                if lines[index].lstrip().startswith("impl ")
            ),
            None,
        )
        if impl_index is None:
            continue
        end_index = next(
            (index for index in range(impl_index + 1, len(lines)) if lines[index] == "}"),
            None,
        )
        if end_index is not None:
            ranges.append((impl_index, end_index))
    return ranges


# Macro invocations whose `///` entries become public PyO3 methods: each doc
# block documents the entry that follows it (`name -> Ret = expr;`, a Greek
# name, or the `build` keyword).
_METHOD_MACRO = re.compile(
    r"^(?:crate::bindings::macros::)?(?:py_getters|instrument_greek_methods|instrument_envelope_methods)!\("
)


def _method_macro_ranges(lines: list[str]) -> list[tuple[int, int]]:
    """Locate invocations of the macros that generate documented PyO3 methods."""
    ranges: list[tuple[int, int]] = []
    for start, line in enumerate(lines):
        if _METHOD_MACRO.match(line) is None:
            continue
        end_index = next(
            (index for index in range(start + 1, len(lines)) if lines[index] in (");", "});")),
            None,
        )
        if end_index is not None:
            ranges.append((start, end_index))
    return ranges


def _following_item(lines: list[str], block_end: int) -> tuple[str, str]:
    """Return attributes and the item line immediately following a doc block."""
    cursor = block_end
    attributes: list[str] = []
    while cursor < len(lines):
        stripped = lines[cursor].strip()
        if not stripped:
            cursor += 1
            continue
        if not stripped.startswith("#["):
            return "\n".join(attributes), stripped
        while cursor < len(lines):
            attribute_line = lines[cursor].strip()
            attributes.append(attribute_line)
            cursor += 1
            if attribute_line.endswith("]"):
                break
    return "\n".join(attributes), ""


def _owner_name(item: str) -> str:
    """Extract a stable owner name from a Rust function, struct, or enum line."""
    match = re.search(r"\b(?:fn|struct|enum)\s+([A-Za-z_][A-Za-z0-9_]*)", item)
    if match is not None:
        return match.group(1)
    entry = re.match(r"([A-Za-z_][A-Za-z0-9_]*)\b", item)
    return entry.group(1) if entry is not None else "PyO3 API"


def public_pyo3_doc_blocks(path: Path) -> list[PublicDocBlock]:
    """Return doc blocks attached to public PyO3 classes, methods, and functions."""
    lines = path.read_text(encoding="utf-8").splitlines()
    pymethod_ranges = _pymethod_ranges(lines)
    macro_ranges = _method_macro_ranges(lines)
    blocks: list[PublicDocBlock] = []
    index = 0
    while index < len(lines):
        if not lines[index].lstrip().startswith("///"):
            index += 1
            continue
        start = index
        content: list[str] = []
        while index < len(lines) and lines[index].lstrip().startswith("///"):
            fragment = lines[index].lstrip()[3:]
            content.append(fragment[1:] if fragment.startswith(" ") else fragment)
            index += 1

        attributes, item = _following_item(lines, index)
        item_is_function = re.search(r"\bfn\s+[A-Za-z_]", item) is not None
        item_is_type = re.search(r"\b(?:struct|enum)\s+[A-Za-z_]", item) is not None
        inside_pymethods = any(range_start < start < range_end for range_start, range_end in pymethod_ranges)
        inside_method_macro = any(range_start < start < range_end for range_start, range_end in macro_ranges)
        is_public_owner = (
            (inside_pymethods and item_is_function)
            or (inside_method_macro and bool(item))
            or ("#[pyfunction" in attributes and item_is_function)
            or ("#[pyclass" in attributes and item_is_type)
        )
        if is_public_owner:
            blocks.append(
                PublicDocBlock(
                    path=path,
                    line=start + 1,
                    owner=_owner_name(item),
                    text="\n".join(content),
                )
            )
    return blocks


def placeholder_errors(path: Path) -> list[PlaceholderError]:
    """Return placeholder diagnostics for one Rust binding source file."""
    errors: list[PlaceholderError] = []
    for block in public_pyo3_doc_blocks(path):
        for message, pattern in PLACEHOLDER_PATTERNS:
            if pattern.search(block.text) is not None:
                errors.append(
                    PlaceholderError(
                        path=block.path,
                        line=block.line,
                        owner=block.owner,
                        message=message,
                    )
                )
    return errors


def input_paths(requested: list[Path]) -> list[Path]:
    """Expand requested Rust files or directories, defaulting to PyO3 bindings."""
    roots = requested or [BINDINGS_ROOT]
    paths: list[Path] = []
    for root in roots:
        path = (REPO_ROOT / root).resolve() if not root.is_absolute() else root
        if path.is_dir():
            paths.extend(path.rglob("*.rs"))
        elif path.suffix == ".rs":
            paths.append(path)
        else:
            raise ValueError(f"not a Rust source file or directory: {root}")
    return sorted(set(paths))


def parse_args() -> argparse.Namespace:
    """Parse optional Rust source paths."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "paths",
        nargs="*",
        type=Path,
        help="Rust binding files/directories to check; defaults to the PyO3 binding tree.",
    )
    return parser.parse_args()


def main() -> int:
    """Check public PyO3 documentation for known placeholder patterns."""
    args = parse_args()
    try:
        paths = input_paths(args.paths)
    except ValueError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2

    blocks = [block for path in paths for block in public_pyo3_doc_blocks(path)]
    errors = [error for path in paths for error in placeholder_errors(path)]
    if not errors:
        print(f"PyO3 runtime documentation: clean ({len(blocks)} public doc blocks in {len(paths)} files)")
        return 0

    for error in errors:
        print(error.format(), file=sys.stderr)
    print(f"PyO3 runtime documentation: {len(errors)} placeholder(s) in {len(paths)} files", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())

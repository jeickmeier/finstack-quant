"""Enforce the INVARIANTS.md §7.2 deprecation-annotation contract.

Every ``#[deprecated(...)]`` attribute in workspace Rust library source must
state:

1. ``since = "X.Y.Z"`` — the release in which the deprecation began;
2. a replacement pointer or an explicit retention rationale in ``note``; and
3. the earliest planned removal release in ``note`` (e.g. ``removable in
   0.8.0`` or ``removed in 1.0.0``).

Bare ``#[deprecated]`` without arguments always fails. Test code, benches,
examples, and vendored trees are not scanned.

Run directly or via ``mise run rust-doc``.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
SCAN_ROOTS = [REPO_ROOT / "finstack-quant"]
EXCLUDE_PARTS = {"tests", "benches", "examples", "target", "fuzz"}

DEPRECATED_RE = re.compile(r"#\s*\[\s*deprecated(?P<args>\s*\(.*?\))?\s*\]", re.DOTALL)
SINCE_RE = re.compile(r'since\s*=\s*"\d+\.\d+\.\d+"')
NOTE_RE = re.compile(r'note\s*=\s*"(?P<note>(?:[^"\\]|\\.)*)"', re.DOTALL)
REMOVAL_RE = re.compile(r"remov(?:able|ed|al)\s+(?:in|at)\s+\d+\.\d+(?:\.\d+)?", re.IGNORECASE)
REPLACEMENT_RE = re.compile(r"(use\s+`|instead|replaced by|retained for)", re.IGNORECASE)


def is_excluded(path: Path) -> bool:
    """Return True when the file is outside the library-source contract."""
    return bool(EXCLUDE_PARTS.intersection(path.parts))


def check_file(path: Path) -> list[str]:
    """Return diagnostics for every non-conforming deprecation in one file."""
    text = path.read_text(encoding="utf-8")
    diagnostics: list[str] = []
    for match in DEPRECATED_RE.finditer(text):
        line = text.count("\n", 0, match.start()) + 1
        location = f"{path.relative_to(REPO_ROOT)}:{line}"
        args = match.group("args") or ""
        problems: list[str] = []
        if not SINCE_RE.search(args):
            problems.append('missing `since = "X.Y.Z"`')
        note_match = NOTE_RE.search(args)
        note = note_match.group("note") if note_match else ""
        if not note:
            problems.append("missing `note`")
        else:
            if not REMOVAL_RE.search(note):
                problems.append('note must state the earliest planned removal release (e.g. "removable in 0.8.0")')
            if not REPLACEMENT_RE.search(note):
                problems.append("note must point at the replacement API or state the retention rationale")
        if problems:
            diagnostics.append(f"{location}: #[deprecated] violates INVARIANTS.md §7.2: " + "; ".join(problems))
    return diagnostics


def main() -> int:
    """Scan workspace library sources and report deprecation-contract violations."""
    diagnostics: list[str] = []
    for root in SCAN_ROOTS:
        for path in sorted(root.rglob("*.rs")):
            if is_excluded(path.relative_to(REPO_ROOT)):
                continue
            diagnostics.extend(check_file(path))
    for diagnostic in diagnostics:
        print(diagnostic, file=sys.stderr)
    if diagnostics:
        print(f"\n{len(diagnostics)} deprecation-annotation violation(s).", file=sys.stderr)
        return 1
    print("check_deprecated_annotations: all #[deprecated] annotations conform.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

"""Enforce substantive JSDoc parameter documentation on WASM exports.

Rustdoc attached to ``#[wasm_bindgen]`` exports is copied into the generated
TypeScript declarations.  Every JavaScript-facing callable that accepts a
caller-supplied input must therefore document each input with a substantive
``@param`` entry in the source-of-truth Rust doc comment. The comment must
appear before its ``#[wasm_bindgen]`` attribute so wasm-bindgen can retain it.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path
import re
import sys

REPO_ROOT = Path(__file__).resolve().parents[1]
WASM_API_ROOT = REPO_ROOT / "finstack-quant-wasm" / "src" / "api"
GENERIC_DESCRIPTION_RE = re.compile(
    r"^(?:the )?(?:input|parameter|value)(?: (?:value|parameter|to use|for the operation))?[.!]?$",
    re.IGNORECASE,
)
DOC_COMMENT_RE = re.compile(r"^\s*/// ?(.*)$")
FUNCTION_RE = re.compile(r"^\s*pub\s+(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(")
IMPL_RE = re.compile(r"^\s*impl(?:<[^>]+>)?\s+[^;{]+\{")
WASM_BINDGEN_RE = re.compile(r"#\[wasm_bindgen(?:\([^]]*\))?\]")


@dataclass(frozen=True)
class DocumentationError:
    """A missing or insufficient JSDoc parameter entry on one WASM export."""

    path: Path
    line: int
    symbol: str
    message: str

    def format(self) -> str:
        """Render this diagnostic in compiler-style form."""
        return f"{self.path.relative_to(REPO_ROOT)}:{self.line}: {self.symbol}: {self.message}"


@dataclass(frozen=True)
class FunctionSignature:
    """A public function signature and its user-supplied input names."""

    name: str
    line: int
    parameters: list[str]
    docstring: str
    docstring_below_wasm_bindgen: bool


def strip_rust_comments(line: str) -> str:
    """Remove line comments before counting syntactic braces."""
    return line.split("//", maxsplit=1)[0]


def split_top_level(value: str) -> list[str]:
    """Split comma-separated Rust syntax while preserving nested types."""
    parts: list[str] = []
    start = 0
    depth = 0
    pairs = {"(": ")", "[": "]", "{": "}", "<": ">"}
    for index, character in enumerate(value):
        if character in pairs:
            depth += 1
        elif character in pairs.values():
            depth -= 1
        elif character == "," and depth == 0:
            parts.append(value[start:index])
            start = index + 1
    parts.append(value[start:])
    return parts


def user_parameters(signature: str) -> list[str]:
    """Return explicit caller inputs from a complete Rust function signature."""
    opening = signature.find("(")
    closing = signature.rfind(")")
    if opening == -1 or closing == -1:
        return []

    parameters: list[str] = []
    for raw_parameter in split_top_level(signature[opening + 1 : closing]):
        parameter = raw_parameter.strip()
        if not parameter or parameter in {"self", "&self", "&mut self", "mut self"}:
            continue
        name = parameter.split(":", maxsplit=1)[0].strip().removeprefix("mut ").strip()
        if re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", name) and name != "_":
            parameters.append(name)
    return parameters


def docstring_before(lines: list[str], index: int) -> str:
    """Collect Rustdoc immediately attached to an attributed public function."""
    fragments: list[str] = []
    cursor = index - 1
    attribute_depth = 0
    while cursor >= 0:
        line = lines[cursor]
        stripped = line.strip()
        match = DOC_COMMENT_RE.match(line)
        if match is not None:
            fragments.append(match.group(1))
            cursor -= 1
            continue
        if attribute_depth:
            attribute_depth += stripped.count(")") - stripped.count("(")
            cursor -= 1
            continue
        if stripped.endswith(")]"):
            attribute_depth = stripped.count(")") - stripped.count("(")
            cursor -= 1
            continue
        if not stripped or stripped.startswith("#"):
            cursor -= 1
            continue
        break
    return "\n".join(reversed(fragments)).strip()


def docstring_below_wasm_bindgen(lines: list[str], index: int) -> bool:
    """Return whether function Rustdoc sits between its binding attribute and body."""
    cursor = index - 1
    attribute_depth = 0
    saw_docstring = False
    while cursor >= 0:
        line = lines[cursor]
        stripped = line.strip()
        if DOC_COMMENT_RE.match(line):
            saw_docstring = True
            cursor -= 1
            continue
        if attribute_depth:
            attribute_depth += stripped.count(")") - stripped.count("(")
            cursor -= 1
            continue
        if stripped.endswith(")]") and not stripped.startswith("#["):
            attribute_depth = 1
            cursor -= 1
            continue
        if WASM_BINDGEN_RE.search(line):
            return saw_docstring
        if not stripped or stripped.startswith("#"):
            cursor -= 1
            continue
        break
    return False


def parameter_description(docstring: str, parameter: str) -> str | None:
    """Return one JSDoc or Rustdoc argument description for an export input."""
    camel_case = re.sub(r"_([a-zA-Z])", lambda match: match.group(1).upper(), parameter)
    names = "|".join(re.escape(name) for name in {parameter, camel_case})
    jsdoc_header = re.compile(rf"^\s*@param(?:\s+\{{[^}}]+\}})?\s+(?:{names})\s*-\s*(.+?)\s*$")
    rustdoc_header = re.compile(rf"^\s*[*-]\s+`?(?:{names})`?\s*-\s*(.+?)\s*$")
    lines = docstring.splitlines()
    for index, line in enumerate(lines):
        match = jsdoc_header.match(line) or rustdoc_header.match(line)
        if match is None:
            continue
        fragments = [match.group(1)]
        for continuation in lines[index + 1 :]:
            stripped = continuation.strip()
            if not stripped:
                continue
            if stripped.startswith(("* ", "- ", "@", "#")):
                break
            fragments.append(stripped)
        return " ".join(fragments)
    return None


def is_substantive(description: str) -> bool:
    """Reject tautological and implausibly short input descriptions."""
    plain_text = re.sub(r"[`*_\[\]()]", "", description).strip()
    return len(plain_text) >= 16 and GENERIC_DESCRIPTION_RE.fullmatch(plain_text) is None


def has_summary(docstring: str) -> bool:
    """Require a reader-facing summary before JSDoc tags or Rustdoc headings."""
    for line in docstring.splitlines():
        text = line.strip()
        if not text:
            continue
        return not text.startswith(("@", "#")) and len(text) >= 16
    return False


def function_signature(lines: list[str], index: int) -> str:
    """Return the complete signature beginning at a ``pub fn`` line."""
    signature_lines: list[str] = []
    depth = 0
    seen_opening = False
    for line in lines[index:]:
        signature_lines.append(line.strip())
        for character in strip_rust_comments(line):
            if character == "(":
                depth += 1
                seen_opening = True
            elif character == ")":
                depth -= 1
        if seen_opening and depth == 0 and ("{" in line or "where" in line or "->" in line):
            return " ".join(signature_lines)
    return " ".join(signature_lines)


def exported_functions(path: Path) -> list[FunctionSignature]:
    """Find direct and ``#[wasm_bindgen] impl`` public exports in one file."""
    lines = path.read_text(encoding="utf-8").splitlines()
    functions: list[FunctionSignature] = []
    brace_depth = 0
    pending_wasm_bindgen = False
    wasm_impl_depths: list[int] = []

    for index, line in enumerate(lines):
        if WASM_BINDGEN_RE.search(line):
            pending_wasm_bindgen = True

        is_impl = IMPL_RE.match(line) is not None
        line_braces = strip_rust_comments(line)
        opening_braces = line_braces.count("{")
        closing_braces = line_braces.count("}")
        if is_impl:
            if pending_wasm_bindgen:
                wasm_impl_depths.append(brace_depth + opening_braces)
            pending_wasm_bindgen = False

        match = FUNCTION_RE.match(line)
        directly_exported = pending_wasm_bindgen
        if match is not None and (directly_exported or wasm_impl_depths):
            signature = function_signature(lines, index)
            functions.append(
                FunctionSignature(
                    name=match.group(1),
                    line=index + 1,
                    parameters=user_parameters(signature),
                    docstring=docstring_before(lines, index),
                    docstring_below_wasm_bindgen=docstring_below_wasm_bindgen(lines, index),
                )
            )
        if match is not None:
            pending_wasm_bindgen = False

        brace_depth += opening_braces - closing_braces
        while wasm_impl_depths and brace_depth < wasm_impl_depths[-1]:
            wasm_impl_depths.pop()

    return functions


def public_callable_errors(path: Path) -> list[DocumentationError]:
    """Return documentation diagnostics for one WASM API source file."""
    errors: list[DocumentationError] = []
    for function in exported_functions(path):
        if function.docstring_below_wasm_bindgen:
            errors.append(
                DocumentationError(
                    path,
                    function.line,
                    function.name,
                    "Rustdoc must appear before #[wasm_bindgen] so wasm-bindgen retains it",
                )
            )
        if not function.parameters:
            continue
        symbol = function.name
        if not function.docstring:
            errors.append(DocumentationError(path, function.line, symbol, "missing Rustdoc for callable inputs"))
            continue
        if not has_summary(function.docstring):
            errors.append(DocumentationError(path, function.line, symbol, "missing substantive callable summary"))
        for parameter in function.parameters:
            description = parameter_description(function.docstring, parameter)
            if description is None:
                errors.append(
                    DocumentationError(path, function.line, symbol, f"missing documentation for `{parameter}`")
                )
            elif not is_substantive(description):
                errors.append(
                    DocumentationError(
                        path,
                        function.line,
                        symbol,
                        f"documentation for `{parameter}` is not substantive",
                    )
                )
    return errors


def parse_args() -> argparse.Namespace:
    """Parse optional WASM Rust paths and diagnostic limits."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "paths",
        nargs="*",
        type=Path,
        help="Rust files or directories to check; defaults to the WASM API source tree.",
    )
    parser.add_argument(
        "--max-errors",
        type=int,
        default=200,
        help="Maximum diagnostics to print before the summary (default: 200).",
    )
    return parser.parse_args()


def input_paths(requested: list[Path]) -> list[Path]:
    """Expand requested Rust files/directories or the full WASM API tree."""
    roots = requested or [WASM_API_ROOT]
    paths: list[Path] = []
    for root in roots:
        path = (REPO_ROOT / root).resolve() if not root.is_absolute() else root
        if path.is_dir():
            paths.extend(path.rglob("*.rs"))
        elif path.suffix == ".rs":
            paths.append(path)
        else:
            raise ValueError(f"not a WASM Rust source file or directory: {root}")
    return sorted(set(paths))


def main() -> int:
    """Check every JavaScript-facing WASM callable for input documentation."""
    args = parse_args()
    try:
        paths = input_paths(args.paths)
    except ValueError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    errors = [error for path in paths for error in public_callable_errors(path)]
    if not errors:
        print(f"WASM input documentation: clean ({len(paths)} files)")
        return 0

    for error in errors[: args.max_errors]:
        print(error.format(), file=sys.stderr)
    if len(errors) > args.max_errors:
        print(f"... {len(errors) - args.max_errors} additional documentation errors omitted", file=sys.stderr)
    print(f"WASM input documentation: {len(errors)} error(s) in {len(paths)} files", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())

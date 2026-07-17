"""Enforce substantive input documentation on Python-facing callables.

The Python stubs are the primary IDE surface for compiled bindings, while pure
Python helpers have no generated alternative. Every public function, method,
factory, and constructor that accepts caller input therefore needs a docstring
with a substantive entry for each input. NumPy-style and Google-style
parameter sections are supported.
"""

from __future__ import annotations

import argparse
import ast
from pathlib import Path
import re
import sys

REPO_ROOT = Path(__file__).resolve().parents[1]
PACKAGE_ROOT = REPO_ROOT / "finstack-quant-py" / "finstack_quant"
GENERIC_DESCRIPTION_RE = re.compile(
    r"^(?:the )?(?:input|parameter|value)(?: (?:value|parameter|to use|for the operation))?[.!]?$",
    re.IGNORECASE,
)


class DocumentationError:
    """A missing or insufficient Python parameter-documentation entry."""

    def __init__(self, path: Path, line: int, symbol: str, message: str) -> None:
        """Create one diagnostic bound to a public callable."""
        self.path = path
        self.line = line
        self.symbol = symbol
        self.message = message

    def format(self) -> str:
        """Render the diagnostic in compiler-style form."""
        return f"{self.path.relative_to(REPO_ROOT)}:{self.line}: {self.symbol}: {self.message}"


def callable_arguments(node: ast.FunctionDef | ast.AsyncFunctionDef) -> list[str]:
    """Return user-supplied argument names from one Python callable."""
    arguments = node.args
    names = [argument.arg for argument in arguments.posonlyargs + arguments.args + arguments.kwonlyargs]
    names = [name for name in names if name not in {"self", "cls"}]
    if arguments.vararg is not None:
        names.append(arguments.vararg.arg)
    if arguments.kwarg is not None:
        names.append(arguments.kwarg.arg)
    return names


def is_public_callable(node: ast.FunctionDef | ast.AsyncFunctionDef) -> bool:
    """Return whether one named callable is a documented public API entry."""
    return node.name == "__init__" or not node.name.startswith("_")


def parameter_description(docstring: str, parameter: str) -> str | None:
    """Extract one NumPy- or Google-style parameter description."""
    escaped = re.escape(parameter)
    lines = docstring.splitlines()
    header = re.compile(rf"^\s*(?:[-*]\s+)?`?{escaped}`?(?:\s*\([^)]*\))?\s*(?::|[-—])\s*(.*)$")
    next_header = re.compile(
        r"^\s*(?:[-*]\s+`?[A-Za-z_][A-Za-z0-9_]*`?\s*(?:[-—:]\s+)|"
        r"`?[A-Za-z_][A-Za-z0-9_]*`?(?:\s*\([^)]*\))?\s*:)"
    )

    for index, line in enumerate(lines):
        match = header.match(line)
        if match is None:
            continue
        fragments = [match.group(1)]
        header_indent = len(line) - len(line.lstrip())
        for continuation in lines[index + 1 :]:
            stripped = continuation.strip()
            if not stripped:
                continue
            continuation_indent = len(continuation) - len(continuation.lstrip())
            if (continuation_indent <= header_indent and next_header.match(continuation)) or stripped in {
                "Returns",
                "Raises",
                "Notes",
                "Examples",
                "Attributes",
                "Warnings",
            }:
                break
            if set(stripped) == {"-"}:
                continue
            fragments.append(stripped)
        description = " ".join(fragment for fragment in fragments if fragment).strip()
        if description:
            return description
    return None


def is_substantive(description: str) -> bool:
    """Reject empty, tautological, and implausibly short descriptions."""
    plain_text = re.sub(r"[`*_\[\]()]", "", description).strip()
    return len(plain_text) >= 16 and GENERIC_DESCRIPTION_RE.fullmatch(plain_text) is None


class PublicCallableVisitor:
    """Collect documentation errors without descending into private helpers."""

    def __init__(self, path: Path) -> None:
        """Prepare to inspect one stub or pure-Python module."""
        self.path = path
        self.errors: list[DocumentationError] = []
        self.scope: list[str] = []

    def inspect_body(self, body: list[ast.stmt], class_docstring: str | None = None) -> None:
        """Inspect module and class members, excluding nested implementation helpers."""
        for node in body:
            if isinstance(node, ast.ClassDef):
                if not node.name.startswith("_"):
                    self.scope.append(node.name)
                    self.inspect_body(node.body, ast.get_docstring(node))
                    self.scope.pop()
            elif isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and is_public_callable(node):
                self.inspect_callable(node, class_docstring)

    def inspect_callable(
        self,
        node: ast.FunctionDef | ast.AsyncFunctionDef,
        class_docstring: str | None,
    ) -> None:
        """Check one public callable that accepts one or more user inputs."""
        parameters = callable_arguments(node)
        if not parameters:
            return

        symbol_name = "constructor" if node.name == "__init__" else node.name
        symbol = ".".join([*self.scope, symbol_name])
        docstring = ast.get_docstring(node)
        if docstring is None and node.name == "__init__":
            docstring = class_docstring
        if docstring is None:
            self.errors.append(
                DocumentationError(self.path, node.lineno, symbol, "missing docstring for callable inputs")
            )
            return

        for parameter in parameters:
            description = parameter_description(docstring, parameter)
            if description is None:
                self.errors.append(
                    DocumentationError(
                        self.path,
                        node.lineno,
                        symbol,
                        f"missing documentation for `{parameter}`",
                    )
                )
            elif not is_substantive(description):
                self.errors.append(
                    DocumentationError(
                        self.path,
                        node.lineno,
                        symbol,
                        f"documentation for `{parameter}` is not substantive",
                    )
                )


def public_callable_errors(path: Path) -> list[DocumentationError]:
    """Return missing input-documentation diagnostics for one Python API file."""
    visitor = PublicCallableVisitor(path)
    visitor.inspect_body(ast.parse(path.read_text(encoding="utf-8"), filename=str(path)).body)
    return visitor.errors


def parse_args() -> argparse.Namespace:
    """Parse optional Python API paths and diagnostic limits."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "paths",
        nargs="*",
        type=Path,
        help="Stub or Python files/directories to check; defaults to the binding package.",
    )
    parser.add_argument(
        "--max-errors",
        type=int,
        default=200,
        help="Maximum diagnostics to print before the summary (default: 200).",
    )
    return parser.parse_args()


def input_paths(requested: list[Path]) -> list[Path]:
    """Expand requested files/directories or the full Python binding package."""
    roots = requested or [PACKAGE_ROOT]
    paths: list[Path] = []
    for root in roots:
        path = (REPO_ROOT / root).resolve() if not root.is_absolute() else root
        if path.is_dir():
            paths.extend([*path.rglob("*.pyi"), *path.rglob("*.py")])
        elif path.suffix in {".py", ".pyi"}:
            paths.append(path)
        else:
            raise ValueError(f"not a Python API file or directory: {root}")
    return sorted(set(paths))


def main() -> int:
    """Check every Python stub and pure-Python public module."""
    args = parse_args()
    try:
        paths = input_paths(args.paths)
    except ValueError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    errors = [error for path in paths for error in public_callable_errors(path)]
    if not errors:
        print(f"Python input documentation: clean ({len(paths)} files)")
        return 0

    for error in errors[: args.max_errors]:
        print(error.format(), file=sys.stderr)
    if len(errors) > args.max_errors:
        print(f"... {len(errors) - args.max_errors} additional documentation errors omitted", file=sys.stderr)
    print(f"Python input documentation: {len(errors)} error(s) in {len(paths)} files", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())

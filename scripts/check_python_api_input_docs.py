"""Enforce complete IntelliSense documentation on Python-facing callables.

The Python stubs are the primary IDE surface for compiled bindings, while pure
Python helpers have no generated alternative. Every public class and callable
therefore needs a substantive summary; callable inputs, outputs, errors, and
usage examples must be documented where they are useful to a Python user.
NumPy-style and Google-style sections are supported.
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
SECTION_NAMES = frozenset({"Parameters", "Args", "Returns", "Raises", "Examples", "Notes", "Warnings"})


class DocumentationError:
    """A missing or insufficient Python API-documentation entry."""

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


def is_section_heading(lines: list[str], index: int) -> bool:
    """Return whether one docstring line starts a supported section."""
    text = lines[index].strip().rstrip(":")
    if text in SECTION_NAMES:
        return True
    if index + 1 >= len(lines) or text not in SECTION_NAMES:
        return False
    underline = lines[index + 1].strip()
    return len(underline) >= 3 and set(underline) == {"-"}


def section_description(docstring: str, section_names: frozenset[str]) -> str | None:
    """Return the substantive content from one NumPy- or Google-style section."""
    lines = docstring.splitlines()
    for index, line in enumerate(lines):
        if line.strip().rstrip(":") not in section_names:
            continue
        fragments: list[str] = []
        for continuation_index, continuation in enumerate(lines[index + 1 :], start=index + 1):
            stripped = continuation.strip()
            if continuation_index != index + 1 and is_section_heading(lines, continuation_index):
                break
            if not stripped or set(stripped) == {"-"}:
                continue
            fragments.append(stripped)
        description = " ".join(fragments).strip()
        if description:
            return description
    return None


def has_summary(docstring: str) -> bool:
    """Return whether a docstring starts with a reader-facing summary."""
    for line in docstring.splitlines():
        text = line.strip()
        if not text:
            continue
        return text.rstrip(":") not in SECTION_NAMES and len(text) >= 16
    return False


def has_example(docstring: str) -> bool:
    """Return whether a docstring has a doctest-style usage example."""
    examples = section_description(docstring, frozenset({"Examples"}))
    return examples is not None and ">>>" in examples


def returns_value(node: ast.FunctionDef | ast.AsyncFunctionDef) -> bool:
    """Return whether a callable's annotation indicates a user-visible result."""
    if node.name == "__init__":
        return False
    annotation = node.returns
    if annotation is None:
        return True
    if isinstance(annotation, ast.Constant) and annotation.value is None:
        return False
    return not (isinstance(annotation, ast.Name) and annotation.id == "None")


def is_class_or_static_method(node: ast.FunctionDef | ast.AsyncFunctionDef) -> bool:
    """Return whether a callable is a public class-level API entry point."""
    return any(
        isinstance(decorator, ast.Name) and decorator.id in {"classmethod", "staticmethod"}
        for decorator in node.decorator_list
    )


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
    """Collect complete API-documentation errors without visiting private helpers."""

    def __init__(self, path: Path) -> None:
        """Prepare to inspect one stub or pure-Python module."""
        self.path = path
        self.errors: list[DocumentationError] = []
        self.scope: list[str] = []

    def inspect_module(self, tree: ast.Module) -> None:
        """Inspect module-level docs and every public class or callable."""
        docstring = ast.get_docstring(tree)
        if docstring is None:
            self.errors.append(DocumentationError(self.path, 1, "module", "missing module docstring"))
        elif not has_summary(docstring):
            self.errors.append(DocumentationError(self.path, 1, "module", "missing substantive module summary"))
        elif not has_example(docstring):
            self.errors.append(DocumentationError(self.path, 1, "module", "missing module usage example"))
        self.inspect_body(tree.body)

    def inspect_body(self, body: list[ast.stmt], class_docstring: str | None = None) -> None:
        """Inspect module and class members, excluding nested implementation helpers."""
        for node in body:
            if isinstance(node, ast.ClassDef):
                if not node.name.startswith("_"):
                    self.scope.append(node.name)
                    self.inspect_class(node)
                    self.scope.pop()
            elif isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and is_public_callable(node):
                self.inspect_callable(node, class_docstring)

    def inspect_class(self, node: ast.ClassDef) -> None:
        """Check a public class as the shared usage entry point for its methods."""
        docstring = ast.get_docstring(node)
        symbol = ".".join(self.scope)
        if docstring is None:
            self.errors.append(DocumentationError(self.path, node.lineno, symbol, "missing class docstring"))
        elif not has_summary(docstring):
            self.errors.append(DocumentationError(self.path, node.lineno, symbol, "missing substantive class summary"))
        elif not has_example(docstring):
            self.errors.append(DocumentationError(self.path, node.lineno, symbol, "missing class usage example"))
        self.inspect_body(node.body, docstring)

    def inspect_callable(
        self,
        node: ast.FunctionDef | ast.AsyncFunctionDef,
        class_docstring: str | None,
    ) -> None:
        """Check one public callable's complete IntelliSense documentation."""
        parameters = callable_arguments(node)
        symbol_name = "constructor" if node.name == "__init__" else node.name
        symbol = ".".join([*self.scope, symbol_name])
        docstring = ast.get_docstring(node)
        if docstring is None and node.name == "__init__":
            docstring = class_docstring
        if docstring is None:
            self.errors.append(DocumentationError(self.path, node.lineno, symbol, "missing callable docstring"))
            return

        if not has_summary(docstring):
            self.errors.append(
                DocumentationError(self.path, node.lineno, symbol, "missing substantive callable summary")
            )

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

        if returns_value(node):
            description = section_description(docstring, frozenset({"Returns"}))
            if description is None:
                self.errors.append(DocumentationError(self.path, node.lineno, symbol, "missing Returns section"))
            elif not is_substantive(description):
                self.errors.append(
                    DocumentationError(self.path, node.lineno, symbol, "Returns section is not substantive")
                )

        if parameters:
            description = section_description(docstring, frozenset({"Raises"}))
            if description is None:
                self.errors.append(DocumentationError(self.path, node.lineno, symbol, "missing Raises section"))
            elif not is_substantive(description):
                self.errors.append(
                    DocumentationError(self.path, node.lineno, symbol, "Raises section is not substantive")
                )

        if (not self.scope or is_class_or_static_method(node)) and not has_example(docstring):
            self.errors.append(DocumentationError(self.path, node.lineno, symbol, "missing callable usage example"))


def public_callable_errors(path: Path) -> list[DocumentationError]:
    """Return complete API-documentation diagnostics for one Python API file."""
    visitor = PublicCallableVisitor(path)
    visitor.inspect_module(ast.parse(path.read_text(encoding="utf-8"), filename=str(path)))
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
        print(f"Python API documentation: clean ({len(paths)} files)")
        return 0

    for error in errors[: args.max_errors]:
        print(error.format(), file=sys.stderr)
    if len(errors) > args.max_errors:
        print(f"... {len(errors) - args.max_errors} additional documentation errors omitted", file=sys.stderr)
    print(f"Python API documentation: {len(errors)} error(s) in {len(paths)} files", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())

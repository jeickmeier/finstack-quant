"""Execute Python stub and module doctest examples against the live package.

Stub ``.pyi`` files are not importable implementations. This runner extracts
``>>>`` examples from public docstrings and executes them against the compiled
``finstack_quant`` package so examples stay accurate.
"""

from __future__ import annotations

import argparse
import ast
import doctest
import importlib
from pathlib import Path
import sys

REPO_ROOT = Path(__file__).resolve().parents[1]
PACKAGE_ROOT = REPO_ROOT / "finstack-quant-py" / "finstack_quant"
OPTIONFLAGS = doctest.ELLIPSIS | doctest.NORMALIZE_WHITESPACE


def module_name_for(path: Path) -> str:
    """Return the import name for one stub or pure-Python API file."""
    rel = path.relative_to(PACKAGE_ROOT)
    parts = list(rel.with_suffix("").parts)
    if parts[-1] == "__init__":
        parts = parts[:-1]
    private = next((i for i, part in enumerate(parts) if part.startswith("_")), None)
    if private is not None:
        # Private stub fragments (`_foo.pyi`, `_dir/foo.pyi`) declare members of
        # the public module above them; they are not runtime submodules.
        parts = parts[:private]
    return ".".join(["finstack_quant", *parts])


def docstring_owners(path: Path) -> list[tuple[str, int, str]]:
    """Return ``(symbol, lineno, docstring)`` for public documented owners."""
    source = path.read_text(encoding="utf-8")
    tree = ast.parse(source, filename=str(path))
    owners: list[tuple[str, int, str]] = []

    module_doc = ast.get_docstring(tree)
    if module_doc:
        owners.append((f"{module_name_for(path)} (module)", 1, module_doc))

    def visit(nodes: list[ast.stmt], scope: list[str]) -> None:
        for node in nodes:
            if isinstance(node, ast.ClassDef) and not node.name.startswith("_"):
                doc = ast.get_docstring(node)
                if doc:
                    owners.append((".".join([*scope, node.name]), node.lineno, doc))
                visit(node.body, [*scope, node.name])
            elif isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and (
                node.name == "__init__" or not node.name.startswith("_")
            ):
                doc = ast.get_docstring(node)
                if doc and ">>>" in doc:
                    name = "constructor" if node.name == "__init__" else node.name
                    owners.append((".".join([*scope, name]), node.lineno, doc))

    visit(tree.body, [])
    return owners


def api_paths() -> list[Path]:
    """Return every Python stub and pure-Python public module."""
    return sorted([*PACKAGE_ROOT.rglob("*.pyi"), *PACKAGE_ROOT.rglob("*.py")])


def run_file(path: Path) -> list[str]:
    """Execute doctest examples from one API file. Return failure messages."""
    module_name = module_name_for(path)
    try:
        module = importlib.import_module(module_name)
    except Exception as error:
        return [f"{path.relative_to(REPO_ROOT)}: failed to import {module_name}: {error}"]

    parser = doctest.DocTestParser()
    captured: list[str] = []
    runner = doctest.DocTestRunner(optionflags=OPTIONFLAGS, verbose=False)
    failures: list[str] = []
    for symbol, lineno, docstring in docstring_owners(path):
        globs = dict(vars(module))
        test = parser.get_doctest(docstring, globs, symbol, str(path), lineno)
        if not test.examples:
            continue
        before = runner.tries, runner.failures
        runner.run(test, out=captured.append, clear_globs=False)
        failed = runner.failures - before[1]
        if failed:
            detail = "".join(captured).strip()
            failures.append(
                f"{path.relative_to(REPO_ROOT)}:{lineno}: {symbol}: "
                f"{failed} of {runner.tries - before[0]} example(s) failed\n{detail}"
            )
        captured.clear()
    return failures


def parse_args() -> argparse.Namespace:
    """Parse optional API paths and a failure cap."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "paths",
        nargs="*",
        type=Path,
        help="Stub or Python files to test; defaults to the binding package.",
    )
    parser.add_argument(
        "--max-failures",
        type=int,
        default=50,
        help="Maximum failure lines to print (default: 50).",
    )
    return parser.parse_args()


def main() -> int:
    """Run stub and module doctest examples against the live package."""
    args = parse_args()
    paths = [path.resolve() for path in args.paths] if args.paths else api_paths()
    failures = [message for path in paths for message in run_file(path)]
    attempted_files = len(paths)
    if not failures:
        print(f"Python stub doctests: clean ({attempted_files} files)")
        return 0
    for message in failures[: args.max_failures]:
        print(message, file=sys.stderr)
    if len(failures) > args.max_failures:
        print(f"... {len(failures) - args.max_failures} additional failures omitted", file=sys.stderr)
    print(f"Python stub doctests: {len(failures)} failing owner(s) in {attempted_files} files", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())

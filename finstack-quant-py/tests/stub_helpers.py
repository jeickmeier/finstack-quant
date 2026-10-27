"""Read a public module's stub together with the private fragments it re-exports.

A public module's ``__init__.pyi`` re-exports declarations from private
fragments (``_foo.pyi``, ``_dir/foo.pyi``) that mirror the Rust binding files.
Tests that inspect stub declarations go through these helpers instead of
reading ``__init__.pyi`` alone.
"""

from __future__ import annotations

import ast
from pathlib import Path

import finstack_quant

PACKAGE_DIR = Path(finstack_quant.__file__).parent


def stub_path(module: str) -> Path:
    """Return the stub file declaring ``module`` (``finstack_quant.<dotted path>``)."""
    base = PACKAGE_DIR.joinpath(*module.split(".")[1:])
    package = base / "__init__.pyi"
    return package if package.exists() else base.with_suffix(".pyi")


def _fragment_imports(module: str, tree: ast.Module) -> list[ast.ImportFrom]:
    return [
        node
        for node in tree.body
        if isinstance(node, ast.ImportFrom)
        and node.module is not None
        and node.module.startswith(module + ".")
        and node.module[len(module) + 1 :].startswith("_")
    ]


def stub_sources(module: str) -> list[Path]:
    """Return the module stub followed by every private fragment it re-exports from."""
    path = stub_path(module)
    tree = ast.parse(path.read_text(encoding="utf-8"))
    paths = [path]
    for node in _fragment_imports(module, tree):
        fragment = stub_path(node.module)
        if fragment not in paths:
            paths.append(fragment)
    return paths


def stub_text(module: str) -> str:
    """Return the concatenated source of the module stub and its fragments."""
    return "\n".join(path.read_text(encoding="utf-8") for path in stub_sources(module))


def stub_definitions(module: str) -> list[ast.stmt]:
    """Return the module's top-level statements with re-exported fragment definitions inlined.

    A ``from <module>._frag import Name as Name`` re-export is replaced by the
    class or function definitions of ``Name`` in that fragment.
    """
    tree = ast.parse(stub_path(module).read_text(encoding="utf-8"))
    reexports = {id(node): node for node in _fragment_imports(module, tree)}
    out: list[ast.stmt] = []
    for node in tree.body:
        imported = reexports.get(id(node))
        if imported is None:
            out.append(node)
            continue
        assert imported.module is not None
        names = {alias.name for alias in imported.names}
        out.extend(
            definition
            for definition in stub_definitions(imported.module)
            if isinstance(definition, (ast.ClassDef, ast.FunctionDef, ast.AsyncFunctionDef))
            and definition.name in names
        )
    return out

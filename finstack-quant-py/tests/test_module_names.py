"""Every public module is named, packaged and registered at its import path.

The compiled extension is installed as ``finstack_quant.finstack_quant`` but its
submodules belong to the public ``finstack_quant`` tree. A module reached as
``finstack_quant.models.credit.lgd`` must report exactly that ``__name__``, be
``sys.modules["finstack_quant.models.credit.lgd"]``, and never appear under the
extension's internal ``finstack_quant.finstack_quant.`` prefix; public functions
must name a module that actually exports them.
"""

from __future__ import annotations

import importlib
import importlib.util
import inspect
from pathlib import Path
import subprocess
import sys
from types import ModuleType

import pytest

import finstack_quant

ROOT = "finstack_quant"
EXTENSION = "finstack_quant.finstack_quant"
PACKAGE_DIR = Path(finstack_quant.__file__).parent


def _public_modules() -> dict[str, ModuleType]:
    """Map every module reachable from ``finstack_quant`` through ``__all__``."""
    found: dict[str, ModuleType] = {}
    pending: list[tuple[str, ModuleType]] = [(ROOT, finstack_quant)]
    while pending:
        path, module = pending.pop()
        if path in found:
            continue
        found[path] = module
        for name in getattr(module, "__all__", ()):
            child = getattr(module, name)
            if isinstance(child, ModuleType):
                pending.append((f"{path}.{name}", child))
    return found


PUBLIC_MODULES = _public_modules()


def _python_files() -> dict[str, Path]:
    """Map the import path of every pure-Python package and module on disk."""
    files: dict[str, Path] = {}
    for file in sorted(PACKAGE_DIR.rglob("*.py")):
        parts = file.relative_to(PACKAGE_DIR).with_suffix("").parts
        if parts[-1] == "__init__":
            parts = parts[:-1]
        if any(part.startswith("_") for part in parts):
            continue
        files[".".join((ROOT, *parts))] = file
    return files


def test_walk_reaches_the_compiled_leaves() -> None:
    assert len(PUBLIC_MODULES) > 60
    for path in ("finstack_quant.models.credit.lgd", "finstack_quant.cashflows.fixings", "finstack_quant.core.schema"):
        assert path in PUBLIC_MODULES


@pytest.mark.parametrize("path", sorted(PUBLIC_MODULES))
def test_module_is_named_and_registered_at_its_import_path(path: str) -> None:
    module = PUBLIC_MODULES[path]
    assert module.__name__ == path
    # Packages and compiled modules carry their own path; a plain ``.py`` module
    # carries its parent package's.
    is_plain_python_module = getattr(module, "__file__", None) is not None and not hasattr(module, "__path__")
    assert module.__package__ == (path.rpartition(".")[0] if is_plain_python_module else path)
    assert sys.modules[path] is module
    assert importlib.import_module(path) is module
    spec = importlib.util.find_spec(path)
    assert spec is not None
    assert spec.name == path
    assert spec.parent == module.__package__


@pytest.mark.parametrize(("path", "file"), sorted(_python_files().items()))
def test_python_package_owns_its_import_path(path: str, file: Path) -> None:
    """A compiled module must never displace the Python file at the same path."""
    module = importlib.import_module(path)
    assert module.__file__ is not None
    assert Path(module.__file__).resolve() == file.resolve()


def test_nothing_is_registered_under_the_extension_path() -> None:
    internal = sorted(key for key in sys.modules if key.startswith(f"{EXTENSION}."))
    assert internal == []


def test_public_functions_and_classes_name_a_module_that_exports_them() -> None:
    """``__module__`` + ``__name__`` must resolve back to the object (as pickle does)."""
    unresolved = []
    for path, module in sorted(PUBLIC_MODULES.items()):
        for name in getattr(module, "__all__", ()):
            member = getattr(module, name)
            if not (inspect.isroutine(member) or inspect.isclass(member)):
                continue
            owner = sys.modules.get(member.__module__) or importlib.import_module(member.__module__)
            if getattr(owner, member.__name__, None) is not member:
                unresolved.append(f"{path}.{name}: __module__={member.__module__!r}")
    assert unresolved == []


@pytest.mark.parametrize(
    "first_import",
    [
        "import finstack_quant.models.credit.lgd",
        "import finstack_quant.cashflows.fixings",
        "import finstack_quant.schema",
        "from finstack_quant import schema",
        "from finstack_quant.models.credit import pd",
        "import finstack_quant",
    ],
)
def test_names_do_not_depend_on_import_order(first_import: str) -> None:
    """A fresh interpreter sees the same names whichever module it imports first."""
    code = f"""
{first_import}
import importlib, sys
from types import ModuleType
import finstack_quant.models.credit.lgd as lgd
assert lgd.__name__ == "finstack_quant.models.credit.lgd", lgd.__name__
assert lgd.__package__ == "finstack_quant.models.credit.lgd", lgd.__package__
pending, seen = [("finstack_quant", importlib.import_module("finstack_quant"))], set()
while pending:
    path, module = pending.pop()
    if path in seen:
        continue
    seen.add(path)
    assert module.__name__ == path, (path, module.__name__)
    assert sys.modules[path] is module, path
    for name in getattr(module, "__all__", ()):
        child = getattr(module, name)
        if isinstance(child, ModuleType):
            pending.append((f"{{path}}.{{name}}", child))
internal = [key for key in sys.modules if key.startswith("finstack_quant.finstack_quant.")]
assert not internal, internal
"""
    result = subprocess.run([sys.executable, "-c", code], capture_output=True, text=True, check=False)  # noqa: S603
    assert result.returncode == 0, result.stderr

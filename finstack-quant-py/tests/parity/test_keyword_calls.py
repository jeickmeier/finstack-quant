"""Every public callable must accept its documented parameter names as keywords.

PyO3 builds the runtime ``inspect.signature`` from ``text_signature``, so a stub/AST
comparison cannot detect a Rust parameter whose name differs from the documented one
(e.g. ``month_number`` documented as ``month``, or the Python keyword ``from``).
The only reliable check is to *call* each callable with each documented keyword
and assert the failure, if any, is not "unexpected keyword argument".
"""

from __future__ import annotations

import ast
import importlib
import inspect
from pathlib import Path
from types import ModuleType
from typing import Any

import pytest

import finstack_quant as fq

PKG_ROOT = Path(fq.__file__).resolve().parent
UNEXPECTED = "unexpected keyword argument"


class _Sentinel:
    """Deliberately wrong-typed argument; never a valid input."""


def _iter_stub_files() -> list[Path]:
    return sorted(p for p in PKG_ROOT.rglob("*.pyi") if "reporting" not in p.parts)


def _module_name_for(stub: Path) -> str:
    rel = stub.relative_to(PKG_ROOT.parent).with_suffix("")
    parts = list(rel.parts)
    if parts[-1] == "__init__":
        parts = parts[:-1]
    return ".".join(parts)


def _params(fn: ast.FunctionDef | ast.AsyncFunctionDef, *, drop_self: bool) -> list[str]:
    names = [a.arg for a in fn.args.args] + [a.arg for a in fn.args.kwonlyargs]
    if drop_self and names and names[0] in {"self", "cls"}:
        names = names[1:]
    return names


def _collect_from(body: list[ast.stmt], module: str, cases: list[tuple[str, str, str | None, str]]) -> None:
    for node in body:
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            if node.name.startswith("_"):
                continue
            cases.extend((module, node.name, None, name) for name in _params(node, drop_self=False))
        elif isinstance(node, ast.ClassDef):
            nested = [n for n in node.body if isinstance(n, ast.ClassDef)]
            if nested and node.name.islower():
                # A lower-case class used as a submodule namespace (e.g. ``class pd:``).
                _collect_from(node.body, f"{module}.{node.name}", cases)
                continue
            for item in node.body:
                if not isinstance(item, (ast.FunctionDef, ast.AsyncFunctionDef)):
                    continue
                if item.name.startswith("_") and item.name != "__init__":
                    continue
                deco = {d.id if isinstance(d, ast.Name) else getattr(d, "attr", "") for d in item.decorator_list}
                if "property" in deco or "setter" in deco:
                    continue
                cases.extend((module, item.name, node.name, name) for name in _params(item, drop_self=True))


def _collect_cases() -> list[tuple[str, str, str | None, str]]:
    """Yield (module, callable, class_or_None, parameter_name)."""
    cases: list[tuple[str, str, str | None, str]] = []
    for stub in _iter_stub_files():
        tree = ast.parse(stub.read_text(encoding="utf-8"))
        _collect_from(tree.body, _module_name_for(stub), cases)
    return cases


def _lookup(mod: ModuleType, name: str) -> Any:
    """Find ``name`` on ``mod`` or on any submodule it exposes (one level)."""
    obj = getattr(mod, name, None)
    if obj is not None:
        return obj
    for attr in dir(mod):
        sub = getattr(mod, attr, None)
        if inspect.ismodule(sub) and sub.__name__.startswith(mod.__name__):
            obj = getattr(sub, name, None)
            if obj is not None:
                return obj
    return None


def _import(module: str) -> ModuleType | None:
    """Import ``module``; a private stub-only module resolves to its parent package."""
    try:
        return importlib.import_module(module)
    except ImportError:
        parent, _, leaf = module.rpartition(".")
        if leaf.startswith("_") and parent:
            return importlib.import_module(parent)
        return None


def _resolve(module: str, func: str, cls: str | None) -> Any:
    mod = _import(module)
    if mod is None:
        return None
    if cls is None:
        return _lookup(mod, func)
    klass = _lookup(mod, cls)
    if klass is None:
        return None
    if func == "__init__":
        return klass
    return getattr(klass, func, None)


_CASES = _collect_cases()


@pytest.mark.parametrize(
    ("module", "func", "cls", "param"),
    _CASES,
    ids=[f"{m}.{c + '.' if c else ''}{f}[{p}]" for m, f, c, p in _CASES],
)
def test_documented_keyword_is_accepted(module: str, func: str, cls: str | None, param: str) -> None:
    target = _resolve(module, func, cls)
    if target is None:
        if module == "finstack_quant.statements":
            pytest.fail(f"public declaration {module}.{cls or ''}.{func} is absent at runtime")
        pytest.skip(f"{module}.{cls or ''}.{func} not importable at runtime")
    if param in {"args", "kwargs"}:
        pytest.skip("variadic")
    # 1. The runtime signature must parse (a Rust parameter named after a Python
    #    keyword such as ``from`` makes ``text_signature`` unparseable).
    try:
        sig = inspect.signature(target)
    except ValueError as exc:
        pytest.fail(f"{module}.{cls or ''}.{func}: runtime signature invalid: {exc}")
    # 2. The documented name must be present in the runtime signature.
    runtime_names = set(sig.parameters)
    if runtime_names and param not in runtime_names and "kwargs" not in runtime_names:
        pytest.fail(
            f"{module}.{cls or ''}.{func}: stub parameter {param!r} not in runtime signature {list(runtime_names)}"
        )
    # 3. Calling by keyword must not be rejected (catches text_signature overrides
    #    that disagree with the Rust parameter name).
    # Unbound instance methods need a receiver; pass a sentinel positionally so
    # PyO3 reaches keyword parsing (receiver type errors are reported separately).
    try:
        if (
            cls is not None
            and func != "__init__"
            and (inspect.ismethoddescriptor(target) or inspect.isfunction(target))
        ):
            target(_Sentinel(), **{param: _Sentinel()})
        else:
            target(**{param: _Sentinel()})
    except TypeError as exc:
        msg = str(exc)
        assert UNEXPECTED not in msg, f"{module}.{cls or ''}.{func} rejects keyword {param!r}: {msg}"
    except Exception:  # noqa: BLE001 - any other failure means the keyword was accepted
        pass

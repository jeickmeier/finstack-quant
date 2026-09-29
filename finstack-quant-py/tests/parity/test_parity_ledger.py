"""Bidirectional Python <-> WASM parity ledger checks.

The ledger lives in ``parity_contract.toml`` under ``[parity]``. It accounts for
every public Python name (walked recursively through ``__all__``) and every key
of the live WASM facade, read from ``finstack-quant-wasm/facade-surface.json``
(the snapshot pinned by ``tests/facade/surface.test.mjs``).

A Python name and a WASM key pair by rule when the WASM namespace holds the
same PascalCase class name or the camelCase of the snake_case (or lower-cased
CONSTANT_CASE) name. Everything else is recorded explicitly:

* ``renames``   -- Python name -> full WASM path, with a reason;
* ``types``     -- Python data class -> TypeScript type declared for WASM;
* ``excluded``  -- Python name -> host-exclusion reason from a fixed vocabulary;
* ``backlog``   -- Python names not yet reachable from WASM (must reach zero);
* ``wasm_only`` -- WASM keys with no Python twin, with a reason.

The same partition applies to the members of every class bound in both hosts.
"""

from __future__ import annotations

import inspect
import json
from pathlib import Path
import re
import tomllib
from typing import Any

import pytest

import finstack_quant

ROOT = Path(__file__).parents[3]
CONTRACT = tomllib.loads((Path(__file__).parents[2] / "parity_contract.toml").read_text())
LEDGER: dict[str, Any] = CONTRACT["parity"]
SURFACE: dict[str, dict[str, Any]] = json.loads((ROOT / "finstack-quant-wasm/facade-surface.json").read_text())
DTS = (ROOT / "finstack-quant-wasm/index.d.ts").read_text()
GENERATED = ROOT / "finstack-quant-wasm/types/generated"
TS_TYPES = set(re.findall(r"^(?:export )?(?:declare )?(?:interface|type|class) (\w+)", DTS, re.M)) | {
    path.stem for path in GENERATED.glob("*.ts") if path.stem != "index"
}
REASONS = set(LEDGER["exclusion_reasons"])
PY_RULE_MEMBERS: dict[str, str] = LEDGER["python_rule_excluded_members"]
JS_RULE_MEMBERS: dict[str, str] = LEDGER["js_rule_members"]
MODULES: dict[str, dict[str, Any]] = LEDGER.get("modules", {})
JS: dict[str, dict[str, Any]] = LEDGER.get("js", {})
MEMBERS: dict[str, dict[str, Any]] = LEDGER.get("members", {})


def rule_excluded(name: str) -> bool:
    """Members excluded by rule: listed exits plus any pandas ``to_*_dataframe`` / ``to_*_series`` exit."""
    return name in PY_RULE_MEMBERS or (name.startswith("to_") and name.endswith(("_dataframe", "_series")))


def js_name(name: str) -> str:
    """WASM spelling of a Python name under the naming rule."""
    if name[:1].isupper() and not name.isupper():
        return name
    base = name.lower() if name.isupper() else name
    head, *rest = base.split("_")
    return head + "".join(part[:1].upper() + part[1:] for part in rest)


def python_surface() -> dict[str, dict[str, Any]]:
    """Canonical public Python names per module import path.

    An object re-exported by several modules is counted once, under the module
    named by its ``__module__`` when that module exports it, otherwise under
    the first module (in sorted path order) that does.
    """
    found: dict[str, dict[str, Any]] = {}

    def walk(module: Any, path: str) -> None:
        if path in found:
            return
        found[path] = {}
        for name in getattr(module, "__all__", None) or []:
            value = getattr(module, name)
            if inspect.ismodule(value):
                walk(value, f"{path}.{name}")
            else:
                found[path][name] = value

    walk(finstack_quant, "finstack_quant")
    owners: dict[int, str] = {}
    for path in sorted(found):
        for name, value in found[path].items():
            if not (inspect.isclass(value) or inspect.isroutine(value)):
                continue
            home = getattr(value, "__module__", None)
            current = owners.get(id(value))
            if home in found and name in found[home]:
                owners[id(value)] = home
            elif current is None:
                owners[id(value)] = path
    return {
        path: {
            name: value
            for name, value in names.items()
            if not (inspect.isclass(value) or inspect.isroutine(value)) or owners[id(value)] == path
        }
        for path, names in found.items()
    }


PY = python_surface()


def entry_problems(label: str, names: set[str], rule_matched: set[str], entry: dict[str, Any]) -> list[str]:
    """Partition ``names`` across rule matches and the entry's lists exactly once."""
    buckets = {
        "renames": set(entry.get("renames", {})),
        "types": set(entry.get("types", {})),
        "excluded": set(entry.get("excluded", {})),
        "backlog": set(entry.get("backlog", [])),
    }
    problems = []
    for name in sorted(names):
        homes = [bucket for bucket, members in buckets.items() if name in members]
        if name in rule_matched:
            homes.insert(0, "rule")
        if not homes:
            problems.append(f"{label}.{name}: not accounted for (rule match, renames, types, excluded or backlog)")
        elif len(homes) > 1:
            problems.append(f"{label}.{name}: listed more than once ({', '.join(homes)})")
    problems.extend(
        f"{label}.{name}: listed in {bucket} but not a public Python name"
        for bucket, members in buckets.items()
        for name in sorted(members - names)
    )
    for name, reason in entry.get("excluded", {}).items():
        if reason not in REASONS:
            problems.append(f"{label}.{name}: exclusion reason {reason!r} is not one of {sorted(REASONS)}")
    for name, rename in entry.get("renames", {}).items():
        if not rename.get("reason"):
            problems.append(f"{label}.{name}: rename needs a reason")
    return problems


def test_every_python_module_has_a_ledger_entry() -> None:
    walked = {path for path, names in PY.items() if names}
    assert sorted(walked - set(MODULES)) == [], "add these modules to [parity.modules]"
    assert sorted(set(MODULES) - walked) == [], "these [parity.modules] entries name no public module"


@pytest.mark.parametrize("module_path", sorted(path for path, names in PY.items() if names))
def test_python_module_is_partitioned(module_path: str) -> None:
    entry = MODULES.get(module_path, {})
    js = entry.get("js", "")
    keys = set(SURFACE[js]["keys"]) if js else set()
    assert not js or SURFACE.get(js, {}).get("kind") == "namespace", f"{js!r} is not a WASM namespace"
    names = set(PY[module_path])
    rule_matched = {name for name in names if js_name(name) in keys}
    problems = entry_problems(module_path, names, rule_matched, entry)
    for name, rename in entry.get("renames", {}).items():
        if rename["js"] not in SURFACE:
            problems.append(f"{module_path}.{name}: rename target {rename['js']!r} is not on the facade")
    for name, ts_type in entry.get("types", {}).items():
        if ts_type not in TS_TYPES:
            problems.append(f"{module_path}.{name}: TypeScript type {ts_type!r} is not declared")
    assert problems == []


def claimed_js_paths() -> set[str]:
    claimed = set()
    for path, entry in MODULES.items():
        js = entry.get("js", "")
        keys = set(SURFACE[js]["keys"]) if js else set()
        for name in PY.get(path, {}):
            if js_name(name) in keys and name not in entry.get("renames", {}):
                claimed.add(f"{js}.{js_name(name)}")
        claimed |= {rename["js"] for rename in entry.get("renames", {}).values()}
    return claimed


def test_every_wasm_key_is_accounted_for() -> None:
    claimed = claimed_js_paths()
    problems = []
    for path, node in SURFACE.items():
        if node["kind"] != "namespace":
            continue
        wasm_only = JS.get(path, {}).get("wasm_only", {})
        for key in node["keys"]:
            full = f"{path}.{key}"
            is_claimed = full in claimed or SURFACE[full]["kind"] == "namespace"
            if is_claimed and key in wasm_only:
                problems.append(f"{full}: has a Python twin but is also listed as wasm_only")
            elif not is_claimed and key not in wasm_only:
                problems.append(f"{full}: no Python twin and not listed in [parity.js.{path!r}].wasm_only")
        for key, reason in wasm_only.items():
            if key not in node["keys"]:
                problems.append(f"{path}.{key}: listed as wasm_only but not on the facade")
            if not reason:
                problems.append(f"{path}.{key}: wasm_only needs a reason")
    assert sorted(set(JS) - {p for p, n in SURFACE.items() if n["kind"] == "namespace"}) == []
    assert problems == []


def shared_classes() -> dict[str, tuple[Any, str]]:
    """Python classes bound in both hosts: qualified name -> (class, WASM path)."""
    out = {}
    for path, entry in MODULES.items():
        js = entry.get("js", "")
        for name, value in PY.get(path, {}).items():
            if not inspect.isclass(value):
                continue
            target = entry.get("renames", {}).get(name, {}).get("js") or (f"{js}.{name}" if js else "")
            if SURFACE.get(target, {}).get("kind") == "class":
                out[f"{path}.{name}"] = (value, target)
    return out


SHARED = shared_classes()


# Serde exits that the TypeScript type itself covers for a data class.
DATA_EXITS = {"to_json", "from_json", "to_dict", "from_dict"}


def typed_classes() -> dict[str, Any]:
    """Python data classes recorded as TypeScript types: qualified name -> class."""
    return {
        f"{path}.{name}": PY[path][name]
        for path, entry in MODULES.items()
        for name in entry.get("types", {})
        if name in PY.get(path, {})
    }


TYPED = typed_classes()


def data_class_methods(cls: Any) -> set[str]:
    """Public methods (not properties) of a data class, beyond its serde exits."""
    methods = set()
    for name in dir(cls):
        if name.startswith("_") or rule_excluded(name) or name in DATA_EXITS:
            continue
        raw = inspect.getattr_static(cls, name)
        if isinstance(raw, (staticmethod, classmethod)) or inspect.isroutine(raw) or inspect.ismethoddescriptor(raw):
            methods.add(name)
    return methods


def test_member_ledger_names_only_bound_classes() -> None:
    assert sorted(set(MEMBERS) - set(SHARED) - set(TYPED)) == []


@pytest.mark.parametrize("qualified", sorted(TYPED))
def test_data_class_methods_are_accounted_for(qualified: str) -> None:
    """A TypeScript type carries the data; the computations need WASM twins too."""
    entry = MEMBERS.get(qualified, {})
    problems = entry_problems(qualified, data_class_methods(TYPED[qualified]), set(), entry)
    for name, rename in entry.get("renames", {}).items():
        if SURFACE.get(rename["js"], {}).get("kind") != "function":
            problems.append(f"{qualified}.{name}: rename target {rename['js']!r} is not a WASM function")
    assert problems == []


@pytest.mark.parametrize("qualified", sorted(SHARED))
def test_shared_class_members_are_partitioned(qualified: str) -> None:
    cls, target = SHARED[qualified]
    node = SURFACE[target]
    js_members = set(node["statics"]) | set(node["methods"]) | set(node["getters"])
    entry = MEMBERS.get(qualified, {})
    py_members = {name for name in dir(cls) if not name.startswith("_") and not rule_excluded(name)}
    rule_matched = {name for name in py_members if js_name(name) in js_members}
    problems = entry_problems(qualified, py_members, rule_matched, entry)
    claimed = {js_name(name) for name in rule_matched} | {rename["js"] for rename in entry.get("renames", {}).values()}
    wasm_only = entry.get("wasm_only", {})
    for name in sorted(js_members - claimed - set(JS_RULE_MEMBERS)):
        if name not in wasm_only:
            problems.append(f"{target}.{name}: no Python twin and not listed as a member wasm_only")
    for name, reason in wasm_only.items():
        if name not in js_members or name in claimed:
            problems.append(f"{target}.{name}: stale member wasm_only entry")
        if not reason:
            problems.append(f"{target}.{name}: wasm_only needs a reason")
    for rename in entry.get("renames", {}).values():
        if rename["js"] not in js_members:
            problems.append(f"{target}: member rename target {rename['js']!r} does not exist")
    assert problems == []


def test_backlog_only_shrinks() -> None:
    """The backlog is the remaining full-parity work; its ceiling is a ratchet.

    Binding work lowers ``[parity].backlog_ceiling`` in the same change, so a
    later change cannot quietly grow the backlog back.
    """
    ceiling = LEDGER["backlog_ceiling"]
    module_backlog = sum(len(entry.get("backlog", [])) for entry in MODULES.values())
    member_backlog = sum(len(entry.get("backlog", [])) for entry in MEMBERS.values())
    assert module_backlog <= ceiling["modules"], "module backlog grew; bind the new name or record why"
    assert member_backlog <= ceiling["members"], "member backlog grew; bind the new member or record why"

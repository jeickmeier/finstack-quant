"""Structural parity checks driven by ``finstack-quant-py/parity_contract.toml``."""

from __future__ import annotations

import ast
from collections import Counter
import importlib
import inspect
from pathlib import Path
import re
import tomllib
from typing import Any

import pytest

CONTRACT_PATH = Path(__file__).parents[2] / "parity_contract.toml"
VALID_MODULE_STATUSES = {"exists", "flattened", "missing"}


def _load_contract() -> dict[str, Any]:
    return tomllib.loads(CONTRACT_PATH.read_text())


CONTRACT = _load_contract()


def _module_entries(*statuses: str) -> list[tuple[str, str, str, str]]:
    entries: list[tuple[str, str, str, str]] = []
    for crate_name, crate in CONTRACT["crates"].items():
        for module_name, spec in crate.get("modules", {}).items():
            status = spec["status"]
            if status in statuses:
                entries.append((crate_name, module_name, spec["python"], status))
    return entries


ROOT_PACKAGES = [
    (crate_name, crate["python_package"])
    for crate_name, crate in CONTRACT["crates"].items()
    if crate.get("status") == "exists"
]

PUBLIC_MODULES = _module_entries("exists", "flattened")
MISSING_MODULES = _module_entries("missing")


def test_contract_lives_with_python_bindings() -> None:
    """The Python parity contract should be stored in the Python package tree.

    Anchored on the test file rather than the process working directory, so the
    suite passes whether pytest runs from the repository root or from
    ``finstack-quant-py/``.
    """
    assert CONTRACT_PATH.is_file()
    assert CONTRACT_PATH.parent.name == "finstack-quant-py"


def test_contract_uses_known_module_statuses() -> None:
    """Module status values should stay explicit and auditable."""
    unknown = [
        (crate_name, module_name, spec["status"])
        for crate_name, crate in CONTRACT["crates"].items()
        for module_name, spec in crate.get("modules", {}).items()
        if spec["status"] not in VALID_MODULE_STATUSES
    ]
    assert unknown == []


def _duplicate_string_lists(value: Any, path: str = "") -> list[tuple[str, list[str]]]:
    duplicates: list[tuple[str, list[str]]] = []
    if isinstance(value, dict):
        for key, child in value.items():
            child_path = f"{path}.{key}" if path else key
            duplicates.extend(_duplicate_string_lists(child, child_path))
    elif isinstance(value, list):
        if all(isinstance(item, str) for item in value):
            counts = Counter(value)
            repeated = sorted(item for item, count in counts.items() if count > 1)
            if repeated:
                duplicates.append((path, repeated))
        else:
            for index, child in enumerate(value):
                duplicates.extend(_duplicate_string_lists(child, f"{path}[{index}]"))
    return duplicates


def test_contract_string_lists_have_unique_entries() -> None:
    """Every string inventory should name each contracted entry exactly once."""
    assert _duplicate_string_lists(CONTRACT) == []


def _umbrella_reexports() -> list[tuple[str, str]]:
    """Return ``(alias, rust_crate)`` pairs from the canonical umbrella crate."""
    umbrella_path = CONTRACT_PATH.parents[1] / CONTRACT["meta"]["umbrella_lib"]
    source = umbrella_path.read_text()
    return [
        (match.group("alias"), match.group("crate"))
        for match in re.finditer(
            r"^\s*pub\s+use\s+(?P<crate>finstack_quant_[A-Za-z0-9_]+)"
            r"\s+as\s+(?P<alias>[A-Za-z_][A-Za-z0-9_]*)\s*;",
            source,
            re.MULTILINE,
        )
    ]


def _contract_umbrella_reexports() -> dict[str, str]:
    """Return the public contract's expected umbrella alias-to-crate mapping."""
    return {
        alias: spec["rust_crate"].replace("-", "_")
        for alias, spec in CONTRACT["crates"].items()
        if spec.get("visibility") == "pub" and spec.get("rust_crate") and spec.get("rust_lib")
    }


def test_rust_umbrella_reexports_match_contract() -> None:
    """Rust umbrella aliases and public parity-contract crates must agree exactly."""
    parsed = _umbrella_reexports()
    expected = _contract_umbrella_reexports()

    alias_counts = Counter(alias for alias, _ in parsed)
    crate_counts = Counter(rust_crate for _, rust_crate in parsed)
    duplicate_aliases = sorted(alias for alias, count in alias_counts.items() if count > 1)
    duplicate_crates = sorted(rust_crate for rust_crate, count in crate_counts.items() if count > 1)
    expected_crate_counts = Counter(expected.values())
    duplicate_contract_crates = sorted(rust_crate for rust_crate, count in expected_crate_counts.items() if count > 1)

    actual = dict(parsed)
    missing = sorted(expected.keys() - actual.keys())
    extra = sorted(actual.keys() - expected.keys())
    mismapped = {
        alias: {"expected": expected[alias], "actual": actual[alias]}
        for alias in sorted(expected.keys() & actual.keys())
        if expected[alias] != actual[alias]
    }

    assert not any((duplicate_aliases, duplicate_crates, duplicate_contract_crates, missing, extra, mismapped)), (
        "Rust umbrella re-exports diverged from the parity contract.\n"
        f"  duplicate aliases: {duplicate_aliases}\n"
        f"  duplicate umbrella crates: {duplicate_crates}\n"
        f"  duplicate contract crates: {duplicate_contract_crates}\n"
        f"  missing aliases: {missing}\n"
        f"  extra aliases: {extra}\n"
        f"  mismapped aliases: {mismapped}"
    )


@pytest.mark.parametrize(("crate_name", "package_name"), ROOT_PACKAGES)
def test_contract_root_packages_are_importable(crate_name: str, package_name: str) -> None:
    """Every crate marked present in the contract should have an importable package."""
    assert crate_name
    importlib.import_module(package_name)


@pytest.mark.parametrize(
    ("crate_name", "module_name", "module_path", "status"),
    PUBLIC_MODULES,
)
def test_contract_public_modules_are_importable(
    crate_name: str,
    module_name: str,
    module_path: str,
    status: str,
) -> None:
    """``exists`` and ``flattened`` contract entries should resolve in Python."""
    assert crate_name
    assert module_name
    assert status in {"exists", "flattened"}
    importlib.import_module(module_path)


@pytest.mark.parametrize(
    ("crate_name", "module_name", "module_path", "status"),
    MISSING_MODULES,
)
def test_contract_missing_modules_are_not_importable(
    crate_name: str,
    module_name: str,
    module_path: str,
    status: str,
) -> None:
    """``missing`` contract entries should stay absent until the contract changes."""
    assert crate_name
    assert module_name
    assert status == "missing"
    with pytest.raises(ModuleNotFoundError) as exc_info:
        importlib.import_module(module_path)

    missing_name = exc_info.value.name
    assert missing_name is not None
    assert module_path == missing_name or module_path.startswith(f"{missing_name}.")


def _pyi_top_level_names(pyi_path: Path) -> set[str]:
    """Extract module-level public names declared in a .pyi stub.

    The regex matches lines starting with a lowercase letter, which by
    convention excludes dunders like ``__all__`` and any underscore-prefixed
    private names without needing a separate filter.
    """
    source = pyi_path.read_text()
    return {m.group(1) for m in re.finditer(r"^([a-z][a-zA-Z0-9_]*)\s*:\s*\w", source, re.MULTILINE)}


def _pyi_all_names(pyi_path: Path) -> list[str]:
    """Extract the string names from a stub's explicit ``__all__`` list."""
    tree = ast.parse(pyi_path.read_text())
    for node in tree.body:
        if isinstance(node, ast.Assign) and any(
            isinstance(target, ast.Name) and target.id == "__all__" for target in node.targets
        ):
            names = ast.literal_eval(node.value)
            assert isinstance(names, list)
            assert all(isinstance(name, str) for name in names)
            return names
    raise AssertionError(f"{pyi_path} must declare an explicit __all__ list")


def test_pyi_top_level_matches_contract() -> None:
    """The native `.pyi`, extension ``__all__``, and contract must agree.

    Drift in any of the three is a maintenance hazard, since they all encode
    the compiled extension's registered domain modules. Pure-Python packages
    such as ``finstack_quant.reporting`` are tracked separately by the crate
    package and symbol contract entries.
    """
    block = CONTRACT["pyi_top_level"]
    pyi_path = CONTRACT_PATH.parent / block["file"]
    contract = set(block["names"])
    pyi = _pyi_top_level_names(pyi_path)
    extension_all = set(importlib.import_module("finstack_quant.finstack_quant").__all__)

    assert pyi == contract, (
        f"finstack_quant.pyi top-level names diverged from contract.\n"
        f"  missing from .pyi: {sorted(contract - pyi)}\n"
        f"  unlisted in contract: {sorted(pyi - contract)}"
    )
    assert extension_all == contract, (
        f"finstack_quant.finstack_quant.__all__ diverged from contract.\n"
        f"  missing from extension __all__: {sorted(contract - extension_all)}\n"
        f"  unlisted in contract: {sorted(extension_all - contract)}"
    )


def _symbol_entries() -> list[tuple[str, str, str]]:
    """Yield (crate_name, package_path, symbol_name) for every contract symbol."""
    entries: list[tuple[str, str, str]] = []
    for crate_name, crate in CONTRACT["crates"].items():
        symbols = crate.get("symbols", {})
        entries.extend((crate_name, crate["python_package"], sym) for sym in symbols.get("public", []))
    return entries


SYMBOL_ENTRIES = _symbol_entries()


@pytest.mark.parametrize(
    ("crate_name", "package_path", "symbol_name"),
    SYMBOL_ENTRIES,
)
def test_contract_symbols_are_importable(
    crate_name: str,
    package_path: str,
    symbol_name: str,
) -> None:
    """Every contract symbol must resolve as an attribute of its package."""
    assert crate_name
    module = importlib.import_module(package_path)
    assert hasattr(module, symbol_name), (
        f"{package_path} does not expose `{symbol_name}` "
        f"(listed in parity contract under `{crate_name}.symbols.public`)"
    )


CRATES_WITH_SYMBOLS = [(crate_name, crate) for crate_name, crate in CONTRACT["crates"].items() if "symbols" in crate]


@pytest.mark.parametrize(("crate_name", "crate"), CRATES_WITH_SYMBOLS)
def test_contract_symbols_match_live_surface(crate_name: str, crate: dict[str, Any]) -> None:
    """The contract's `symbols.public` list must match the live public surface.

    Catches both directions: a public name added without contract update, and
    a contract entry that no longer exists in Python.
    """
    expected = set(crate["symbols"]["public"])
    # Only count module entries that live inside this crate's own package;
    # cross-package homes (e.g. analytics' correlation module surfacing under
    # finstack_quant.models.correlation) are not part of this surface.
    expected_all = expected | {
        spec["python"].rsplit(".", 1)[-1]
        for spec in crate.get("modules", {}).values()
        if spec["status"] == "exists" and spec["python"].startswith(crate["python_package"] + ".")
    }
    module = importlib.import_module(crate["python_package"])
    module_all = set(getattr(module, "__all__", []))
    actual = {n for n in dir(module) if not n.startswith("_") and not inspect.ismodule(getattr(module, n))}
    assert module_all == expected_all, (
        f"finstack_quant.{crate_name} __all__ diverged from contract.\n"
        f"  missing from __all__: {sorted(expected_all - module_all)}\n"
        f"  unlisted in contract: {sorted(module_all - expected_all)}"
    )
    assert actual == expected, (
        f"finstack_quant.{crate_name} public surface diverged from contract.\n"
        f"  missing from Python: {sorted(expected - actual)}\n"
        f"  unlisted in contract: {sorted(actual - expected)}"
    )


def _module_symbol_entries() -> list[tuple[str, str, dict[str, Any]]]:
    """Yield per-submodule symbol contracts declared under each crate."""
    entries: list[tuple[str, str, dict[str, Any]]] = []
    for crate_name, crate in CONTRACT["crates"].items():
        entries.extend((crate_name, module_name, spec) for module_name, spec in crate.get("module_symbols", {}).items())
    return entries


@pytest.mark.parametrize(
    ("crate_name", "module_name", "spec"),
    _module_symbol_entries(),
)
def test_module_symbol_contract_matches_live_surface(
    crate_name: str,
    module_name: str,
    spec: dict[str, Any],
) -> None:
    """Every contracted submodule symbol resolves and agrees with `__all__`."""
    assert crate_name
    assert module_name
    module = importlib.import_module(spec["python_package"])
    expected = set(spec["public"])
    missing = {name for name in expected if not hasattr(module, name)}
    assert not missing, f"{spec['python_package']} is missing contracted symbols: {sorted(missing)}"

    module_all = set(getattr(module, "__all__", []))
    if spec.get("allow_additional", False):
        assert expected <= module_all, (
            f"{spec['python_package']}.__all__ omits contracted symbols: {sorted(expected - module_all)}"
        )
    else:
        assert module_all == expected, (
            f"{spec['python_package']}.__all__ diverged from its module-symbol contract.\n"
            f"  missing from __all__: {sorted(expected - module_all)}\n"
            f"  unlisted in contract: {sorted(module_all - expected)}"
        )


def _wasm_index_js_namespaces(index_js_path: Path) -> set[str]:
    """Extract top-level namespaces re-exported from `./exports/<file>.js`.

    Matches lines like:
        export { core } from './exports/core.js';
    """
    source = index_js_path.read_text()
    return {
        m.group(1)
        for m in re.finditer(
            r"^export\s+\{\s+([A-Za-z_][A-Za-z0-9_]*)\s+\}\s+from\s+'\./exports/",
            source,
            re.MULTILINE,
        )
    }


def test_wasm_top_level_matches_contract() -> None:
    """`finstack-quant-wasm/index.js` top-level namespaces must match the contract."""
    block = CONTRACT["wasm_top_level"]
    # The `file` field is a path relative to the contract file itself.
    index_path = (CONTRACT_PATH.parent / block["file"]).resolve()
    expected = set(block["namespaces"])
    actual = _wasm_index_js_namespaces(index_path)
    assert actual == expected, (
        f"finstack-quant-wasm/index.js top-level namespaces diverged from contract.\n"
        f"  missing from index.js: {sorted(expected - actual)}\n"
        f"  unlisted in contract: {sorted(actual - expected)}"
    )


def test_wasm_top_level_has_exports_files() -> None:
    """Each contract namespace must have a corresponding ``exports/<name>.js`` file.

    Unique failure mode this catches: contract and ``index.js`` agree on a
    namespace, but the underlying ``exports/<name>.js`` was deleted. The
    matches-contract test would still pass (the regex match in ``index.js``
    still resolves the name) but JS consumers would error at runtime.
    """
    block = CONTRACT["wasm_top_level"]
    exports_dir = (CONTRACT_PATH.parent / block["file"]).resolve().parent / "exports"
    missing = [ns for ns in block["namespaces"] if not (exports_dir / f"{ns}.js").exists()]
    assert not missing, f"contract lists namespaces that have no exports/*.js file: {missing}"


def _parse_valuations_js_root_exports(js_path: Path) -> set[str]:
    """Extract top-level keys from `export const valuations = { ... }`."""
    source = js_path.read_text().splitlines()
    keys: set[str] = set()
    depth = 0
    for line in source:
        if depth == 0:
            if "export const valuations = {" in line:
                depth = 1
            continue
        stripped = line.strip()
        if depth == 1:
            if not stripped or stripped.startswith("//"):
                pass
            else:
                key_match = re.match(r"^([A-Za-z_][A-Za-z0-9_]*)\s*:", stripped)
                if key_match:
                    keys.add(key_match.group(1))
                else:
                    method_match = re.match(r"^([A-Za-z_][A-Za-z0-9_]*)\s*\(", stripped)
                    if method_match:
                        keys.add(method_match.group(1))
                    else:
                        shorthand_match = re.match(r"^([A-Za-z_][A-Za-z0-9_]*)\s*,?\s*$", stripped)
                        if shorthand_match:
                            keys.add(shorthand_match.group(1))
        depth += line.count("{") - line.count("}")
        if depth <= 0:
            break
    return keys


def _parse_exported_const_object_keys(js_path: Path, const_name: str) -> set[str]:
    """Extract first-level keys from `export const <name> = { ... }`."""
    source = js_path.read_text().splitlines()
    keys: set[str] = set()
    depth = 0
    pattern = f"export const {const_name} = {{"
    for line in source:
        if depth == 0:
            if pattern in line:
                depth = 1
            continue
        stripped = line.strip()
        if depth == 1 and stripped and not stripped.startswith("//"):
            key_match = re.match(r"^([A-Za-z_][A-Za-z0-9_]*)\s*[:,]", stripped)
            if key_match:
                keys.add(key_match.group(1))
        depth += line.count("{") - line.count("}")
        if depth <= 0:
            break
    return keys


def test_wasm_valuations_exports_match_contract() -> None:
    """`exports/valuations.js` root keys must match [wasm_valuations_subset]."""
    block = CONTRACT["wasm_valuations_subset"]
    js_path = (CONTRACT_PATH.parent / block["js_export_file"]).resolve()
    expected_root = set(block["root_exports"])
    expected_nested = set(block["nested"])
    actual_root = _parse_valuations_js_root_exports(js_path)
    assert expected_nested <= actual_root, (
        f"nested facade keys missing from valuations.js root: {sorted(expected_nested - actual_root)}"
    )
    non_nested_actual = actual_root - expected_nested
    assert non_nested_actual == expected_root, (
        f"valuations.js root exports diverged from contract.\n"
        f"  missing from JS: {sorted(expected_root - non_nested_actual)}\n"
        f"  unlisted in contract: {sorted(non_nested_actual - expected_root)}"
    )


def test_wasm_models_exports_match_contract() -> None:
    """`exports/models.js` root and nested keys must match the models subset."""
    block = CONTRACT["wasm_models_subset"]
    js_path = (CONTRACT_PATH.parent / block["js_export_file"]).resolve()
    expected = set(block["root_exports"]) | set(block["nested"])
    actual = _parse_exported_const_object_keys(js_path, "models")
    assert actual == expected, (
        "models.js exports diverged from contract.\n"
        f"  missing from JS: {sorted(expected - actual)}\n"
        f"  unlisted in contract: {sorted(actual - expected)}"
    )


def test_wasm_models_nested_exports_match_contract() -> None:
    """Nested `exports/models/*.js` facade keys must match the contract."""
    block = CONTRACT["wasm_models_subset"]
    models_js_path = (CONTRACT_PATH.parent / block["js_export_file"]).resolve()
    nested_dir = models_js_path.parent / "models"
    for namespace, expected_names in block["nested_exports"].items():
        js_path = nested_dir / f"{namespace}.js"
        assert js_path.exists(), f"nested models facade missing: {js_path}"
        actual = _parse_exported_const_object_keys(js_path, namespace)
        expected = set(expected_names)
        assert actual == expected, (
            f"models.{namespace} facade exports diverged from contract.\n"
            f"  missing from JS: {sorted(expected - actual)}\n"
            f"  unlisted in contract: {sorted(actual - expected)}"
        )


def test_wasm_models_root_surface_is_triplet_accounted_for() -> None:
    """Every models root export must map to a live Python root symbol."""
    block = CONTRACT["wasm_models_subset"]
    assert set(block["python_js_map"].values()) == set(block["root_exports"])
    module = importlib.import_module("finstack_quant.models")
    assert not [name for name in block["python_js_map"] if not hasattr(module, name)]


def test_wasm_attribution_exports_match_contract() -> None:
    """`exports/attribution.js` root keys must match [wasm_attribution_subset]."""
    block = CONTRACT["wasm_attribution_subset"]
    js_path = (CONTRACT_PATH.parent / block["js_export_file"]).resolve()
    expected = set(block["root_exports"])
    actual = _parse_exported_const_object_keys(js_path, "attribution")
    assert actual == expected, (
        f"attribution.js exports diverged from contract.\n"
        f"  missing from JS: {sorted(expected - actual)}\n"
        f"  unlisted in contract: {sorted(actual - expected)}"
    )


def test_wasm_attribution_root_exports_are_triplet_accounted_for() -> None:
    """Every attribution WASM root export is mapped from Python or listed wasm-only."""
    block = CONTRACT["wasm_attribution_subset"]
    root_exports = set(block["root_exports"])
    wasm_only = set(block.get("wasm_only", {}).get("symbols", []))
    mapped_js = set(block["python_js_map"].values())
    _assert_wasm_subset_root_exports_accounted(
        "wasm_attribution_subset",
        root_exports=root_exports,
        mapped_js=mapped_js,
        wasm_only=wasm_only,
    )


def test_wasm_cashflows_exports_match_contract() -> None:
    """`exports/cashflows.js` root keys must match [wasm_cashflows_subset]."""
    block = CONTRACT["wasm_cashflows_subset"]
    js_path = (CONTRACT_PATH.parent / block["js_export_file"]).resolve()
    expected = set(block["root_exports"])
    actual = _parse_exported_const_object_keys(js_path, "cashflows")
    assert actual == expected, (
        f"cashflows.js exports diverged from contract.\n"
        f"  missing from JS: {sorted(expected - actual)}\n"
        f"  unlisted in contract: {sorted(actual - expected)}"
    )


def test_wasm_cashflows_root_exports_are_triplet_accounted_for() -> None:
    """Every cashflows WASM root export is pinned in python_js_map."""
    block = CONTRACT["wasm_cashflows_subset"]
    root_exports = set(block["root_exports"])
    mapped_js = set(block["python_js_map"].values())
    assert mapped_js == root_exports, (
        f"cashflows python_js_map must cover all root exports exactly.\n"
        f"  unmapped root exports: {sorted(root_exports - mapped_js)}\n"
        f"  mapped but not exported: {sorted(mapped_js - root_exports)}"
    )


def test_python_cashflows_surface_matches_contract() -> None:
    """`finstack_quant.cashflows` __all__ = JSON triplet names + python_only submodules."""
    block = CONTRACT["wasm_cashflows_subset"]
    module = importlib.import_module("finstack_quant.cashflows")
    json_names = set(block["python_js_map"])
    typed_submodules = set(block.get("python_only", []))
    typed_symbols = set(block.get("python_only_symbols", []))
    expected = json_names | typed_submodules | typed_symbols
    actual = set(module.__all__)
    assert actual == expected, (
        f"finstack_quant.cashflows __all__ diverged from contract.\n"
        f"  missing from Python: {sorted(expected - actual)}\n"
        f"  unlisted in contract: {sorted(actual - expected)}"
    )
    missing = [name for name in json_names if not callable(getattr(module, name, None))]
    assert not missing, f"finstack_quant.cashflows symbols not callable: {missing}"
    not_modules = [name for name in typed_submodules if not inspect.ismodule(getattr(module, name, None))]
    assert not not_modules, f"finstack_quant.cashflows entries not modules: {not_modules}"
    missing_typed = [name for name in typed_symbols if not hasattr(module, name)]
    assert not missing_typed, f"finstack_quant.cashflows typed symbols missing: {missing_typed}"


def test_cashflows_has_no_cross_crate_symbols() -> None:
    """Cashflows must not own helpers from other crates."""
    assert "cross_crate" not in CONTRACT["crates"]["cashflows"]


WASM_NAMESPACE_SUBSETS = [
    ("wasm_analytics_subset", "analytics", "finstack_quant.analytics"),
    ("wasm_calibration_subset", "calibration", "finstack_quant.calibration"),
    ("wasm_covenants_subset", "covenants", "finstack_quant.covenants"),
    ("wasm_features_subset", "features", "finstack_quant.features"),
    ("wasm_margin_subset", "margin", "finstack_quant.margin"),
    ("wasm_scenarios_subset", "scenarios", "finstack_quant.scenarios"),
    ("wasm_statements_subset", "statements", "finstack_quant.statements"),
    ("wasm_statements_analytics_subset", "statements_analytics", "finstack_quant.statements_analytics"),
]


def test_analytics_periodic_returns_shared_method_contract() -> None:
    """The raw periodic-return panel must stay named across all three hosts."""
    block = CONTRACT["wasm_analytics_subset"]
    assert block["shared_methods"] == {"Performance.periodic_returns": "Performance.periodicReturns"}

    performance = importlib.import_module("finstack_quant.analytics").Performance
    assert callable(getattr(performance, "periodic_returns", None))

    stub = (CONTRACT_PATH.parent / "finstack_quant" / "analytics" / "__init__.pyi").read_text()
    assert "def periodic_returns(" in stub
    assert "list[list[tuple[datetime.date, float]]]" in stub

    dts = (CONTRACT_PATH.parent.parent / "finstack-quant-wasm" / "index.d.ts").read_text()
    assert "periodicReturns(frequency?: string): PeriodicReturnPoint[][];" in dts


@pytest.mark.parametrize(("section", "const_name", "python_package"), WASM_NAMESPACE_SUBSETS)
def test_wasm_namespace_subset_exports_match_contract(section: str, const_name: str, python_package: str) -> None:
    """`exports/<ns>.js` root keys must match the contract subset section."""
    assert python_package
    block = CONTRACT[section]
    js_path = (CONTRACT_PATH.parent / block["js_export_file"]).resolve()
    expected = set(block["root_exports"])
    actual = _parse_exported_const_object_keys(js_path, const_name)
    assert actual == expected, (
        f"{const_name}.js exports diverged from contract.\n"
        f"  missing from JS: {sorted(expected - actual)}\n"
        f"  unlisted in contract: {sorted(actual - expected)}"
    )


def _assert_wasm_subset_root_exports_accounted(
    section: str,
    *,
    root_exports: set[str],
    mapped_js: set[str],
    wasm_only: set[str],
) -> None:
    overlap = wasm_only & mapped_js
    assert not overlap, f"{section} wasm_only overlaps python_js_map values: {sorted(overlap)}"

    accounted = mapped_js | wasm_only
    assert accounted == root_exports, (
        f"{section} mapped_js union wasm_only must equal root_exports exactly.\n"
        f"  unaccounted root exports: {sorted(root_exports - accounted)}\n"
        f"  accounted but not exported: {sorted(accounted - root_exports)}"
    )


def test_wasm_namespace_subset_accounting_rejects_extra_mapping() -> None:
    """A mapped JS name absent from root_exports must fail the topology gate."""
    with pytest.raises(AssertionError, match="exactly"):
        _assert_wasm_subset_root_exports_accounted(
            "test_subset",
            root_exports={"liveExport"},
            mapped_js={"liveExport", "staleExport"},
            wasm_only=set(),
        )


def test_wasm_namespace_subset_accounting_rejects_overlap() -> None:
    """A root export cannot be both mapped and explicitly WASM-only."""
    with pytest.raises(AssertionError, match="overlaps"):
        _assert_wasm_subset_root_exports_accounted(
            "test_subset",
            root_exports={"liveExport"},
            mapped_js={"liveExport"},
            wasm_only={"liveExport"},
        )


@pytest.mark.parametrize(("section", "const_name", "python_package"), WASM_NAMESPACE_SUBSETS)
def test_wasm_namespace_subset_root_exports_are_triplet_accounted_for(
    section: str, const_name: str, python_package: str
) -> None:
    """Every subset root export is mapped from Python or listed wasm-only."""
    assert const_name
    assert python_package
    block = CONTRACT[section]
    root_exports = set(block["root_exports"])
    wasm_only = set(block.get("wasm_only", {}).get("symbols", []))
    mapped_js = set(block["python_js_map"].values())
    _assert_wasm_subset_root_exports_accounted(
        section,
        root_exports=root_exports,
        mapped_js=mapped_js,
        wasm_only=wasm_only,
    )


@pytest.mark.parametrize(("section", "const_name", "python_package"), WASM_NAMESPACE_SUBSETS)
def test_wasm_namespace_subset_python_twins_resolve(section: str, const_name: str, python_package: str) -> None:
    """Each python_js_map key must resolve on the live Python module."""
    assert const_name
    block = CONTRACT[section]
    module = importlib.import_module(python_package)
    missing = [name for name in block["python_js_map"] if not hasattr(module, name)]
    assert not missing, f"{python_package} missing python_js_map twins: {missing}"


def test_wasm_valuations_nested_exports_match_contract() -> None:
    """Nested `exports/valuations/*.js` facade keys must match the contract."""
    block = CONTRACT["wasm_valuations_subset"]
    nested_exports = block.get("nested_exports", {})
    valuations_js_path = (CONTRACT_PATH.parent / block["js_export_file"]).resolve()
    nested_dir = valuations_js_path.parent / "valuations"

    for namespace, expected_names in nested_exports.items():
        js_path = nested_dir / f"{namespace}.js"
        assert js_path.exists(), f"nested valuations facade missing: {js_path}"
        actual = _parse_exported_const_object_keys(js_path, namespace)
        expected = set(expected_names)
        assert actual == expected, (
            f"valuations.{namespace} facade exports diverged from contract.\n"
            f"  missing from JS: {sorted(expected - actual)}\n"
            f"  unlisted in contract: {sorted(actual - expected)}"
        )


def _python_path_js_map_violations(block: dict[str, Any]) -> list[str]:
    """Return a description for each ``python_path_js_map`` entry that can't resolve.

    An entry resolves if its JS name is a root export, appears in
    ``nested_exports`` for its own Python namespace, or appears in
    ``nested_exports`` for a namespace explicitly pinned via
    ``python_path_alias_namespaces``. Unlike a "search every namespace"
    fallback, an unpinned alias never resolves — a name parked under an
    unrelated namespace (or under no namespace at all) is reported.
    """
    root_exports = set(block.get("root_exports", []))
    nested_exports = block.get("nested_exports", {})
    alias_namespaces = block.get("python_path_alias_namespaces", {})
    violations: list[str] = []
    for python_path, js_name in block.get("python_path_js_map", {}).items():
        if js_name in root_exports:
            continue
        namespace = python_path.partition(".")[0]
        allowed_namespaces = [namespace, *alias_namespaces.get(namespace, [])]
        allowed_names = {name for ns in allowed_namespaces for name in nested_exports.get(ns, [])}
        if js_name not in allowed_names:
            violations.append(
                f"python_path_js_map[{python_path!r}] -> {js_name!r} not in root_exports or "
                f"nested_exports for namespace(s) {allowed_namespaces!r}"
            )
    return violations


def test_wasm_valuations_python_js_map_matches_facade_exports() -> None:
    """Pinned Python-to-JS mappings must resolve at their declared facade level."""
    block = CONTRACT["wasm_valuations_subset"]
    root_exports = set(block["root_exports"])
    for python_name, js_name in block["python_js_map"].items():
        assert js_name in root_exports, f"python_js_map[{python_name!r}] -> {js_name!r} not in root_exports"

    violations = _python_path_js_map_violations(block)
    assert not violations, "\n".join(violations)


def test_wasm_valuations_python_js_map_rejects_name_in_no_namespace() -> None:
    """Adversarial check: a name absent from every namespace must still fail.

    Guards against a check that's specific enough to pin real flattens but
    accidentally so loose it accepts anything.
    """
    block = {
        "root_exports": [],
        "nested_exports": {"instruments": ["Bond"], "fx": ["FxForward"]},
        "python_path_alias_namespaces": {"instruments": ["fx"]},
        "python_path_js_map": {"instruments.Ghost": "ghostFn"},
    }
    violations = _python_path_js_map_violations(block)
    assert violations, "expected a violation for a name absent from every namespace"


def test_wasm_valuations_python_js_map_rejects_wrong_namespace() -> None:
    """Adversarial check: a real name parked under the wrong namespace must still fail.

    ``creditGradesModelJson`` genuinely exists, but only under ``credit`` —
    not under ``instruments`` or its pinned alias ``fx``. A "search any
    namespace" fallback would incorrectly accept this; the alias-pinned
    check must not.
    """
    block = {
        "root_exports": [],
        "nested_exports": {
            "instruments": ["Bond"],
            "fx": ["FxForward"],
            "credit": ["creditGradesModelJson"],
        },
        "python_path_alias_namespaces": {"instruments": ["fx"]},
        "python_path_js_map": {"instruments.creditGradesModelJson": "creditGradesModelJson"},
    }
    violations = _python_path_js_map_violations(block)
    assert violations, "expected a violation for a name parked under an unrelated namespace"


def test_wasm_valuations_python_twins_resolve() -> None:
    """Every root and qualified Python twin must resolve on the live package."""
    block = CONTRACT["wasm_valuations_subset"]
    module = importlib.import_module("finstack_quant.valuations")
    missing = [name for name in block["python_js_map"] if not hasattr(module, name)]
    for path in block.get("python_path_js_map", {}):
        value: Any = module
        for component in path.split("."):
            value = getattr(value, component, None)
            if value is None:
                missing.append(path)
                break
    assert not missing, f"finstack_quant.valuations missing mapped twins: {missing}"


def test_wasm_valuations_root_exports_are_triplet_accounted_for() -> None:
    """Every root export is pinned in python_js_map or listed wasm_only."""
    block = CONTRACT["wasm_valuations_subset"]
    root_exports = set(block["root_exports"])
    wasm_only = set(block.get("wasm_only", []))
    mapped_js = set(block["python_js_map"].values())
    _assert_wasm_subset_root_exports_accounted(
        "wasm_valuations_subset",
        root_exports=root_exports,
        mapped_js=mapped_js,
        wasm_only=wasm_only,
    )


def test_wasm_valuations_python_only_excludes_wasm_map() -> None:
    """python_only symbols must not appear in the WASM python_js_map."""
    block = CONTRACT["wasm_valuations_subset"]
    python_only = set(block["python_only"])
    mapped_python = set(block["python_js_map"])
    overlap = python_only & mapped_python
    assert not overlap, f"python_only overlaps python_js_map keys: {sorted(overlap)}"


def test_wasm_valuations_python_surface_is_exhaustively_accounted_for() -> None:
    """Every Python root symbol is mapped to WASM or explicitly Python-only."""
    block = CONTRACT["wasm_valuations_subset"]
    public = set(CONTRACT["crates"]["valuations"]["symbols"]["public"])
    mapped_python = set(block["python_js_map"])
    python_only = set(block["python_only"])
    assert not mapped_python & python_only, "mapped Python names cannot also be Python-only"
    accounted = mapped_python | python_only
    assert accounted == public, (
        "valuations Python/WASM accounting diverged from the canonical Python surface.\n"
        f"  unaccounted Python symbols: {sorted(public - accounted)}\n"
        f"  non-public accounting entries: {sorted(accounted - public)}"
    )


def test_wasm_valuations_python_js_names_use_camel_or_pascal_case() -> None:
    """WASM export names should be camelCase or PascalCase, not snake_case."""
    block = CONTRACT["wasm_valuations_subset"]
    mappings = block["python_js_map"] | block.get("python_path_js_map", {})
    for js_name in mappings.values():
        assert "_" not in js_name, f"WASM name must not be snake_case: {js_name!r}"


def test_wasm_portfolio_exports_match_contract() -> None:
    """`exports/portfolio.js` root keys must match [wasm_portfolio_subset]."""
    block = CONTRACT["wasm_portfolio_subset"]
    js_path = (CONTRACT_PATH.parent / block["js_export_file"]).resolve()
    expected = set(block["root_exports"])
    actual = _parse_exported_const_object_keys(js_path, "portfolio")
    assert actual == expected, (
        "portfolio.js exports diverged from contract.\n"
        f"  missing from JS: {sorted(expected - actual)}\n"
        f"  unlisted in contract: {sorted(actual - expected)}"
    )


def test_wasm_portfolio_python_js_map_matches_facade_exports() -> None:
    """Pinned portfolio Python-to-JS mappings must resolve on the facade."""
    block = CONTRACT["wasm_portfolio_subset"]
    root_exports = set(block["root_exports"])
    for python_name, js_name in block["python_js_map"].items():
        assert js_name in root_exports, f"python_js_map[{python_name!r}] -> {js_name!r} not in root_exports"


def test_wasm_portfolio_python_twins_resolve() -> None:
    """Every mapped portfolio Python twin must resolve on the live package."""
    block = CONTRACT["wasm_portfolio_subset"]
    module = importlib.import_module("finstack_quant.portfolio")
    missing = [name for name in block["python_js_map"] if not hasattr(module, name)]
    assert not missing, f"finstack_quant.portfolio missing mapped twins: {missing}"


def test_wasm_portfolio_root_exports_are_triplet_accounted_for() -> None:
    """Every portfolio root export is mapped from Python or listed WASM-only."""
    block = CONTRACT["wasm_portfolio_subset"]
    root_exports = set(block["root_exports"])
    wasm_only = set(block.get("wasm_only", []))
    mapped_js = set(block["python_js_map"].values())
    _assert_wasm_subset_root_exports_accounted(
        "wasm_portfolio_subset",
        root_exports=root_exports,
        mapped_js=mapped_js,
        wasm_only=wasm_only,
    )


def test_wasm_portfolio_python_only_excludes_wasm_map() -> None:
    """Portfolio Python-only symbols must not appear in the WASM map."""
    block = CONTRACT["wasm_portfolio_subset"]
    overlap = set(block["python_only"]) & set(block["python_js_map"])
    assert not overlap, f"python_only overlaps python_js_map keys: {sorted(overlap)}"


def test_wasm_portfolio_python_surface_is_exhaustively_accounted_for() -> None:
    """Every portfolio Python symbol is mapped to WASM or explicitly Python-only."""
    block = CONTRACT["wasm_portfolio_subset"]
    public = set(CONTRACT["crates"]["portfolio"]["symbols"]["public"])
    mapped_python = set(block["python_js_map"])
    python_only = set(block["python_only"])
    assert not mapped_python & python_only, "mapped Python names cannot also be Python-only"
    accounted = mapped_python | python_only
    assert accounted == public, (
        "portfolio Python/WASM accounting diverged from the canonical Python surface.\n"
        f"  unaccounted Python symbols: {sorted(public - accounted)}\n"
        f"  non-public accounting entries: {sorted(accounted - public)}"
    )


def test_wasm_portfolio_python_js_names_use_camel_or_pascal_case() -> None:
    """Portfolio WASM names should be camelCase or PascalCase, not snake_case."""
    block = CONTRACT["wasm_portfolio_subset"]
    for js_name in block["python_js_map"].values():
        assert "_" not in js_name, f"WASM name must not be snake_case: {js_name!r}"


def test_wasm_core_exports_match_contract() -> None:
    """``exports/core.js`` root keys must match [wasm_core_subset]."""
    block = CONTRACT["wasm_core_subset"]
    js_path = (CONTRACT_PATH.parent / block["js_export_file"]).resolve()
    expected = set(block["root_exports"])
    actual = _parse_exported_const_object_keys(js_path, "core")
    assert actual == expected, (
        f"core.js exports diverged from contract.\n"
        f"  missing from JS: {sorted(expected - actual)}\n"
        f"  unlisted in contract: {sorted(actual - expected)}"
    )


def test_wasm_core_python_only_market_data_not_on_facade() -> None:
    """Python-only market_data symbols must not appear on the WASM core facade."""
    block = CONTRACT["wasm_core_subset"]
    js_path = (CONTRACT_PATH.parent / block["js_export_file"]).resolve()
    actual = _parse_exported_const_object_keys(js_path, "core")
    overlap = set(block["python_only_market_data"]) & actual
    assert not overlap, f"python_only_market_data must not be on core.js: {sorted(overlap)}"


def test_wasm_core_market_data_types_on_facade() -> None:
    """WASM core exposes the agreed market_data type subset from the contract."""
    block = CONTRACT["wasm_core_subset"]
    js_path = (CONTRACT_PATH.parent / block["js_export_file"]).resolve()
    actual = _parse_exported_const_object_keys(js_path, "core")
    public = set(CONTRACT["crates"]["core"]["market_data"]["public"])
    python_only = set(block["python_only_market_data"])
    js_map = block.get("python_js_map", {})
    expected_on_wasm = {js_map.get(f"market_data.{name}", name) for name in public - python_only}
    missing = expected_on_wasm - actual
    assert not missing, f"market_data WASM subset missing from core.js: {sorted(missing)}"


def test_wasm_core_root_exports_accounted() -> None:
    """Every core root export is pinned in python_js_map or listed wasm_only."""
    block = CONTRACT["wasm_core_subset"]
    _assert_wasm_subset_root_exports_accounted(
        "wasm_core_subset",
        root_exports=set(block["root_exports"]),
        mapped_js=set(block["python_js_map"].values()),
        wasm_only=set(block["wasm_only"]),
    )


def test_wasm_core_python_js_map_paths_resolve() -> None:
    """Each dotted python_js_map key must resolve under finstack_quant.core."""
    block = CONTRACT["wasm_core_subset"]
    core = importlib.import_module("finstack_quant.core")
    missing = []
    for dotted in block["python_js_map"]:
        obj: Any = core
        for part in dotted.split("."):
            obj = getattr(obj, part, None)
            if obj is None:
                missing.append(dotted)
                break
    assert not missing, f"finstack_quant.core missing python_js_map twins: {missing}"


def test_wasm_core_python_only_lists_stay_within_contracted_surfaces() -> None:
    """python_only_dates/types entries must remain contracted Python symbols."""
    block = CONTRACT["wasm_core_subset"]
    core_crate = CONTRACT["crates"]["core"]
    dates_public = set(core_crate["module_symbols"]["dates"]["public"])
    types_public = set(core_crate["module_symbols"]["types"]["public"])
    stray_dates = set(block["python_only_dates"]) - dates_public
    stray_types = set(block["python_only_types"]) - types_public
    assert not stray_dates, f"python_only_dates not in contracted dates surface: {sorted(stray_dates)}"
    assert not stray_types, f"python_only_types not in contracted types surface: {sorted(stray_types)}"


def test_wasm_core_curve_member_pins_match_binding_source() -> None:
    """Pinned curve/cube members must use their canonical wasm-bindgen JS names."""
    block = CONTRACT["wasm_core_subset"]
    wasm_src = (
        CONTRACT_PATH.parent.parent / "finstack-quant-wasm" / "src" / "api" / "core" / "market_data.rs"
    ).read_text()

    for member_key in [
        "discount_curve_members",
        "hazard_curve_members",
        "forward_curve_members",
        "vol_cube_members",
        "fx_delta_vol_surface_members",
    ]:
        for js_name in block[member_key]:
            assert (
                f"js_name = {js_name}" in wasm_src
                or f"pub fn {js_name}(" in wasm_src
                or f"pub fn {re.sub(r'(?<!^)(?=[A-Z])', '_', js_name).lower()}(" in wasm_src
            ), f"WASM binding source is missing pinned member {js_name!r}"


def test_python_curve_phase4_members_are_live() -> None:
    """Python mirrors the canonical Rust Phase 4 curve metadata."""
    module = importlib.import_module("finstack_quant.core.market_data")
    for class_name, members in {
        "DiscountCurve": ["forward"],
        "ForwardCurve": ["rate_between", "projection_grid", "reset_lag"],
    }.items():
        cls = getattr(module, class_name)
        for member in members:
            assert hasattr(cls, member), f"{class_name} missing {member}"


def test_models_credit_migration_member_pins_resolve() -> None:
    """Pinned GeneratorMatrix diagnostics must remain live Python members."""
    members = CONTRACT["crates"]["models"]["credit_migration_members"]
    module = importlib.import_module("finstack_quant.models.credit.migration")

    for key, python_name in members.items():
        class_name, _, canonical_member = key.partition(".")
        cls = getattr(module, class_name, None)
        assert cls is not None, f"credit.migration missing class {class_name}"
        assert python_name == canonical_member
        assert hasattr(cls, python_name), f"{class_name} missing pinned member `{python_name}`"


def test_core_market_data_public_matches_contract() -> None:
    """``finstack_quant.core.market_data.__all__`` must match [crates.core.market_data]."""
    block = CONTRACT["crates"]["core"]["market_data"]
    expected = block["public"]
    module = importlib.import_module(block["python_package"])
    assert module.__all__ == expected, (
        f"{block['python_package']}.__all__ diverged from contract.\n"
        f"  missing: {sorted(set(expected) - set(module.__all__))}\n"
        f"  unlisted: {sorted(set(module.__all__) - set(expected))}"
    )

    root_stub = CONTRACT_PATH.parent / "finstack_quant" / "core" / "market_data" / "__init__.pyi"
    assert _pyi_all_names(root_stub) == expected


@pytest.mark.parametrize(
    "submodule_name",
    ["curves", "fx", "context", "scalars"],
)
def test_core_market_data_submodule_stubs_match_runtime(submodule_name: str) -> None:
    """Each runtime market-data submodule must have an exact stub ``__all__``."""
    package = "finstack_quant.core.market_data"
    module = importlib.import_module(f"{package}.{submodule_name}")
    stub = CONTRACT_PATH.parent / "finstack_quant" / "core" / "market_data" / f"{submodule_name}.pyi"
    assert stub.exists(), f"missing stub for {package}.{submodule_name}"
    assert _pyi_all_names(stub) == module.__all__


def test_core_market_data_scalars_exports_are_explicit() -> None:
    """The scalars submodule exports its canonical types at both package levels."""
    root = importlib.import_module("finstack_quant.core.market_data")
    scalars = importlib.import_module("finstack_quant.core.market_data.scalars")

    assert scalars.__all__ == ["InflationIndex", "ScalarTimeSeries"]
    assert root.ScalarTimeSeries is scalars.ScalarTimeSeries
    assert root.InflationIndex is scalars.InflationIndex


def test_models_correlation_public_matches_contract() -> None:
    """``finstack_quant.models.correlation.__all__`` must match [crates.models.correlation].

    Pins the correlation symbol surface so a binding rename (e.g. the
    Rust-canonical ``LatentFactorKind``) cannot drift from the contract, the
    package ``__all__``, or the importable surface without failing parity.
    """
    block = CONTRACT["crates"]["models"]["correlation"]
    expected = block["public"]
    module = importlib.import_module(block["python_package"])
    assert module.__all__ == expected, (
        f"{block['python_package']}.__all__ diverged from contract.\n"
        f"  missing: {sorted(set(expected) - set(module.__all__))}\n"
        f"  unlisted: {sorted(set(module.__all__) - set(expected))}"
    )
    for name in expected:
        assert hasattr(module, name), f"{block['python_package']} does not expose `{name}`"


def test_valuations_instruments_public_matches_contract() -> None:
    """``finstack_quant.valuations.instruments.__all__`` must match [crates.valuations.instruments]."""
    block = CONTRACT["crates"]["valuations"]["instruments"]
    expected = block["public"]
    module = importlib.import_module(block["python_package"])
    assert module.__all__ == expected, (
        f"{block['python_package']}.__all__ diverged from contract.\n"
        f"  missing: {sorted(set(expected) - set(module.__all__))}\n"
        f"  unlisted: {sorted(set(module.__all__) - set(expected))}"
    )
    for name in expected:
        assert hasattr(module, name), f"{block['python_package']} does not expose `{name}`"


@pytest.mark.parametrize(
    "contract_path",
    [
        ("models", "credit"),
        ("models", "factor"),
        ("models", "correlation"),
        ("models", "monte_carlo"),
        ("models", "rates"),
        ("models", "rates", "dtsm"),
        ("models", "volatility"),
    ],
)
def test_models_nested_public_matches_contract(contract_path: tuple[str, ...]) -> None:
    """Rust-shaped nested model modules must keep their pinned Python surface."""
    block: dict[str, Any] = CONTRACT["crates"]
    for key in contract_path:
        block = block[key]
    expected = block["public"]
    module = importlib.import_module(block["python_package"])
    assert module.__all__ == expected, (
        f"{block['python_package']}.__all__ diverged from contract.\n"
        f"  missing: {sorted(set(expected) - set(module.__all__))}\n"
        f"  unlisted: {sorted(set(module.__all__) - set(expected))}"
    )
    for name in expected:
        assert hasattr(module, name), f"{block['python_package']} does not expose `{name}`"


def test_dtsm_removed_from_core_market_data() -> None:
    """DTSM must have one canonical host path under models."""
    core_market_data = importlib.import_module("finstack_quant.core.market_data")
    assert not hasattr(core_market_data, "dtsm")
    with pytest.raises(ModuleNotFoundError):
        importlib.import_module("finstack_quant.core.market_data.dtsm")


def test_volatility_models_have_one_canonical_host_namespace() -> None:
    """Computational volatility APIs must live only under models.volatility."""
    models = importlib.import_module("finstack_quant.models")
    volatility = importlib.import_module("finstack_quant.models.volatility")
    core_market_data = importlib.import_module("finstack_quant.core.market_data")

    for name in ("SabrParameters", "SabrModel", "SabrSmile", "SabrCalibrator"):
        assert hasattr(volatility, name)
        assert not hasattr(models, name)
    for artifact_name, removed_methods in {
        "VolSurface": ("value_checked", "value_clamped"),
        "VolCube": ("vol", "vol_clamped", "vol_normal", "vol_normal_clamped"),
        "FxDeltaVolSurface": ("implied_vol", "pillar_vols", "to_vol_surface"),
    }.items():
        artifact = getattr(core_market_data, artifact_name)
        assert not [name for name in removed_methods if hasattr(artifact, name)]
    assert not hasattr(core_market_data, "arbitrage")
    with pytest.raises(ModuleNotFoundError):
        importlib.import_module("finstack_quant.core.market_data.arbitrage")


def test_wasm_models_rates_dtsm_map_matches_leaf_facade() -> None:
    """The moved DTSM triplet must resolve in Python and the WASM leaf facade."""
    mapping = CONTRACT["wasm_models_subset"]["rates_dtsm_python_js_map"]
    python_module = importlib.import_module("finstack_quant.models.rates.dtsm")
    assert not [name for name in mapping if not hasattr(python_module, name)]

    leaf = CONTRACT_PATH.parent.parent / "finstack-quant-wasm" / "exports" / "models" / "rates" / "dtsm.js"
    assert _parse_exported_const_object_keys(leaf, "dtsm") == set(mapping.values())


def test_models_correlation_member_pins_resolve_in_both_hosts() -> None:
    """[wasm_models_subset.correlation_members] pins shared class members.

    Export-name pins cannot see method drift on classes exposed in both
    hosts, so each pinned member is checked against (a) the live Python
    class attribute and (b) the WASM binding source (`js_name = "..."` on
    the wasm-bindgen attribute, or a plain `pub fn` for identical names).
    """
    members = CONTRACT["wasm_models_subset"]["correlation_members"]
    module = importlib.import_module("finstack_quant.models.correlation")
    wasm_src = (
        CONTRACT_PATH.parent.parent / "finstack-quant-wasm" / "src" / "api" / "models" / "correlation" / "mod.rs"
    ).read_text()

    for key, js_name in members.items():
        class_name, _, python_member = key.partition(".")
        cls = getattr(module, class_name, None)
        assert cls is not None, f"finstack_quant.models.correlation missing class {class_name}"
        assert hasattr(cls, python_member), f"{class_name} missing Python member `{python_member}`"
        wasm_pin = f"js_name = {js_name}"
        assert wasm_pin in wasm_src or f"pub fn {js_name}(" in wasm_src, (
            f"WASM correlation binding missing `{js_name}` (pinned as {key}); "
            f"expected `{wasm_pin}` in finstack-quant-wasm/src/api/models/correlation/mod.rs"
        )


def test_correlated_bernoulli_requested_correlation_matches_rust_and_stub() -> None:
    """Python exposes the canonical Rust requested-correlation diagnostic."""
    module = importlib.import_module("finstack_quant.models.correlation")
    assert hasattr(module.CorrelatedBernoulli, "requested_correlation")

    stub = (CONTRACT_PATH.parent / "finstack_quant" / "models" / "correlation" / "__init__.pyi").read_text()
    assert "def requested_correlation(self) -> float:" in stub

    rust = (CONTRACT_PATH.parent.parent / "finstack-quant" / "core" / "src" / "math" / "probability.rs").read_text()
    assert "pub fn requested_correlation(&self) -> f64" in rust

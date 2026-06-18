"""Structural parity checks driven by ``finstack-quant-py/parity_contract.toml``."""

from __future__ import annotations

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
    """The Python parity contract should be stored in the Python package tree."""
    assert Path("finstack-quant-py/parity_contract.toml").resolve() == CONTRACT_PATH


def test_contract_uses_known_module_statuses() -> None:
    """Module status values should stay explicit and auditable."""
    unknown = [
        (crate_name, module_name, spec["status"])
        for crate_name, crate in CONTRACT["crates"].items()
        for module_name, spec in crate.get("modules", {}).items()
        if spec["status"] not in VALID_MODULE_STATUSES
    ]
    assert unknown == []


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


def test_pyi_top_level_matches_contract() -> None:
    """The `.pyi` stub, ``finstack_quant.__all__``, and the contract must agree.

    Drift in any of the three is a maintenance hazard, since they all encode
    the same fact (the public top-level subpackages of finstack_quant).
    """
    block = CONTRACT["pyi_top_level"]
    pyi_path = CONTRACT_PATH.parent / block["file"]
    contract = set(block["names"])
    pyi = _pyi_top_level_names(pyi_path)
    finstack_all = set(importlib.import_module("finstack_quant").__all__)

    assert pyi == contract, (
        f"finstack_quant.pyi top-level names diverged from contract.\n"
        f"  missing from .pyi: {sorted(contract - pyi)}\n"
        f"  unlisted in contract: {sorted(pyi - contract)}"
    )
    assert finstack_all == contract, (
        f"finstack_quant.__all__ diverged from contract.\n"
        f"  missing from finstack_quant.__all__: {sorted(contract - finstack_all)}\n"
        f"  unlisted in contract: {sorted(finstack_all - contract)}"
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
    # finstack_quant.valuations.correlation) are not part of this surface.
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
            key_match = re.match(r"^([A-Za-z_][A-Za-z0-9_]*)\s*:", stripped)
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
    unaccounted = root_exports - wasm_only - mapped_js
    assert not unaccounted, f"attribution exports must be mapped or wasm-only: {sorted(unaccounted)}"
    overlap = wasm_only & mapped_js
    assert not overlap, f"attribution wasm_only overlaps python_js_map values: {sorted(overlap)}"


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
    """`finstack_quant.cashflows` __all__ must equal the pinned python triplet names."""
    block = CONTRACT["wasm_cashflows_subset"]
    module = importlib.import_module("finstack_quant.cashflows")
    expected = set(block["python_js_map"])
    actual = set(module.__all__)
    assert actual == expected, (
        f"finstack_quant.cashflows __all__ diverged from contract python_js_map keys.\n"
        f"  missing from Python: {sorted(expected - actual)}\n"
        f"  unlisted in contract: {sorted(actual - expected)}"
    )
    missing = [name for name in expected if not callable(getattr(module, name, None))]
    assert not missing, f"finstack_quant.cashflows symbols not callable: {missing}"


def test_cashflows_cross_crate_symbols_recorded() -> None:
    """Cross-crate cashflows symbols must be pinned with their canonical crate."""
    cross = CONTRACT["crates"]["cashflows"]["cross_crate"]
    assert set(cross) == {"bond_from_cashflows_json"}
    entry = cross["bond_from_cashflows_json"]
    assert entry["rust_crate"] == "finstack-quant-valuations"
    rust_lib = (CONTRACT_PATH.parent / ".." / entry["rust_lib"]).resolve()
    assert rust_lib.exists(), f"cross_crate rust_lib path missing: {rust_lib}"
    assert "bond_from_cashflows_json" in rust_lib.read_text(), (
        "canonical bond_from_cashflows_json not found in declared rust_lib"
    )


WASM_NAMESPACE_SUBSETS = [
    ("wasm_features_subset", "features", "finstack_quant.features"),
    ("wasm_statements_subset", "statements", "finstack_quant.statements"),
    ("wasm_statements_analytics_subset", "statements_analytics", "finstack_quant.statements_analytics"),
]


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
    unaccounted = root_exports - wasm_only - mapped_js
    assert not unaccounted, f"{section} exports must be mapped or wasm-only: {sorted(unaccounted)}"
    overlap = wasm_only & mapped_js
    assert not overlap, f"{section} wasm_only overlaps python_js_map values: {sorted(overlap)}"


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


def test_wasm_valuations_python_js_map_matches_root_exports() -> None:
    """Pinned python->js map must resolve to root exports on the WASM facade."""
    block = CONTRACT["wasm_valuations_subset"]
    root_exports = set(block["root_exports"])
    for python_name, js_name in block["python_js_map"].items():
        assert js_name in root_exports, f"python_js_map[{python_name!r}] -> {js_name!r} not in root_exports"


def test_wasm_valuations_root_exports_are_triplet_accounted_for() -> None:
    """Every root export is pinned in python_js_map or listed wasm_only."""
    block = CONTRACT["wasm_valuations_subset"]
    root_exports = set(block["root_exports"])
    wasm_only = set(block.get("wasm_only", []))
    mapped_js = set(block["python_js_map"].values())
    unaccounted = root_exports - wasm_only - mapped_js
    assert not unaccounted, f"root_exports must appear in python_js_map or wasm_only: {sorted(unaccounted)}"
    overlap = wasm_only & mapped_js
    assert not overlap, f"wasm_only must not overlap python_js_map values: {sorted(overlap)}"


def test_wasm_valuations_python_only_excludes_wasm_map() -> None:
    """python_only symbols must not appear in the WASM python_js_map."""
    block = CONTRACT["wasm_valuations_subset"]
    python_only = set(block["python_only"])
    mapped_python = set(block["python_js_map"])
    overlap = python_only & mapped_python
    assert not overlap, f"python_only overlaps python_js_map keys: {sorted(overlap)}"


def test_wasm_valuations_python_js_names_use_camel_or_pascal_case() -> None:
    """WASM export names should be camelCase or PascalCase, not snake_case."""
    block = CONTRACT["wasm_valuations_subset"]
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
    expected_on_wasm = public - python_only
    missing = expected_on_wasm - actual
    assert not missing, f"market_data WASM subset missing from core.js: {sorted(missing)}"


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


def test_valuations_correlation_public_matches_contract() -> None:
    """``finstack_quant.valuations.correlation.__all__`` must match [crates.valuations.correlation].

    Pins the correlation symbol surface so a binding rename (e.g. the
    Rust-canonical ``LatentFactorKind``) cannot drift from the contract, the
    package ``__all__``, or the importable surface without failing parity.
    """
    block = CONTRACT["crates"]["valuations"]["correlation"]
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
        ("valuations", "instruments", "commodity"),
        ("valuations", "instruments", "credit_derivatives"),
        ("valuations", "instruments", "equity"),
        ("valuations", "instruments", "exotics"),
        ("valuations", "instruments", "fixed_income"),
        ("valuations", "instruments", "fx"),
        ("valuations", "instruments", "rates"),
        ("valuations", "models"),
        ("valuations", "models", "credit"),
    ],
)
def test_valuations_nested_public_matches_contract(contract_path: tuple[str, ...]) -> None:
    """Rust-shaped nested valuation modules must keep their pinned Python surface."""
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


def test_valuations_correlation_member_pins_resolve_in_both_hosts() -> None:
    """[wasm_valuations_subset.correlation_members] pins shared class members.

    Export-name pins cannot see method drift on classes exposed in both
    hosts, so each pinned member is checked against (a) the live Python
    class attribute and (b) the WASM binding source (`js_name = "..."` on
    the wasm-bindgen attribute, or a plain `pub fn` for identical names).
    """
    members = CONTRACT["wasm_valuations_subset"]["correlation_members"]
    module = importlib.import_module("finstack_quant.valuations.correlation")
    wasm_src = (
        CONTRACT_PATH.parent.parent / "finstack-quant-wasm" / "src" / "api" / "valuations" / "correlation" / "mod.rs"
    ).read_text()

    for key, js_name in members.items():
        class_name, _, python_member = key.partition(".")
        cls = getattr(module, class_name, None)
        assert cls is not None, f"finstack_quant.valuations.correlation missing class {class_name}"
        assert hasattr(cls, python_member), f"{class_name} missing Python member `{python_member}`"
        wasm_pin = f"js_name = {js_name}"
        assert wasm_pin in wasm_src or f"pub fn {js_name}(" in wasm_src, (
            f"WASM correlation binding missing `{js_name}` (pinned as {key}); "
            f"expected `{wasm_pin}` in finstack-quant-wasm/src/api/valuations/correlation/mod.rs"
        )

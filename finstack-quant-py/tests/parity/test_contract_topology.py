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


def test_cashflows_has_no_cross_crate_symbols() -> None:
    """Cashflows must not own helpers from other crates."""
    assert "cross_crate" not in CONTRACT["crates"]["cashflows"]


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


def test_correlated_bernoulli_requested_correlation_matches_rust_and_stub() -> None:
    """Python exposes the canonical Rust requested-correlation diagnostic."""
    module = importlib.import_module("finstack_quant.models.correlation")
    assert hasattr(module.CorrelatedBernoulli, "requested_correlation")

    stub = (CONTRACT_PATH.parent / "finstack_quant" / "models" / "correlation" / "__init__.pyi").read_text()
    assert "def requested_correlation(self) -> float:" in stub

    rust = (CONTRACT_PATH.parent.parent / "finstack-quant" / "core" / "src" / "math" / "probability.rs").read_text()
    assert "pub fn requested_correlation(&self) -> f64" in rust

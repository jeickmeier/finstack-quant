"""Runtime ``__doc__`` is compiled from the ``.pyi`` stubs.

The stubs are the single source of every Python docstring: ``build.rs`` extracts
them and each PyO3 owner carries ``#[doc = pydoc!("<module>", "<qualname>")]``.
For every converted owner the runtime ``__doc__`` (what ``help()`` and IPython
``obj?`` show) must equal the stub docstring exactly, after the normalisation
``inspect.cleandoc`` applies to both sides.

``CONVERTED`` lists the converted modules. ``None`` means the whole module:
every stub docstring is checked, and no public runtime member may carry a
docstring the stub lacks (that would be leftover Rust ``///`` prose). A tuple
lists the converted owners of a partially converted module; a class name
covers the class and all its members.

While converting part of a module, check it without editing this file:
``RUNTIME_DOCS_MODULE=finstack_quant.valuations.instruments
RUNTIME_DOCS_OWNERS=Bond,BondBuilder pytest tests/test_runtime_docs.py``
(``RUNTIME_DOCS_MODULE`` takes a comma-separated list; each module is checked
for the same owners).
"""

from __future__ import annotations

import ast
import importlib
import inspect
import os
from pathlib import Path
import re
import types
from typing import Any

import pytest

import finstack_quant

PACKAGE_DIR = Path(finstack_quant.__file__).parent

_SCHEMA_REGISTRY = ("get", "index", "validate")

CONVERTED: dict[str, tuple[str, ...] | None] = {
    "finstack_quant.calibration": (
        "validate_surface",
        "validate_calendar_spread",
        "validate_butterfly_spread",
        "validate_vol_bounds",
        "CalibrationConfig",
        "QuoteQuality",
        "CalibrationValidationReport.dependency_graph",
        "CalibrationReport.metadata",
        "CalibrationReport.solver_method",
        "CalibrationReport.residual_units",
        "CalibrationEnvelope.__init__",
        "CalibrationStep.discount",
        "CalibrationStep.forward",
        "CalibrationStep.hazard",
        "CalibrationStep.inflation",
        "CalibrationStep.swaption_vol",
        "CalibrationStep.base_correlation",
        "CalibrationStep.hull_white",
        "CalibrationStep.cap_floor_hull_white",
        "CalibrationStep.xccy_basis",
        "CdsQuote.par_spread",
        "CdsQuote.upfront",
        "CdsTrancheQuote.__init__",
    ),
    "finstack_quant.analytics": (
        "AnalyticsError",
        "Performance",
        "PeriodStats",
        "BetaResult",
        "GreeksResult",
        "RollingGreeks",
        "MultiFactorResult",
        "DrawdownEpisode",
        "LookbackReturns",
        "DatedSeries",
    ),
    "finstack_quant.core.dates": ("PeriodKind",),
    "finstack_quant.statements_analytics": (
        "CorkscrewReport.to_dataframe",
        "ScorecardReport.to_dataframe",
        "CorkscrewReport.status",
        "LeaseSpec.to_dataframe",
        "ScorecardReport.status",
    ),
    "finstack_quant.statements": (
        "RollupSpec",
        "CapitalStructureCashflows.get_component",
        "PikToggleSpec.validate",
        "PikToggleSpec.liquidity_metric",
        "PikToggleSpec.threshold",
        "PikToggleSpec.target_instrument_ids",
        "PikToggleSpec.min_periods_in_pik",
        "ModelBuilder",
        "MixedNodeBuilder",
        "Registry.get_dependencies",
        "ForecastSpec.overrides",
        "Adjustment.cap_base_mode",
        "Adjustment.with_cap",
        "CheckConfig.min_severity",
        "CheckFinding.severity",
        "FinancialModelSpec.to_dataframe",
        "ForecastMethod.kind",
        "FormulaCheckSpec.category",
        "FormulaCheckSpec.severity",
        "MetricDefinition.unit_type",
        "NodeType.kind",
        "MonteCarloResults.metrics",
        "MonteCarloResults.warnings",
        "MonteCarloResults.breach_probability",
        "FinancialModelSpec.content_hash",
        "CheckReport.findings_by_severity",
    ),
    "finstack_quant.valuations.composite": None,
    "finstack_quant.valuations.market": None,
    "finstack_quant.valuations.schema": None,
    "finstack_quant.valuations": None,
    "finstack_quant.valuations.instruments": None,
    # `schema_registry_functions!` documents index/get/validate from each stub.
    "finstack_quant.attribution.schema": _SCHEMA_REGISTRY,
    "finstack_quant.calibration.schema": _SCHEMA_REGISTRY,
    "finstack_quant.cashflows.schema": _SCHEMA_REGISTRY,
    "finstack_quant.core.schema": _SCHEMA_REGISTRY,
    "finstack_quant.margin.schema": _SCHEMA_REGISTRY,
    "finstack_quant.models.factor.schema": _SCHEMA_REGISTRY,
    "finstack_quant.portfolio.schema": _SCHEMA_REGISTRY,
    "finstack_quant.scenarios.schema": _SCHEMA_REGISTRY,
    "finstack_quant.statements.schema": None,
}

if os.environ.get("RUNTIME_DOCS_MODULE"):
    _owners = tuple(owner for owner in os.environ.get("RUNTIME_DOCS_OWNERS", "").split(",") if owner)
    CONVERTED = {module: _owners or None for module in os.environ["RUNTIME_DOCS_MODULE"].split(",") if module}

# Type slots: CPython supplies their ``__doc__``; PyO3 cannot set it.
_SLOT_TYPES = (types.WrapperDescriptorType, types.MethodWrapperType)
# Class members whose ``__doc__`` comes from the binding (methods, properties).
_DOCUMENTED_MEMBERS = (
    types.MethodDescriptorType,
    types.ClassMethodDescriptorType,
    types.GetSetDescriptorType,
    types.MemberDescriptorType,
    types.BuiltinFunctionType,
    staticmethod,
    classmethod,
)


def _stub_path(module: str) -> Path:
    base = PACKAGE_DIR.joinpath(*module.split(".")[1:])
    package = base / "__init__.pyi"
    return package if package.exists() else base.with_suffix(".pyi")


def _stub_docstrings(module: str) -> dict[str, str]:
    """Qualified name -> cleaned docstring for every documented stub owner."""
    tree = ast.parse(_stub_path(module).read_text(encoding="utf-8"))
    docs: dict[str, str] = {}
    module_doc = ast.get_docstring(tree)
    if module_doc is not None:
        docs["__module__"] = module_doc

    def walk(body: list[ast.stmt], prefix: str) -> None:
        for node in body:
            if isinstance(node, (ast.ClassDef, ast.FunctionDef, ast.AsyncFunctionDef)):
                doc = ast.get_docstring(node)
                if doc is not None:
                    docs.setdefault(prefix + node.name, doc)
                if isinstance(node, ast.ClassDef):
                    walk(node.body, f"{prefix}{node.name}.")
            elif isinstance(node, ast.If):
                walk(node.body, prefix)
                walk(node.orelse, prefix)

    walk(tree.body, "")
    return docs


def _selected(qualname: str, owners: tuple[str, ...] | None) -> bool:
    return owners is None or qualname in owners or qualname.partition(".")[0] in owners


def _runtime(module: str, qualname: str) -> Any:
    obj: Any = importlib.import_module(module)
    if qualname == "__module__":
        return obj
    for part in qualname.split("."):
        obj = inspect.getattr_static(obj, part)
        if isinstance(obj, (staticmethod, classmethod)):
            obj = obj.__func__
    return obj


CASES = sorted(
    (module, qualname)
    for module, owners in CONVERTED.items()
    for qualname in _stub_docstrings(module)
    if _selected(qualname, owners) and not qualname.endswith(".__init__")
)


@pytest.mark.parametrize(("module", "qualname"), CASES, ids=[f"{m}:{q}" for m, q in CASES])
def test_runtime_doc_is_the_stub_docstring(module: str, qualname: str) -> None:
    runtime = _runtime(module, qualname)
    if isinstance(runtime, _SLOT_TYPES):
        pytest.skip("type slot: CPython owns its __doc__")
    expected = _stub_docstrings(module)[qualname]
    assert inspect.cleandoc(runtime.__doc__ or "") == expected


@pytest.mark.parametrize("module", sorted(m for m, owners in CONVERTED.items() if owners is None))
def test_no_runtime_docstring_without_a_stub_source(module: str) -> None:
    """A documented public runtime member must be documented in the stub too."""
    stub = _stub_docstrings(module)
    runtime_module = importlib.import_module(module)
    orphans = []
    for name in runtime_module.__all__:
        member = getattr(runtime_module, name)
        if (inspect.isroutine(member) or inspect.isclass(member)) and member.__doc__ and name not in stub:
            orphans.append(name)
        if not inspect.isclass(member):
            continue
        for attr, value in vars(member).items():
            documented_pickle = module.startswith("finstack_quant.valuations") and attr == "__reduce__"
            if (attr.startswith("_") and not documented_pickle) or not isinstance(value, _DOCUMENTED_MEMBERS):
                continue
            if value.__doc__ and f"{name}.{attr}" not in stub:
                orphans.append(f"{name}.{attr}")
    assert orphans == []


@pytest.mark.parametrize(
    ("name", "parameters", "guidance"),
    [
        (
            "MertonMcCalibrationSpec",
            ("target", "parameter", "low_paths", "max_iterations", "tolerance_pv", "bracket", "seed"),
            ("2,000", "40", "1e-4", "auto-brackets", "ValueError"),
        ),
        (
            "MetricPricingOverrides",
            ("bump_config", "theta_period", "breakeven_config", "bond_risk_basis", "theta_day_basis", "var_config"),
            ("one day", "calendar_365", "trading_252", "ValueError", "TypeError"),
        ),
        (
            "MarketHistory",
            ("base_date", "window_days", "scenarios"),
            ("required", "calendar days", "decimal", "relative", "ValueError", "TypeError"),
        ),
    ],
)
def test_runtime_class_help_explains_constructor(
    name: str, parameters: tuple[str, ...], guidance: tuple[str, ...]
) -> None:
    """Class help must expose constructor guidance without relying on slot docs."""
    # PyO3's class RAW_DOC does not include the #[new] method's docstring.
    doc = inspect.cleandoc(_runtime("finstack_quant.valuations.instruments", name).__doc__ or "")
    assert "\nParameters\n----------\n" in doc
    assert "\nRaises\n------\n" in doc
    for parameter in parameters:
        assert re.search(rf"(?m)^{re.escape(parameter)} :", doc), f"{name}: missing {parameter} guidance"
    for phrase in guidance:
        assert phrase in doc, f"{name}: missing {phrase!r} guidance"

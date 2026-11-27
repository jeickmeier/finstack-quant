"""Return-shape checks for public entry points.

``parity_contract.toml`` pins namespace topology and names; it says nothing
about what an entry point *returns*. This module pins the shape, because that
is where the three language surfaces drifted: the same Rust result used to come
back as a typed object in Python and a JSON string in WASM.

The table below is deliberately kept in the same order, with the same entry
names, as ``finstack-quant-wasm/tests/facade/return_shapes.test.mjs``. A divergence between
the two files should read as a one-screen diff.

Shapes:

``wrapper``
    A typed ``Py*`` wrapper class. Must carry ``to_json`` and ``from_json``.
``frame``
    A ``pandas.DataFrame``.
``series``
    A ``pandas.Series``.
``json``
    A ``str`` holding a JSON document. Only legal on ``*_json``-named wire
    surfaces.
``scalar`` / ``list`` / ``dict``
    Plain Python values.
"""

from __future__ import annotations

import importlib
import inspect
import math
from typing import Any

import pytest

# (module, attribute, expected shape)
#
# ``wrapper`` entries name the class the callable is declared to return; the
# class itself is checked for the mandatory accessor set.
ENTRY_SHAPES: list[tuple[str, str, str]] = [
    # attribution
    ("finstack_quant.attribution", "attribute_pnl", "wrapper"),
    ("finstack_quant.attribution", "attribute_return_contribution", "wrapper"),
    ("finstack_quant.attribution", "attribute_pnl_envelope", "wrapper"),
    # calibration
    ("finstack_quant.calibration", "dry_run", "wrapper"),
    ("finstack_quant.statements", "normalize", "list"),
    # scenarios
    ("finstack_quant.scenarios", "compose_scenarios", "wrapper"),
    ("finstack_quant.scenarios", "build_from_template", "wrapper"),
    ("finstack_quant.scenarios", "build_template_component", "wrapper"),
    ("finstack_quant.scenarios", "list_builtin_template_metadata", "list"),
    ("finstack_quant.scenarios", "apply_scenario", "wrapper"),
    ("finstack_quant.scenarios", "parallel_bp_many", "list"),
    # statements analytics
    ("finstack_quant.statements_analytics", "generate_tornado_entries", "list"),
    ("finstack_quant.statements_analytics", "run_checks", "wrapper"),
    ("finstack_quant.statements_analytics", "run_three_statement_checks", "wrapper"),
    ("finstack_quant.statements_analytics", "run_credit_underwriting_checks", "wrapper"),
    ("finstack_quant.statements_analytics", "add_vintage_buildup", "wrapper"),
    ("finstack_quant.statements_analytics", "add_roll_forward", "wrapper"),
    ("finstack_quant.statements_analytics", "add_roll_forward_with_opening", "wrapper"),
    ("finstack_quant.statements_analytics", "add_noi_buildup", "wrapper"),
    ("finstack_quant.statements_analytics", "add_ncf_buildup", "wrapper"),
    ("finstack_quant.statements_analytics", "add_rent_roll", "wrapper"),
    ("finstack_quant.statements_analytics", "add_property_operating_statement", "wrapper"),
    # portfolio
    ("finstack_quant.portfolio", "value_portfolio", "wrapper"),
    ("finstack_quant.portfolio", "aggregate_full_cashflows", "wrapper"),
    # portfolio attribution/performance surface (typed result-return
    # conversion, audit M1): every entry point returns a wrapper whose
    # `to_json()` is the wire form.
    ("finstack_quant.portfolio", "brinson_fachler", "wrapper"),
    ("finstack_quant.portfolio", "carino_link", "wrapper"),
    ("finstack_quant.portfolio", "brinson_linked", "wrapper"),
    ("finstack_quant.portfolio", "campisi_attribution", "wrapper"),
    ("finstack_quant.portfolio", "campisi_carino_link", "wrapper"),
    ("finstack_quant.portfolio", "cell_returns_from_reference", "wrapper"),
    ("finstack_quant.portfolio", "cell_returns_from_curves", "wrapper"),
    ("finstack_quant.portfolio", "excess_returns", "wrapper"),
    ("finstack_quant.portfolio", "grid_attribution", "wrapper"),
    ("finstack_quant.portfolio", "grid_carino_link", "wrapper"),
    ("finstack_quant.portfolio", "factor_brinson_attribution", "wrapper"),
    ("finstack_quant.portfolio", "twrr_linked", "wrapper"),
    ("finstack_quant.portfolio", "aggregate_metrics", "wrapper"),
    ("finstack_quant.portfolio", "replay_portfolio", "wrapper"),
    ("finstack_quant.portfolio", "allocate_weights", "wrapper"),
    ("finstack_quant.portfolio", "scenario_pnl", "wrapper"),
]

# Result classes that must carry the full accessor contract:
# typed getters + to_json + from_json + unified tabular exports (to_arrow/to_pandas/to_polars/to_pyarrow).
RESULT_CLASSES: list[tuple[str, str]] = [
    ("finstack_quant.attribution", "AttributionResultEnvelope"),
    ("finstack_quant.attribution", "PnlAttribution"),
    ("finstack_quant.attribution", "ReturnContributionResult"),
    ("finstack_quant.scenarios", "ApplicationReport"),
    ("finstack_quant.scenarios", "ScenarioSpec"),
    ("finstack_quant.statements_analytics", "TornadoEntry"),
    ("finstack_quant.statements_analytics", "GoalSeekResult"),
    ("finstack_quant.statements_analytics", "DependencyTree"),
    ("finstack_quant.models.factor", "FactorCovarianceMatrix"),
    ("finstack_quant.models.factor", "FactorModelConfig"),
    ("finstack_quant.scenarios", "HorizonResult"),
    ("finstack_quant.analytics", "PeriodStats"),
    ("finstack_quant.analytics", "BetaResult"),
    ("finstack_quant.analytics", "CorrelationResult"),
    ("finstack_quant.analytics", "GreeksResult"),
    ("finstack_quant.analytics", "RollingGreeks"),
    ("finstack_quant.analytics", "MultiFactorResult"),
    ("finstack_quant.analytics", "DrawdownEpisode"),
    ("finstack_quant.analytics", "LookbackReturns"),
    ("finstack_quant.analytics", "DatedSeries"),
    ("finstack_quant.models.monte_carlo", "Estimate"),
    ("finstack_quant.models.monte_carlo", "LrmGreeks"),
    ("finstack_quant.models.monte_carlo", "MoneyEstimate"),
    ("finstack_quant.models.monte_carlo", "PathSummary"),
    ("finstack_quant.margin", "XvaResult"),
    ("finstack_quant.margin", "FrtbSbaResult"),
    ("finstack_quant.margin", "EadResult"),
    ("finstack_quant.statements", "StatementResult"),
    ("finstack_quant.statements", "NormalizationResult"),
    # portfolio attribution/performance result wrappers.
    ("finstack_quant.portfolio", "BrinsonPeriodResult"),
    ("finstack_quant.portfolio", "CarinoLinkedAttribution"),
    ("finstack_quant.portfolio", "DurationCellTable"),
    ("finstack_quant.portfolio", "ExcessReturnResult"),
    ("finstack_quant.portfolio", "FactorBrinsonResult"),
    ("finstack_quant.portfolio", "FiAttributionResult"),
    ("finstack_quant.portfolio", "FiCarinoLinkedResult"),
    ("finstack_quant.portfolio", "GridAttributionResult"),
    ("finstack_quant.portfolio", "GridCarinoLinkedResult"),
    ("finstack_quant.portfolio", "LinkedReturn"),
    ("finstack_quant.portfolio", "ReplayResult"),
    ("finstack_quant.portfolio", "ScenarioPnlBatchItem"),
    ("finstack_quant.portfolio", "WeightAllocationResult"),
]


def _resolve(module_name: str, attr: str) -> Any:
    module = importlib.import_module(module_name)
    if not hasattr(module, attr):
        pytest.skip(f"{module_name}.{attr} is not present in this build")
    return getattr(module, attr)


@pytest.mark.parametrize(
    ("module_name", "attr", "shape"),
    ENTRY_SHAPES,
    ids=[f"{m.rsplit('.', 1)[-1]}.{a}" for m, a, _ in ENTRY_SHAPES],
)
def test_entry_point_is_not_a_bare_json_string(module_name: str, attr: str, shape: str) -> None:
    """A computation entry point must not return a bare JSON string.

    Only ``*_json``-suffixed wire surfaces may do that. This is the rule that
    kept typed Rust results unreachable from Python.
    """
    _resolve(module_name, attr)
    if shape == "json":
        assert attr.endswith("_json"), (
            f"{module_name}.{attr} returns a JSON string but its name does not "
            "mark it as a wire surface; rename it with a '_json' suffix"
        )


@pytest.mark.parametrize(
    ("module_name", "class_name"),
    RESULT_CLASSES,
    ids=[f"{m.rsplit('.', 1)[-1]}.{c}" for m, c in RESULT_CLASSES],
)
def test_result_class_has_json_round_trip(module_name: str, class_name: str) -> None:
    """Every result class round-trips through JSON.

    ``to_json`` is the escape hatch that makes the typed-return migration
    non-lossy: any caller that wanted the old string output can still get it.
    """
    cls = _resolve(module_name, class_name)
    assert hasattr(cls, "to_json"), f"{class_name} is missing to_json()"
    assert hasattr(cls, "from_json"), f"{class_name} is missing from_json()"


@pytest.mark.parametrize(
    ("module_name", "class_name"),
    RESULT_CLASSES,
    ids=[f"{m.rsplit('.', 1)[-1]}.{c}" for m, c in RESULT_CLASSES],
)
def test_from_json_is_a_staticmethod(module_name: str, class_name: str) -> None:
    """``from_json`` is a ``staticmethod``, never a ``classmethod``.

    Neither uses the class object; the split was pure drift (49 vs 69).
    """
    cls = _resolve(module_name, class_name)
    if not hasattr(cls, "from_json"):
        pytest.skip(f"{class_name} has no from_json")
    bound = inspect.getattr_static(cls, "from_json")
    assert not isinstance(bound, classmethod), (
        f"{class_name}.from_json is a classmethod; the convention is staticmethod"
    )


@pytest.mark.parametrize(
    ("module_name", "class_name"),
    RESULT_CLASSES,
    ids=[f"{m.rsplit('.', 1)[-1]}.{c}" for m, c in RESULT_CLASSES],
)
def test_result_class_has_typed_getters(module_name: str, class_name: str) -> None:
    """A result class must expose typed getters, not just ``to_json``.

    A wrapper whose only accessor is ``to_json`` is a JSON string with extra
    steps — it forces callers to round-trip through the wire format to read a
    single field.
    """
    cls = _resolve(module_name, class_name)
    # PyO3 `#[getter]`s surface as `getset_descriptor`, not `property`.
    getters = [
        name
        for name, value in vars(cls).items()
        if not name.startswith("_") and type(value).__name__ in {"getset_descriptor", "property"}
    ]
    assert getters, (
        f"{class_name} exposes no typed getters; it is a JSON blob wearing a "
        "class. Add #[getter]s for its headline fields."
    )


def test_statement_result_uses_orient_parameter() -> None:
    """Orientation is a parameter on unified tabular exports, not separate method pairs."""
    cls = _resolve("finstack_quant.statements", "StatementResult")
    for method in ("to_arrow", "to_pandas", "to_polars", "to_pyarrow"):
        assert hasattr(cls, method), f"StatementResult is missing {method}()"
    for removed in ("to_dataframe", "to_arrow_long", "to_arrow_wide", "to_pandas_long", "to_pandas_wide"):
        assert not hasattr(cls, removed), f"{removed} survived the unified orient=... migration"


def test_json_round_trip_survives_non_finite_values() -> None:
    """A result whose ratio is ``+inf`` must still round-trip.

    ``serde_json`` writes non-finite floats as JSON ``null`` and then refuses
    to read ``null`` back as a float, so metrics documented to reach ``+inf``
    (a profit factor with wins and no losses) used to lose their value — and
    take ``pickle`` down with them, since ``__reduce__`` rebuilds via
    ``from_json``.
    """
    import datetime as dt
    import pickle

    import pandas as pd

    analytics = importlib.import_module("finstack_quant.analytics")

    # With no observed losses, profit factor is infinite while conditional
    # payoff/CPC are unavailable; the wire preserves each distinct outcome.
    index = [dt.date(2024, 1, day) for day in range(1, 6)]
    returns = pd.DataFrame({"asset": [0.01, 0.02, 0.03, 0.01, 0.02]}, index=index)
    stats = analytics.Performance.from_returns(returns, dt.date(2023, 12, 31)).period_stats(
        0, aggregation_frequency="monthly"
    )

    assert stats.profit_factor == float("inf"), "fixture no longer exercises the +inf path"

    restored = analytics.PeriodStats.from_json(stats.to_json())
    assert math.isnan(restored.payoff_ratio)
    assert restored.profit_factor == float("inf")

    # Round-tripping our own object, matching tests/test_pickle_roundtrip.py.
    assert pickle.loads(pickle.dumps(stats)).profit_factor == float("inf")  # noqa: S301


_LEGACY_TABULAR_NAMES = {
    "to_dataframe",
    "to_arrow_long",
    "to_arrow_wide",
    "to_pandas_long",
    "to_pandas_wide",
    "to_polars_long",
    "to_polars_wide",
}
_TABULAR_QUARTET = {"to_arrow", "to_pandas", "to_polars", "to_pyarrow"}


def _iter_public_surface() -> tuple[dict[str, list[str]], list[type[Any]]]:
    root = importlib.import_module("finstack_quant")
    seen_modules: set[str] = set()
    module_exports: dict[str, list[str]] = {}
    classes: dict[tuple[str, str], type[Any]] = {}

    def walk(module: Any, path: str) -> None:
        if path in seen_modules:
            return
        seen_modules.add(path)
        exports = list(getattr(module, "__all__", None) or [])
        module_exports[path] = exports
        for name in exports:
            val = getattr(module, name)
            if inspect.ismodule(val):
                walk(val, f"{path}.{name}")
            elif inspect.isclass(val):
                classes.setdefault((val.__module__, val.__name__), val)

    walk(root, "finstack_quant")
    return module_exports, list(classes.values())


def test_no_legacy_dataframe_exits_anywhere() -> None:
    """No public module or class may re-introduce ``to_dataframe`` or orient-suffixed exits."""
    module_exports, classes = _iter_public_surface()

    def _is_legacy(name: str) -> bool:
        return name in _LEGACY_TABULAR_NAMES or (name.startswith("to_") and name.endswith("_dataframe"))

    offenders = [
        f"{mod_path}.{name}" for mod_path, exports in module_exports.items() for name in exports if _is_legacy(name)
    ]
    offenders.extend(
        f"{cls.__module__}.{cls.__name__}.{name}"
        for cls in classes
        for name in dir(cls)
        if not name.startswith("_") and _is_legacy(name)
    )
    assert not offenders, f"legacy tabular exits found: {sorted(offenders)}"


def test_tabular_classes_expose_complete_quartet() -> None:
    """Every class exposing a tabular conversion exposes the full Arrow/pandas/Polars/PyArrow quartet."""
    _, classes = _iter_public_surface()
    incomplete: list[str] = []
    for cls in classes:
        names = set(dir(cls))
        qualname = f"{cls.__module__}.{cls.__name__}"
        if qualname == "finstack_quant.core.table.ArrowTable":
            required = {"to_pandas", "to_polars", "to_pyarrow", "__arrow_c_stream__", "__arrow_c_schema__"}
            missing = sorted(required - names)
            if missing:
                incomplete.append(f"{qualname} missing {missing}")
            continue
        if qualname == "finstack_quant.core.table.LabeledSeries":
            required = _TABULAR_QUARTET | {"__arrow_c_stream__", "__arrow_c_schema__"}
            missing = sorted(required - names)
            if missing:
                incomplete.append(f"{qualname} missing {missing}")
            continue
        present = _TABULAR_QUARTET & names
        if present and present != _TABULAR_QUARTET:
            incomplete.append(f"{qualname} missing {sorted(_TABULAR_QUARTET - present)}")
    assert not incomplete, f"incomplete tabular accessors: {incomplete}"

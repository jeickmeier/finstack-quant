"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements_analytics/analysis/runners.rs``."""

from __future__ import annotations

from collections.abc import Mapping
from datetime import date
from typing import Any, Literal, TypedDict
import pandas as pd
import polars as pl
from finstack_quant.statements import FinancialModelSpec, StatementResult
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.core.table import ArrowTable, LabeledSeries
from finstack_quant.statements_analytics._typed.results import (
    BridgeChart,
    ScenarioDiff,
    ScenarioResults,
    SensitivityResult,
    TornadoEntry,
    VarianceReport,
)

def run_sensitivity(model: Any, config: Any) -> SensitivityResult:
    """Run sensitivity analysis on a financial model.

    Each perturbation replaces its node's value in its one forecast period
    (Value > Forecast > Formula). It does not re-anchor the node's own
    forecast: later periods of a growth, seasonal or stochastic forecast keep
    their unshocked base. Formula nodes that read the shocked cell move, in
    its period and in later periods reached through ``lag``, ``ttm`` or
    ``cumsum``. Add one parameter per period to shock a whole forecast path.

    Parameters
    ----------
    model : FinancialModelSpec | str
        A ``FinancialModelSpec`` object or a JSON string.
    config : SensitivityConfig | str
        A typed configuration or its JSON serialization.

    Returns
    -------
    SensitivityResult
        Typed sensitivity result with per-scenario outputs, ``baseline`` and
        DataFrame exits.

    Raises
    ------
    ValueError
        If the configuration is malformed, has duplicate parameters or
        non-finite values, exceeds 128 parameters, 10,000 scenarios or
        10 million model node-period evaluation cells, references a missing
        perturbed parameter or target metric, or a scenario fails.
    KeyError
        If model evaluation cannot find required data or a formula reference.

    Examples
    --------
    >>> from finstack_quant.statements import ModelBuilder
    >>> from finstack_quant.statements_analytics import ParameterSpec, SensitivityConfig, run_sensitivity
    >>> b = ModelBuilder("m")
    >>> _ = b.periods("2025Q1..Q2", None)
    >>> _ = b.value("revenue", [("2025Q1", 100.0), ("2025Q2", 110.0)])
    >>> _ = b.compute("profit", "revenue * 0.5")
    >>> cfg = SensitivityConfig(
    ...     "diagonal", [ParameterSpec.with_percentages("revenue", "2025Q2", 110.0, [-10.0, 10.0])], ["profit"]
    ... )
    >>> len(run_sensitivity(b.build(), cfg))
    2
    """

def generate_tornado_entries(result: Any, metric_node: str, period: str) -> list[TornadoEntry] | None:
    """Generate tornado chart entries for a sensitivity result.

    Parameters
    ----------
    result : SensitivityResult | str
        A typed sensitivity result or its JSON serialization.
    metric_node : str
        Node to extract tornado entries for.
    period : str
        Period at which the metric is read in the baseline and every scenario
        (e.g. ``"2025Q4"``). Required: a shock replaces one forecast cell, so
        the metric moves only in the shocked period and in later periods whose
        formulas read it.

    Returns
    -------
    list[TornadoEntry] or None
        Typed entries sorted by descending absolute swing, with parameter IDs
        preserving the perturbed ``node@period`` key; ``None`` for a full-grid
        run, whose scenarios combine parameters and cannot be ranked.

    Raises
    ------
    ValueError
        If ``period`` does not parse, ``result`` is malformed JSON, a
        one-at-a-time run's scenario does not contain exactly one perturbed
        parameter, its unperturbed baseline is absent, the target metric/period
        is missing, or a metric value or impact is non-finite.

    Examples
    --------
    >>> from finstack_quant.statements import ModelBuilder
    >>> from finstack_quant.statements_analytics import (
    ...     ParameterSpec,
    ...     SensitivityConfig,
    ...     generate_tornado_entries,
    ...     run_sensitivity,
    ... )
    >>> b = ModelBuilder("m")
    >>> _ = b.periods("2025Q1..Q2", None)
    >>> _ = b.value("revenue", [("2025Q1", 100.0), ("2025Q2", 110.0)])
    >>> _ = b.compute("profit", "revenue * 0.5")
    >>> cfg = SensitivityConfig(
    ...     "diagonal", [ParameterSpec.with_percentages("revenue", "2025Q2", 110.0, [-10.0, 10.0])], ["profit"]
    ... )
    >>> entries = generate_tornado_entries(run_sensitivity(b.build(), cfg), "profit", "2025Q2")
    >>> [entry.parameter_id for entry in entries]
    ['revenue@2025Q2']
    """

def run_variance(base: Any, comparison: Any, config: Any) -> VarianceReport:
    """Run variance analysis comparing two statement results.

    Parameters
    ----------
    base : StatementResult | str
        A ``StatementResult`` object or a JSON string.
    comparison : StatementResult | str
        A ``StatementResult`` object or a JSON string.
    config : VarianceConfig | str
        A typed configuration or its JSON serialization.

    Returns
    -------
    VarianceReport
        Per-metric, per-period variance rows.

    Raises
    ------
    ValueError
        If the configuration is malformed.
    KeyError
        If a configured metric is missing at a configured period.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import run_variance
    >>> run_variance("{}", "{}", "{}")
    Traceback (most recent call last):
    ValueError: ...
    """

def evaluate_scenario_set(model: Any, scenario_set: Any) -> ScenarioResults:
    """Evaluate all scenarios in a scenario set.

    Like ``Evaluator.evaluate``, a capital structure is evaluated from an
    empty market, which serves fixed-rate instruments; use
    :func:`evaluate_scenario_set_with_market` for instruments that need market
    data.

    Parameters
    ----------
    model : FinancialModelSpec | str
        A ``FinancialModelSpec`` object or a JSON string.
    scenario_set : ScenarioSet | str
        A typed scenario set or its JSON serialization.

    Returns
    -------
    ScenarioResults
        Typed mapping of scenario names to statement results.

    Raises
    ------
    ValueError
        If the set is empty, a parent chain cycles, an override is
        incompatible with its node, a forecast override has invalid
        parameters, or evaluation fails.
    KeyError
        If an override names a node missing from the model, or a
        capital-structure instrument needs market data.
    RuntimeError
        If capital-structure cashflow generation or the waterfall fails.

    Examples
    --------
    >>> from finstack_quant.statements import ModelBuilder
    >>> from finstack_quant.statements_analytics import ScenarioSet, evaluate_scenario_set
    >>> b = ModelBuilder("m")
    >>> _ = b.periods("2025Q1..Q2", None)
    >>> _ = b.value("revenue", [("2025Q1", 100.0), ("2025Q2", 110.0)])
    >>> results = evaluate_scenario_set(b.build(), ScenarioSet({"base": {}, "down": {"revenue": 90.0}}))
    >>> results.names
    ['base', 'down']
    """

def evaluate_scenario_set_with_market(
    model: Any, scenario_set: Any, market: MarketContext, as_of: Any
) -> ScenarioResults:
    """Evaluate all scenarios in a scenario set against a market as of a date.

    Each scenario is evaluated like ``Evaluator.evaluate_with_market``: the
    capital structure's instruments are priced from ``market`` as of
    ``as_of`` (floating coupons, swaps, caps), so every result carries
    ``cs_cashflows``, and explicit actuals not yet available at ``as_of`` are
    hidden. The market is shared unchanged by every scenario.

    Parameters
    ----------
    model : FinancialModelSpec | str
        A ``FinancialModelSpec`` object or a JSON string.
    scenario_set : ScenarioSet | str
        A typed scenario set or its JSON serialization.
    market : MarketContext
        Curves, fixings and FX the instruments are priced from.
    as_of : datetime.date | datetime.datetime | pandas.Timestamp | str
        Valuation date for the instruments' cashflows and the cutoff for
        explicit-value visibility; ISO ``YYYY-MM-DD`` strings are accepted.

    Returns
    -------
    ScenarioResults
        Typed mapping of scenario names to statement results.

    Raises
    ------
    ValueError
        If ``as_of`` is not date-like, the set is empty, a parent chain
        cycles, an override is incompatible with its node, a forecast
        override has invalid parameters, or evaluation fails.
    KeyError
        If an override names a node missing from the model, or an instrument
        needs market data ``market`` lacks.
    RuntimeError
        If capital-structure cashflow generation or the waterfall fails.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.statements import ModelBuilder
    >>> from finstack_quant.statements_analytics import ScenarioSet, evaluate_scenario_set_with_market
    >>> b = ModelBuilder("m")
    >>> _ = b.periods("2025Q1..Q2", None)
    >>> _ = b.value("revenue", [("2025Q1", 100.0), ("2025Q2", 110.0)])
    >>> results = evaluate_scenario_set_with_market(
    ...     b.build(), ScenarioSet({"base": {}, "down": {"revenue": 90.0}}), MarketContext(), "2025-01-01"
    ... )
    >>> results.names
    ['base', 'down']
    """

def scenario_diff(
    scenario_set: Any, results: ScenarioResults, baseline: str, comparison: str, metrics: list[str], periods: list[str]
) -> ScenarioDiff:
    """Compare two evaluated scenarios metric-by-metric.

    Parameters
    ----------
    scenario_set : ScenarioSet | str
        A typed scenario set or its JSON serialization.
    results : ScenarioResults
        Output of ``evaluate_scenario_set`` for the same scenario set.
    baseline : str
        Name of the scenario to treat as the baseline.
    comparison : str
        Name of the scenario to compare against the baseline.
    metrics : list[str]
        Node identifiers to compare. Must be non-empty.
    periods : list[str]
        Period identifiers (e.g. ``"2025Q1"``). Must be non-empty.

    Returns
    -------
    ScenarioDiff
        Baseline and comparison names alongside the variance report.

    Raises
    ------
    ValueError
        If ``metrics`` or ``periods`` is empty, a scenario name is unknown, or
        a period fails to parse.
    KeyError
        If a metric is missing at a period in either scenario.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import scenario_diff
    >>> scenario_diff("{}", "{}", "base", "up", ["ebitda"], ["2025Q1"])
    Traceback (most recent call last):
    TypeError: ...
    """

def variance_bridge(
    base: Any,
    comparison: Any,
    target_metric: str,
    period: str,
    drivers: list[str],
    baseline_label: str,
    comparison_label: str,
) -> BridgeChart:
    """Decompose a metric's scenario variance across named drivers.

    Driver contributions are raw deltas in *driver* units rather than
    sensitivities of the target metric, so they generally do not sum to the
    target variance. The gap is reported in ``BridgeChart.unexplained``.

    Parameters
    ----------
    base : StatementResult | str
        Baseline evaluated statement result, or its JSON serialization.
    comparison : StatementResult | str
        Comparison evaluated statement result, or its JSON serialization.
    target_metric : str
        Node identifier whose variance is being explained.
    period : str
        Period identifier (e.g. ``"2025Q4"``).
    drivers : list[str]
        Node identifiers treated as explanatory drivers.
    baseline_label : str
        Display label for the baseline column.
    comparison_label : str
        Display label for the comparison column.

    Returns
    -------
    BridgeChart
        Ordered driver contributions plus the unexplained residual.

    Raises
    ------
    ValueError
        If the period fails to parse.
    KeyError
        If the target or any driver is missing from either result at ``period``.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import variance_bridge
    >>> variance_bridge("{}", "{}", "ebitda", "2025Q1", ["revenue"], "base", "actual")
    Traceback (most recent call last):
    ValueError: ...
    """

class ForecastMetrics:
    """Forecast accuracy metrics (MAE, MAPE, sMAPE, RMSE).

    Examples
    --------
    >>> from finstack_quant.statements_analytics import backtest_forecast
    >>> metrics = backtest_forecast([100.0, 110.0], [98.0, 112.0])
    >>> metrics.n, metrics.mae
    (2, 2.0)
    """
    @property
    def mae(self) -> float:
        """
        Mean absolute error in data units.

        This property does not raise.

        Returns
        -------
        float
            Mean absolute error in data units.
        """
    @property
    def mape(self) -> float | None:
        """
        Mean absolute percentage error in percent (``5.0`` = 5%).

        This property does not raise.

        Returns
        -------
        float | None
            Percentage error, excluding absolute actuals below ``1e-10``;
            ``None`` when no actual contributes to MAPE.
        """
    @property
    def mape_effective_n(self) -> int:
        """
        Number of observations with a non-zero actual used by ``mape``.

        This property does not raise.

        Returns
        -------
        int
            Number of observations with a non-zero actual used by ``mape``.
        """
    @property
    def smape(self) -> float | None:
        """
        Symmetric MAPE in percent.

        This property does not raise.

        Returns
        -------
        float | None
            Symmetric MAPE in percent, or ``None`` when every denominator
            ``(abs(actual) + abs(forecast)) / 2`` is below ``1e-10``.
        """
    @property
    def rmse(self) -> float:
        """
        Root mean squared error in data units.

        This property does not raise.

        Returns
        -------
        float
            Root mean squared error in data units.
        """
    @property
    def n(self) -> int:
        """
        Number of observations.

        This property does not raise.

        Returns
        -------
        int
            Number of observations.
        """
    def summary(self) -> str:
        """
        One-line human-readable summary (Rust ``ForecastMetrics::summary``).

        This method reads already-computed state and does not raise.

        Returns
        -------
        str
            One-line human-readable summary (Rust ``ForecastMetrics::summary``).
        """
    def to_series(self) -> LabeledSeries:
        """
        Export as a ``LabeledSeries`` indexed by metric name.

        Index: ``mae``, ``mape``, ``mape_effective_n``, ``smape``, ``rmse``,
        ``n``; counts are represented as floats and unavailable percentages
        become null (``NaN`` in ``to_pandas()``).

        Returns
        -------
        LabeledSeries
            Metrics indexed by name, with counts as floats and unavailable
            percentages represented as null.

        Raises
        ------
        ValueError
            If the series cannot be constructed.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        A non-finite metric (``mape`` when no actual is non-zero) is written
        as ``"nan"``, ``"inf"`` or ``"-inf"`` so the document round-trips.

        Returns
        -------
        str
            Canonical JSON encoding of this value, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> ForecastMetrics:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        ForecastMetrics
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``ForecastMetrics`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import ForecastMetrics
        >>> ForecastMetrics.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

def backtest_forecast(actual: list[float], forecast: list[float]) -> ForecastMetrics:
    """Compute forecast accuracy metrics (MAE, MAPE, sMAPE, RMSE).

    Parameters
    ----------
    actual : list[float]
        Finite observed values in consistent units; negative values are accepted.
    forecast : list[float]
        Finite forecast values in the same units and aligned with ``actual``.

    Returns
    -------
    ForecastMetrics
        Typed metrics with ``summary()`` and ``to_series()``. ``mape`` and
        ``smape`` are ``None`` when no denominator contributes; JSON emits ``null``.

    Raises
    ------
    ValueError
        If the sequences are empty, of different lengths, contain a non-finite
        value, or metric arithmetic overflows.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import backtest_forecast
    >>> backtest_forecast([1.0, 2.0], [1.0, 2.5]).n
    2
    """

class _GoalCell(TypedDict):
    node: str
    period: str

class _GoalBracket(TypedDict):
    kind: Literal["bracket"]
    lower: float
    upper: float

class _GoalExpansion(TypedDict):
    kind: Literal["bounded"]
    lower: float
    upper: float
    start: float
    initial_half_width: float
    max_expansions: int

class GoalSeekSpec:
    """Elect a dated editable forecast cell, target, and allowed root search.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import GoalSeekSpec
    >>> spec = GoalSeekSpec(
    ...     driver={"node": "revenue", "period": "2025Q1"},
    ...     target={"node": "profit", "period": "2025Q1"},
    ...     target_value=60.0,
    ...     target_tolerance=1e-9,
    ...     driver_tolerance=1e-12,
    ...     max_iterations=100,
    ...     search={"kind": "bracket", "lower": 0.0, "upper": 200.0},
    ...     as_of="2024-12-31",
    ...     observed_through="2024-12-31",
    ...     update_model=True,
    ... )
    >>> isinstance(spec, GoalSeekSpec)
    True
    """

    def __init__(
        self,
        *,
        driver: _GoalCell,
        target: _GoalCell,
        target_value: Money | float,
        target_tolerance: Money | float,
        driver_tolerance: float,
        max_iterations: int,
        search: _GoalBracket | _GoalExpansion,
        as_of: date | str,
        observed_through: date | str,
        update_model: bool,
    ) -> None:
        """Validate and assemble the complete point-in-time solve request.

        Parameters
        ----------
        driver : mapping
            Exact node and period of an explicitly authored editable forecast observation.
        target : mapping
            Exact target node and period, on or after the driver period.
        target_value : Money or float
            Requested outcome in exactly the target cell's units.
        target_tolerance : Money or float
            Nonnegative absolute residual allowance, in exactly the target's units.
        driver_tolerance : float
            Positive numerical root tolerance in represented driver units.
        max_iterations : int
            Positive number of allowed root iterations after locating a bracket.
        search : mapping
            Explicit finite increasing bracket, or bounded expansion with a starting
            driver, positive initial half-width and maximum number of expansions.
        as_of : date or str
            Held market pricing date for every trial and the final verification.
        observed_through : date or str
            Inclusive publication cutoff on or before as_of.
        update_model : bool
            Return the verified model copy with its solved assumption installed.

        Raises
        ------
        ValueError
            If dates, search limits, tolerances or any required key are invalid.
        """
        ...

class GoalSeekResult:
    """Verified typed driver and fresh target evaluation on the elected cut.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import GoalSeekSpec
    >>> spec = GoalSeekSpec(
    ...     driver={"node": "revenue", "period": "2025Q1"},
    ...     target={"node": "profit", "period": "2025Q1"},
    ...     target_value=60.0,
    ...     target_tolerance=1e-9,
    ...     driver_tolerance=1e-12,
    ...     max_iterations=100,
    ...     search={"kind": "bracket", "lower": 0.0, "upper": 200.0},
    ...     as_of="2024-12-31",
    ...     observed_through="2024-12-31",
    ...     update_model=True,
    ... )
    >>> from finstack_quant.statements import ModelBuilder
    >>> from finstack_quant.statements_analytics import goal_seek
    >>> b = ModelBuilder("forecast").periods("2025Q1..Q1")
    >>> _ = b.source_snapshot("goal-example-forecast")
    >>> _ = b.value_scalar("revenue", {"2025Q1": 100.0})
    >>> _ = b.compute("profit", "revenue * 0.5")
    >>> result = goal_seek(b.build(), spec)
    >>> (result.solved_value, result.achieved_value)
    (120.0, 60.0)
    """

    @property
    def solved_value(self) -> float | Money:
        """Read the reported solved value.

        Returns
        -------
        float | Money
            Driver value selected by the solve in original driver units.

        Raises
        ------
        MemoryError
            If a Python representation cannot be allocated.
        """
        ...

    @property
    def target_value(self) -> float | Money:
        """Read the reported target value.

        Returns
        -------
        float | Money
            Authored target in the target cell’s units.

        Raises
        ------
        MemoryError
            If a Python representation cannot be allocated.
        """
        ...

    @property
    def achieved_value(self) -> float | Money:
        """Read the reported achieved value.

        Returns
        -------
        float | Money
            Target value observed in the fresh evaluation.

        Raises
        ------
        MemoryError
            If a Python representation cannot be allocated.
        """
        ...

    @property
    def residual(self) -> float | Money:
        """Read the reported residual.

        Returns
        -------
        float | Money
            Achieved less requested target, signed in target units.

        Raises
        ------
        MemoryError
            If a Python representation cannot be allocated.
        """
        ...

    @property
    def target_tolerance(self) -> float | Money:
        """Read the reported target tolerance.

        Returns
        -------
        float | Money
            Authored absolute residual tolerance in target units.

        Raises
        ------
        MemoryError
            If a Python representation cannot be allocated.
        """
        ...

    @property
    def driver_tolerance(self) -> float:
        """Read the reported driver tolerance.

        Returns
        -------
        float
            Positive numerical root tolerance in driver units.

        This property does not raise.
        """
        ...

    @property
    def evaluations(self) -> int:
        """Read the reported evaluations.

        Returns
        -------
        int
            Evaluation count including the fresh verification.

        This property does not raise.
        """
        ...

    @property
    def original_run(self) -> dict[str, object]:
        """Read the reported original run.

        Returns
        -------
        dict[str, object]
            Content and cutoff identity of the original evaluation.

        Raises
        ------
        ValueError
            If the stored provenance cannot be serialized.
        """
        ...

    @property
    def solved_run(self) -> dict[str, object]:
        """Read the reported solved run.

        Returns
        -------
        dict[str, object]
            Content and cutoff identity of the verified solved evaluation.

        Raises
        ------
        ValueError
            If the stored provenance cannot be serialized.
        """
        ...

    @property
    def market_hash(self) -> str:
        """Read the reported market hash.

        Returns
        -------
        str
            Canonical content identity of the held market inputs.

        This property does not raise.
        """
        ...

    @property
    def model(self) -> FinancialModelSpec | None:
        """Read the reported model.

        Returns
        -------
        FinancialModelSpec | None
            Verified copy with the solved assumption installed, or None when omitted.

        This property does not raise.
        """
        ...

    def to_json(self) -> str:
        """Serialize the verified result for wire interchange.

        Returns
        -------
        str
            Canonical JSON with typed driver, target and provenance.

        Raises
        ------
        ValueError
            If the result cannot be encoded.
        """
        ...

    @staticmethod
    def from_json(json: str) -> GoalSeekResult:
        """Read a canonical result and validate its embedded model, if present.

        Parameters
        ----------
        json : str
            Exact native goal-seek result document.

        Returns
        -------
        GoalSeekResult
            Decoded result with the same typed values and solved model.

        Raises
        ------
        ValueError
            If the wire values or embedded model are invalid.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import GoalSeekResult
        >>> GoalSeekResult.from_json("{}")
        Traceback (most recent call last):
        ValueError: ...
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support pickling through the canonical result document.

        Returns
        -------
        tuple
            Result loader and its serialized input.

        Raises
        ------
        ValueError
            If the result cannot be encoded.
        """
        ...

def goal_seek(
    model: FinancialModelSpec | str,
    spec: GoalSeekSpec,
    market: MarketContext | None = None,
) -> GoalSeekResult:
    """Find and verify an editable forecast assumption at fixed observation and market dates.

    Parameters
    ----------
    model : FinancialModelSpec or str
        Original model containing both the editable assumption and the target.
    spec : GoalSeekSpec
        Named driver and target, typed target amount, tolerances and explicit search bounds.
    market : MarketContext or None
        The held market inputs for linked financing. None uses an empty market.

    Returns
    -------
    GoalSeekResult
        Freshly verified typed outcome, signed target residual, audit provenance and
        the solved model copy when requested. The input model is immutable.

    Raises
    ------
    ValueError
        If the cells, dates, units, search domain or a trial evaluation are invalid,
        or if bracket search or verification cannot achieve the elected target tolerance.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import GoalSeekSpec
    >>> spec = GoalSeekSpec(
    ...     driver={"node": "revenue", "period": "2025Q1"},
    ...     target={"node": "profit", "period": "2025Q1"},
    ...     target_value=60.0,
    ...     target_tolerance=1e-9,
    ...     driver_tolerance=1e-12,
    ...     max_iterations=100,
    ...     search={"kind": "bracket", "lower": 0.0, "upper": 200.0},
    ...     as_of="2024-12-31",
    ...     observed_through="2024-12-31",
    ...     update_model=True,
    ... )
    >>> from finstack_quant.statements import ModelBuilder
    >>> from finstack_quant.statements_analytics import goal_seek
    >>> b = ModelBuilder("forecast").periods("2025Q1..Q1")
    >>> _ = b.source_snapshot("goal-example-forecast")
    >>> _ = b.value_scalar("revenue", {"2025Q1": 100.0})
    >>> _ = b.compute("profit", "revenue * 0.5")
    >>> result = goal_seek(b.build(), spec)
    >>> result.solved_value
    120.0
    """
    ...

"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements_analytics/analysis.rs``."""

from __future__ import annotations

from typing import Any
import pandas as pd
from finstack_quant.statements import FinancialModelSpec, StatementResult
from finstack_quant.core.market_data import MarketContext
from finstack_quant.statements_analytics._typed import (
    BridgeChart,
    ScenarioDiff,
    ScenarioResults,
    SensitivityResult,
    TornadoEntry,
    VarianceReport,
)

class DependencyTracer:
    """
    Reusable dependency tracer for a financial model.

    Parameters
    ----------
    model : FinancialModelSpec or str
        ``FinancialModelSpec`` object or JSON string.

    Examples
    --------
    >>> from finstack_quant.statements import ModelBuilder
    >>> from finstack_quant.statements_analytics import DependencyTracer
    >>> builder = ModelBuilder("demo")
    >>> _ = builder.periods("2025Q1..Q1")
    >>> _ = builder.value("revenue", [("2025Q1", 100.0)])
    >>> _ = builder.value("cost", [("2025Q1", 60.0)])
    >>> _ = builder.compute("profit", "revenue - cost")
    >>> sorted(DependencyTracer(builder.build()).direct_dependencies("profit"))
    ['cost', 'revenue']

    """

    def __init__(self, model: FinancialModelSpec | str) -> None:
        """
        Create a dependency tracer for the given model.

        Parameters
        ----------
        model : FinancialModelSpec or str
            ``FinancialModelSpec`` object or JSON string.

        Raises
        ------
        ValueError
            If model JSON is malformed or its dependency graph is invalid.

        """
        ...
    def dependency_tree(self, node_id: str) -> DependencyTree:
        """
        Return the dependency tree for ``node_id`` (Rust ``DependencyTracer::dependency_tree``).

        Parameters
        ----------
        node_id : str
            Root node to trace.

        Returns
        -------
        DependencyTree
            Typed tree rooted at ``node_id`` with its complete upstream
            dependency hierarchy; ``to_json()`` matches the WASM
            ``DependencyTracer.dependencyTree`` result.

        Raises
        ------
        KeyError
            If ``node_id`` or a reachable dependency is not in the model.
        ValueError
            If the dependency graph is invalid.

        """
        ...
    def dependency_tree_text(self, node_id: str) -> str:
        """
        Return the ASCII rendering of ``node_id``'s dependency tree.

        The root is on the first line; every dependency follows on its own
        line behind ``├──`` / ``└──`` connectors, indented by depth, with its
        formula in parentheses. Identical to the WASM ``DependencyTracer.dependencyTreeText``.

        Parameters
        ----------
        node_id : str
            Root node to trace.

        Returns
        -------
        str
            Multi-line ASCII tree ending with a newline.

        Raises
        ------
        KeyError
            If ``node_id`` or a reachable dependency is not in the model.
        ValueError
            If the dependency graph is invalid.

        """
        ...

    def dependency_tree_detailed(self, results: StatementResult | str, node_id: str, period: str) -> str:
        """
        Return an ASCII dependency tree annotated with values for one period.

        Parameters
        ----------
        results : StatementResult or str
            Statement results to annotate with.
        node_id : str
            Root node to trace.
        period : str
            Period to annotate.

        Returns
        -------
        str
            Multi-line ASCII dependency tree whose nodes are annotated with
            values from ``results`` for ``period``.

        Raises
        ------
        ValueError
            If results JSON or period is invalid, or node_id cannot be traced.

        """
        ...

    def direct_dependencies(self, node_id: str) -> list[str]:
        """
        List immediate dependencies of ``node_id``.

        Parameters
        ----------
        node_id : str
            Statement node ID whose directly referenced inputs are requested.

        Returns
        -------
        list[str]
            Statement node IDs referenced directly by ``node_id``.

        Raises
        ------
        ValueError
            If node_id is unknown or its dependency graph is invalid.

        """
        ...
    def all_dependencies(self, node_id: str) -> list[str]:
        """
        List all transitive dependencies of ``node_id``.

        Parameters
        ----------
        node_id : str
            Statement node ID whose complete upstream dependency set is requested.

        Returns
        -------
        list[str]
            Direct and transitive upstream node IDs in dependency order, with
            dependencies preceding their dependents.

        Raises
        ------
        ValueError
            If node_id is unknown or the dependency traversal fails.

        """
        ...
    def dependents(self, node_id: str) -> list[str]:
        """
        List nodes that depend on ``node_id``.

        Parameters
        ----------
        node_id : str
            Statement node ID whose downstream dependents are requested.

        Returns
        -------
        list[str]
            Statement node IDs that directly depend on ``node_id``.

        Raises
        ------
        ValueError
            If node_id is unknown or its dependency graph is invalid.

        """
        ...

# Comparable-company analysis

# Credit scorecard extension

class DependencyTree:
    """
    Dependency tree of one statement node (the Rust ``DependencyTree``).

    Returned by :meth:`DependencyTracer.dependency_tree`; its JSON form is the
    WASM ``DependencyTracer.dependencyTree`` result.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import DependencyTree
    >>> tree = DependencyTree.from_json(
    ...     '{"node_id":"profit","formula":"revenue * 0.5","children":'
    ...     '[{"node_id":"revenue","formula":null,"children":[]}]}'
    ... )
    >>> tree.node_id, tree.formula, [child.node_id for child in tree.children]
    ('profit', 'revenue * 0.5', ['revenue'])
    """
    @property
    def node_id(self) -> str:
        """
        Node identifier.

        A dependency already on the current path appears as a leaf named
        ``"<id> (cycle)"``. This property does not raise.

        Returns
        -------
        str
            Node identifier of this tree's root.
        """
    @property
    def formula(self) -> str | None:
        """
        Formula text of the node.

        This property does not raise.

        Returns
        -------
        str | None
            Formula text when the node is calculated, ``None`` for a value node.
        """
    @property
    def children(self) -> list[DependencyTree]:
        """
        Direct dependencies of the node.

        This property does not raise.

        Returns
        -------
        list[DependencyTree]
            One tree per direct dependency, in formula reference order.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding (``node_id``, ``formula``, ``children``),
            accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> DependencyTree:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json` or the WASM
            ``DependencyTracer.dependencyTree`` result; unknown fields are rejected.

        Returns
        -------
        DependencyTree
            The tree decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``DependencyTree`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import DependencyTree
        >>> DependencyTree.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class Explanation:
    """Explanation of how a node's value was derived at one period.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import Explanation
    >>> e = Explanation.from_json(
    ...     '{"node_id":"profit","period_id":"2025Q1","final_value":50.0,'
    ...     '"node_type":"calculated","formula_text":"revenue * 0.5","breakdown":[]}'
    ... )
    >>> e.node_id, e.final_value
    ('profit', 50.0)
    """
    @property
    def node_id(self) -> str:
        """
        Explained node id.

        This property does not raise.

        Returns
        -------
        str
            Explained node id.
        """
    @property
    def period_id(self) -> str:
        """
        Period-id string of the explanation.

        This property does not raise.

        Returns
        -------
        str
            Period-id string of the explanation.
        """
    @property
    def final_value(self) -> float:
        """
        Node value at the period.

        This property does not raise.

        Returns
        -------
        float
            Node value at the period.
        """
    @property
    def node_type(self) -> str:
        """
        Node type serde name (e.g. ``"calculated"``, ``"input"``).

        This property does not raise.

        Returns
        -------
        str
            Node type serde name (e.g. ``"calculated"``, ``"input"``).
        """
    @property
    def source(self) -> str | None:
        """
        Evaluation layer that produced the value under the
        ``Value > Forecast > Formula`` precedence: ``"value"`` (the explicit
        value stored on the node), ``"forecast"``, ``"formula"`` or
        ``"where_masked"`` (the node's ``where`` clause zeroed the cell).

        This property does not raise.

        Returns
        -------
        str | None
            The source label, or ``None`` when the results carry no source for the cell (hand-built results, or JSON written before sources were recorded).
        """
    @property
    def forecast(self) -> dict[str, Any] | None:
        """
        The node's forecast specification when ``source`` is ``"forecast"``.

        The dict is the serde form ``{"method": ..., "params": {...}}``, for
        example ``{"method": "growth_pct", "params": {"rate": 0.05}}``.

        This property does not raise.

        Returns
        -------
        dict[str, Any] | None
            The forecast spec, or ``None`` for every other source.
        """
    @property
    def formula_text(self) -> str | None:
        """
        Formula text, or ``None`` for non-formula nodes.

        This property does not raise.

        Returns
        -------
        str | None
            Formula text, or ``None`` for non-formula nodes.
        """
    @property
    def breakdown(self) -> list[ExplanationStep]:
        """
        Formula components with the values they resolved to at the period,
        including ``cs.<component>.<instrument_or_total>`` capital-structure
        references.

        Populated only when the formula produced the value (``source`` is
        ``"formula"`` or ``None``); empty when ``source`` is ``"value"``,
        ``"forecast"`` or ``"where_masked"``. When the formula is a pure
        sum/difference of references there is one step per term in formula
        order, each with ``operation`` ``"+"`` or ``"-"``, and the signed
        values sum to ``final_value``; for any other formula each referenced
        component is listed once with ``operation`` ``None``.

        This property does not raise.

        Returns
        -------
        list[ExplanationStep]
            Formula components in formula order; empty when the value did not come from the formula.
        """
    def to_text(self) -> str:
        """
        Human-readable multi-line explanation.

        This method reads already-computed state and does not raise.

        Returns
        -------
        str
            Human-readable multi-line explanation.
        """
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the breakdown as a pandas ``DataFrame``.

        Columns: ``component``, ``value``, ``operation`` (``None`` when absent).
        One row per breakdown step in evaluation order.

        Returns
        -------
        pd.DataFrame
            Export the breakdown as a pandas ``DataFrame``. Columns: ``component``, ``value``, ``operation`` (``None`` when absent). One row per breakdown step in evaluation order.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to its wire form.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON (identical to the WASM ``explainFormula`` output).

        A non-finite ``final_value`` or step ``value`` is written as
        ``"nan"``, ``"inf"`` or ``"-inf"`` so the document round-trips.

        Returns
        -------
        str
            Serialize to canonical JSON (identical to the WASM ``explainFormula`` output).

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> Explanation:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        Explanation
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``Explanation`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import Explanation
        >>> Explanation.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class ExplanationStep:
    """
    One component of a formula explanation.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import ExplanationStep
    >>> [field for field in ("component", "value") if hasattr(ExplanationStep, field)]
    ['component', 'value']
    """
    @property
    def component(self) -> str:
        """
        Component node id or literal text.

        This property does not raise.

        Returns
        -------
        str
            Component node id or literal text.
        """
    @property
    def value(self) -> float:
        """
        Component value at the explained period.

        This property does not raise.

        Returns
        -------
        float
            Component value at the explained period.
        """
    @property
    def operation(self) -> str | None:
        """
        Sign with which the component enters the formula.

        Set to ``"+"`` or ``"-"`` only when the formula is a pure
        sum/difference of references, in which case the signed step values sum
        to the explained value.

        This property does not raise.

        Returns
        -------
        str | None
            ``"+"`` or ``"-"``, or ``None`` for any other formula.
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
    def to_series(self) -> pd.Series:
        """
        Export as a pandas ``Series`` indexed by metric name.

        Index: ``mae``, ``mape``, ``mape_effective_n``, ``smape``, ``rmse``,
        ``n``; counts are cast to float and unavailable percentages become
        pandas ``NaN``.

        Returns
        -------
        pd.Series
            Metrics indexed by name, with counts cast to float and unavailable
            percentages represented as pandas ``NaN``.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to its wire form.
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

class GoalSeekResult:
    """
    Result of a goal-seek solve (the Rust ``GoalSeekResult``).

    Its JSON form ``{"solved_value", "model", "residual", "tolerance",
    "evaluations", "converged"}`` is the WASM ``goalSeek`` result; ``model``
    is ``null`` when it was not requested.

    Examples
    --------
    >>> from finstack_quant.statements import ModelBuilder
    >>> from finstack_quant.statements_analytics import GoalSeekResult, goal_seek
    >>> b = ModelBuilder("m")
    >>> _ = b.periods("2025Q1..Q1", None)
    >>> _ = b.value("revenue", [("2025Q1", 100.0)])
    >>> _ = b.compute("profit", "revenue * 0.5")
    >>> result = goal_seek(b.build(), "profit", "2025Q1", 60.0, "revenue", "2025Q1", True)
    >>> round(result.solved_value, 6), result.model is None
    (120.0, False)
    >>> round(GoalSeekResult.from_json(result.to_json()).solved_value, 6)
    120.0
    >>> result.converged, abs(result.residual) <= result.tolerance, result.evaluations > 0
    (True, True, True)
    """
    @property
    def solved_value(self) -> float:
        """
        Driver value that reaches the target, in the driver node's units.

        This property does not raise.

        Returns
        -------
        float
            Driver value that reaches the target.
        """
    @property
    def model(self) -> FinancialModelSpec | None:
        """
        Model with the solved driver written in.

        This property does not raise.

        Returns
        -------
        FinancialModelSpec | None
            The updated model, or ``None`` when ``update_model=False``.
        """
    @property
    def residual(self) -> float:
        """
        Signed residual at ``solved_value``: the target node's value in the
        target period minus the requested target, in the target node's units.

        It is re-evaluated on the model after the solver returned, so it is
        the residual of the reported driver value.

        This property does not raise.

        Returns
        -------
        float
            The residual; ``0.0`` for a result loaded from JSON written before diagnostics were recorded (``evaluations == 0``).
        """
    @property
    def tolerance(self) -> float:
        """
        Absolute acceptance tolerance on ``residual``, in the target node's
        units: ``1e-9 * max(abs(target_value), 1)``.

        This property does not raise.

        Returns
        -------
        float
            The tolerance; ``0.0`` for a result loaded from JSON written before diagnostics were recorded.
        """
    @property
    def evaluations(self) -> int:
        """
        Number of model evaluations the solve used: every objective probe by
        the root finder (bracket search and Brent iterations, including a
        widened second attempt) plus the final residual check.

        This property does not raise.

        Returns
        -------
        int
            Evaluation count; ``0`` only for a result loaded from JSON written before diagnostics were recorded.
        """
    @property
    def converged(self) -> bool:
        """
        Whether ``abs(residual) <= tolerance``.

        ``goal_seek`` raises instead of returning when the residual check
        fails, so every result it returns has ``converged`` ``True``.

        This property does not raise.

        Returns
        -------
        bool
            ``True`` for a result returned by ``goal_seek``; ``False`` only for a result loaded from JSON written before diagnostics were recorded.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            ``{"solved_value": ..., "model": ..., "residual": ...,
            "tolerance": ..., "evaluations": ..., "converged": ...}`` with
            ``model`` ``null`` when it was not requested; identical to the
            WASM ``goalSeek`` result and accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> GoalSeekResult:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json` or the WASM ``goalSeek``
            result; unknown fields are rejected.

        Returns
        -------
        GoalSeekResult
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``GoalSeekResult`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import GoalSeekResult
        >>> GoalSeekResult.from_json('{"solved_value": 1.5, "model": null}').solved_value
        1.5
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
    model: Any, scenario_set: Any, market: MarketContext | str, as_of: Any
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
    market : MarketContext | str
        Curves, fixings and FX the instruments are priced from, typed or its
        canonical JSON.
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

def explain_formula(model: Any, results: Any, node_id: str, period: str) -> Explanation:
    """Explain a formula for a specific node and period.

    Parameters
    ----------
    model : FinancialModelSpec | str
        A ``FinancialModelSpec`` object or a JSON string.
    results : StatementResult | str
        A ``StatementResult`` object or a JSON string.
    node_id : str
        Node whose formula to explain.
    period : str
        Period string.

    Returns
    -------
    Explanation
        Typed explanation with the cell ``source``, the ``forecast`` spec of a
        forecast cell, ``breakdown`` steps, ``to_text()`` and
        ``to_dataframe()``; ``to_json()`` matches the WASM ``explainFormula``.

    Raises
    ------
    KeyError
        If ``node_id`` is not in the model or has no value at ``period``.
    ValueError
        If ``period`` does not parse or a payload is malformed JSON.

    Examples
    --------
    >>> from finstack_quant.statements import ModelBuilder, Evaluator
    >>> from finstack_quant.statements_analytics import explain_formula
    >>> b = ModelBuilder("m")
    >>> _ = b.periods("2025Q1..Q1", None)
    >>> _ = b.value("revenue", [("2025Q1", 100.0)])
    >>> _ = b.compute("profit", "revenue * 0.5")
    >>> model = b.build()
    >>> explain_formula(model, Evaluator().evaluate(model), "profit", "2025Q1").final_value
    50.0
    """

def generate_tornado_entries(result: Any, metric_node: str, period: str | None = None) -> list[TornadoEntry]:
    """Generate tornado chart entries for a sensitivity result.

    Parameters
    ----------
    result : SensitivityResult | str
        A typed sensitivity result or its JSON serialization.
    metric_node : str
        Node to extract tornado entries for.
    period : str | None
        Optional period string to pin the tornado to.

    Returns
    -------
    list[TornadoEntry]
        Typed entries sorted by descending absolute swing, with parameter IDs
        preserving the perturbed ``node@period`` key.

    Raises
    ------
    ValueError
        If ``period`` does not parse, ``result`` is malformed JSON, the run
        uses a full grid, a scenario does not contain exactly one perturbed
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

def goal_seek(
    model: Any,
    target_node: str,
    target_period: str,
    target_value: float,
    driver_node: str,
    driver_period: str,
    update_model: bool,
    bounds: tuple[float, float] | None = None,
) -> GoalSeekResult:
    """Find the driver value that makes a target node reach a target value.

    Parameters
    ----------
    model : FinancialModelSpec | str
        A ``FinancialModelSpec`` object or a JSON string.
    target_node : str
        Node to drive towards ``target_value``.
    target_period : str
        Period string for the target (e.g. ``"2025Q4"``).
    target_value : float
        Desired value for the target node.
    driver_node : str
        Node whose value is adjusted to reach the target.
    driver_period : str
        Period string for the driver.
    update_model : bool
        If ``True``, the result's ``model`` is a copy of ``model`` with the
        solved value written in; if ``False`` it is ``None``. Required, as in
        Rust and WASM.
    bounds : tuple[float, float] | None
        Optional search bracket ``(lo, hi)`` in the driver node's units.

    Returns
    -------
    GoalSeekResult
        ``solved_value`` plus ``model`` (the updated ``FinancialModelSpec`` or
        ``None``) and the solve diagnostics ``residual``, ``tolerance``,
        ``evaluations`` and ``converged``. ``float(result)`` yields the solved
        value. The input model is never modified.

    Raises
    ------
    ValueError
        If a period does not parse, ``target_node`` or ``driver_node`` is
        missing, the solver fails to converge, the bracket does not contain a
        root, or the final objective residual exceeds 1e-9 times
        max(1, abs(target_value)).

    Examples
    --------
    >>> from finstack_quant.statements import ModelBuilder
    >>> from finstack_quant.statements_analytics import goal_seek
    >>> b = ModelBuilder("m")
    >>> _ = b.periods("2025Q1..Q1", None)
    >>> _ = b.value("revenue", [("2025Q1", 100.0)])
    >>> _ = b.compute("profit", "revenue * 0.5")
    >>> round(goal_seek(b.build(), "profit", "2025Q1", 60.0, "revenue", "2025Q1", False).solved_value, 6)
    120.0
    """

def run_sensitivity(model: Any, config: Any) -> SensitivityResult:
    """Run sensitivity analysis on a financial model.

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

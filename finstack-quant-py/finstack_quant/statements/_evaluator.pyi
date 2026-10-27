"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements/evaluator.rs``."""

from __future__ import annotations

from datetime import date
from typing import Any, Literal
import pandas as pd
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.core.table import ArrowTable
from finstack_quant.statements._capital_structure import CapitalStructureCashflows
from finstack_quant.statements._checks import CheckReport, CheckSuiteSpec
from finstack_quant.statements._monte_carlo import MonteCarloConfig, MonteCarloResults
from finstack_quant.statements._rollup import RollupSpec
from finstack_quant.statements._types import FinancialModelSpec, NumericMode

class StatementResult:
    """
    Per-node, per-period numeric results from evaluating a model.

    JSON and pickle preserve non-finite values with ``"nan"``, ``"inf"``, and
    ``"-inf"`` strings in node cells and non-finite warnings. Accessors return
    Python floats, including NaN and infinities.

    Examples
    --------
    >>> from finstack_quant.statements import Evaluator, ModelBuilder
    >>> builder = ModelBuilder("demo")
    >>> _ = builder.periods("2025Q1..Q1")
    >>> _ = builder.value("revenue", [("2025Q1", 100.0)])
    >>> Evaluator().evaluate(builder.build()).get("revenue", "2025Q1")
    100.0

    """

    @staticmethod
    def from_json(json: str) -> StatementResult:
        """
        Deserialize evaluation results from JSON.

        Parameters
        ----------
        json:
            JSON document for ``StatementResult``.

        Returns
        -------
        StatementResult
            Parsed results.

        Raises
        ------
        ValueError
            If JSON parsing fails.

        Examples
        --------
        >>> from finstack_quant.statements import Evaluator, ModelBuilder, StatementResult
        >>> builder = ModelBuilder("demo")
        >>> _ = builder.periods("2025Q1..Q1")
        >>> _ = builder.value("revenue", [("2025Q1", 100.0)])
        >>> result = Evaluator().evaluate(builder.build())
        >>> StatementResult.from_json(result.to_json()).get("revenue", "2025Q1")
        100.0

        """
        ...

    def to_json(self) -> str:
        """
        Serialize these results to compact JSON.

        Returns
        -------
        str
            JSON text.

            Canonical JSON representation of this `StatementResult`, suitable for a matching `from_json` call.
        Raises
        ------
        ValueError
            If serialization fails.

        """
        ...

    def get(self, node_id: str, period: str) -> float | None:
        """
        Get the value for a node at a specific period.

        Returns the f64 view of whichever source won this period under the
        crate's **Value > Forecast > Formula** precedence rule.

        Parameters
        ----------
        node_id:
            Node identifier.
        period:
            Period label such as ``"2025Q1"``.

        Returns
        -------
        float | None
            The value in the node's own units — a currency amount for
            monetary nodes (currency not carried; use :meth:`get_money` for
            that) and a unitless scalar otherwise. ``None`` when the node or
            period is unknown.

        Raises
        ------
        ValueError
            If ``period`` cannot be parsed as a period id.

        """
        ...

    def get_money(self, node_id: str, period: str) -> Money | None:
        """
        Return the currency-tagged ``Money`` value for a monetary node.

        Preserves fixed-point precision and currency. Returns ``None`` when
        the node is not monetary or has no value for this period.

        Parameters
        ----------
        node_id:
            Node identifier.
        period:
            Period label such as ``"2025Q1"``.

        Returns
        -------
        Money | None
            Monetary value when found, otherwise ``None``.

        Raises
        ------
        ValueError
            If ``period`` cannot be parsed as a period id.
        """
        ...

    def get_scalar(self, node_id: str, period: str) -> float | None:
        """
        Return the scalar value for a non-monetary node.

        Returns ``None`` when the node is monetary or has no value for this
        period.

        Parameters
        ----------
        node_id:
            Node identifier.
        period:
            Period label such as ``"2025Q1"``.

        Returns
        -------
        float | None
            Scalar value when found, otherwise ``None``.

        Raises
        ------
        ValueError
            If ``period`` cannot be parsed as a period id.
        """
        ...

    def get_or(self, node_id: str, period: str, default: float) -> float:
        """
        Return the value for a node at a period, or a default when missing.

        Parameters
        ----------
        node_id:
            Node identifier.
        period:
            Period label such as ``"2025Q1"``.
        default:
            Value returned when the node or period is unknown.

        Returns
        -------
        float
            The evaluated value in the node's own units, or ``default``.

        Raises
        ------
        ValueError
            If ``period`` cannot be parsed as a period id.

        Examples
        --------
        >>> from finstack_quant.statements import Evaluator, ModelBuilder
        >>> builder = ModelBuilder("demo")
        >>> _ = builder.periods("2025Q1..Q1")
        >>> _ = builder.value("revenue", [("2025Q1", 100.0)])
        >>> result = Evaluator().evaluate(builder.build())
        >>> result.get_or("missing", "2025Q1", 0.0)
        0.0
        """
        ...

    def all_periods(self, node_id: str) -> list[tuple[str, float]]:
        """
        Get every evaluated period for one node as ordered pairs.

        Parameters
        ----------
        node_id:
            Node identifier.

        Returns
        -------
        list[tuple[str, float]]
            ``(period, value)`` pairs in evaluation order, in the node's own
            units. Empty when the node is not in the result.

        Notes
        -----
        This method does not raise; an unknown *node_id* yields an empty list.

        Examples
        --------
        >>> from finstack_quant.statements import Evaluator, ModelBuilder
        >>> builder = ModelBuilder("demo")
        >>> _ = builder.periods("2025Q1..Q1")
        >>> _ = builder.value("revenue", [("2025Q1", 100.0)])
        >>> Evaluator().evaluate(builder.build()).all_periods("revenue")
        [('2025Q1', 100.0)]
        """
        ...

    def rollup(self, model: FinancialModelSpec | str, spec: RollupSpec | str) -> StatementResult:
        """
        Roll these results up to a coarser frequency, such as quarterly to annual.

        Each target period combines its source periods node by node: summed by
        default (flows), the last source value for ``"period_end"`` nodes
        (balances, LTM metrics), and the node's formula re-evaluated on the
        rolled-up values for ``"formula"`` nodes (ratios). ``lag`` and other
        time-series functions then step in target periods. Every target period
        must be complete.

        Parameters
        ----------
        model : FinancialModelSpec | str
            The model these results were evaluated from, typed or its canonical
            JSON. It supplies the period dates and the formulas of
            ``"formula"`` nodes.
        spec : RollupSpec | str
            Target frequency and per-node methods, typed or its JSON.

        Returns
        -------
        StatementResult
            Values keyed by the target periods (``"2025"``, ``"2025H1"``,
            ``"2025Q1"``; fiscal periods keep their ``FY`` prefix), with
            monetary values and value types. It carries no ``node_sources``,
            capital-structure cashflows or check report.

        Raises
        ------
        ValueError
            If the JSON is malformed, the target frequency is not a whole
            multiple of the model's, a target period is incomplete, a method
            names an unknown node, a ``"formula"`` node has no formula or reads
            ``cs.*`` values, or a re-evaluated formula fails.

        Examples
        --------
        >>> from finstack_quant.statements import Evaluator, ModelBuilder, RollupSpec
        >>> b = ModelBuilder("m")
        >>> _ = b.periods("2025Q1..Q4", None)
        >>> _ = b.value("revenue", [(f"2025Q{q}", 100.0) for q in range(1, 5)])
        >>> _ = b.value("cash", [("2025Q1", 10.0), ("2025Q2", 20.0), ("2025Q3", 30.0), ("2025Q4", 40.0)])
        >>> _ = b.compute("ebitda", "revenue * 0.25")
        >>> _ = b.compute("margin", "ebitda / revenue")
        >>> model = b.build()
        >>> spec = RollupSpec("annual", {"cash": "period_end", "margin": "formula"})
        >>> annual = Evaluator().evaluate(model).rollup(model, spec)
        >>> (annual.get("revenue", "2025"), annual.get("cash", "2025"), annual.get("margin", "2025"))
        (400.0, 40.0, 0.25)
        """
        ...

    def to_dated_schedule(
        self,
        model: FinancialModelSpec | str,
        node_id: str,
        convention: Literal["end", "start"] | None = None,
    ) -> list[tuple[date, float]]:
        """
        Export one node as a dated cashflow schedule.

        Mirrors Rust ``evaluator::node_to_dated_schedule`` (WASM
        ``statements.nodeToDatedSchedule``). Periods are taken in ``model``
        timeline order; periods without a value are skipped.

        Parameters
        ----------
        model : FinancialModelSpec | str
            The model that produced this result (its periods supply the
            dates); a typed model or its JSON.
        node_id : str
            Node identifier to export.
        convention : {"end", "start"}, optional
            ``"end"`` (the Rust default when omitted) dates each period on its
            last inclusive day (``end - 1 day``, since periods are half-open
            ``[start, end)``); ``"start"`` uses the period start date.

        Returns
        -------
        list[tuple[datetime.date, float]]
            ``(date, value)`` pairs in timeline order, in the node's own units.

        Raises
        ------
        KeyError
            If ``node_id`` has no values in the result.
        ValueError
            If ``convention`` is not ``"end"`` or ``"start"``.

        Examples
        --------
        >>> from finstack_quant.statements import Evaluator, ModelBuilder
        >>> builder = ModelBuilder("demo")
        >>> _ = builder.periods("2025Q1..Q1")
        >>> _ = builder.value("revenue", [("2025Q1", 100.0)])
        >>> model = builder.build()
        >>> Evaluator().evaluate(model).to_dated_schedule(model, "revenue")
        [(datetime.date(2025, 3, 31), 100.0)]
        """
        ...

    @property
    def node_sources(self) -> dict[str, dict[str, str]]:
        """
        Which evaluation layer produced each cell of the result.

        The evaluator resolves every (node, period) cell by the precedence
        **Value > Forecast > Formula**, after the node's optional ``where``
        mask. The model alone does not determine the outcome: an as-of
        visibility cutoff can hide an explicit value so that the forecast or
        formula fires instead.

        This property does not raise.

        Returns
        -------
        dict[str, dict[str, str]]
            Node identifier → period identifier (``"2025Q1"``) → source label:
            ``"value"`` (explicit value), ``"forecast"`` (forecast method),
            ``"formula"`` (formula) or ``"where_masked"`` (the ``where`` clause
            was false, so the cell is ``0.0``). Same keys as the evaluated
            values. Empty for a result that was not produced by an evaluator
            or was serialized before sources were recorded.
        """
        ...

    @property
    def check_report(self) -> CheckReport | None:
        """
        Check report attached by an evaluator configured with checks.

        Returns
        -------
        CheckReport | None
            The report produced by :meth:`Evaluator.with_checks`, or
            ``None`` when no check suite ran during evaluation.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def get_node(self, node_id: str) -> dict[str, float] | None:
        """
        Get every evaluated period for one node as a dict.

        Parameters
        ----------
        node_id:
            Node identifier.

        Returns
        -------
        dict[str, float] | None
            Period identifier string → value, in evaluation order, in the
            node's own units (currency amount for monetary nodes, unitless
            otherwise). ``None`` when the node is not in the result.

        Notes
        -----
        This method does not raise; an unknown *node_id* returns ``None``.
        """
        ...

    def node_ids(self) -> list[str]:
        """
        Return every node id present in this result set, in evaluation order.

        Returns
        -------
        list[str]
            Node ids as declared in the model graph.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def node_count(self) -> int:
        """
        Number of nodes in the result.

        Returns
        -------
        int
            Count of evaluated nodes.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def num_periods(self) -> int:
        """
        Number of periods evaluated.

        Returns
        -------
        int
            Count in **periods** on the model's own cadence (quarters, months,
            years), not months.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def eval_time_ms(self) -> int | None:
        """
        Wall-clock evaluation time in milliseconds, if recorded.

        Diagnostic only — it is not part of the deterministic result, so two
        identical runs may report different values.

        Returns
        -------
        int or None
            Elapsed milliseconds, or ``None`` when the producing run did not
            record it.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def warning_count(self) -> int:
        """
        Number of evaluation warnings recorded.

        Warnings never fail an evaluation; see :attr:`warnings` for what was
        flagged.

        Returns
        -------
        int
            Warning count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def warnings(self) -> list[dict[str, Any]]:
        """
        Evaluation warnings in their serde form, one dict per ``EvalWarning``.

        Each dict has a single snake_case variant key (``"division_by_zero"``,
        ``"non_finite_value"``, ``"non_finite_aggregate_input"``, ...) whose
        value holds the variant fields: node ids as strings, periods as
        ``"2025Q1"``-style ids and non-finite numbers as ``"nan"`` / ``"inf"``
        / ``"-inf"``. The same objects appear in ``to_json()["meta"]`` and in
        the WASM ``Evaluator.evaluate`` result.

        Returns
        -------
        list[dict[str, Any]]
            One dict per recorded warning.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def numeric_mode(self) -> NumericMode:
        """
        Numeric mode stamped into the result envelope (policy visibility).

        Returns
        -------
        NumericMode
            Always ``NumericMode.float64()`` today; the reserved decimal mode
            exists only so persisted payloads can evolve.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def parallel(self) -> bool:
        """
        Whether the evaluation ran in parallel (policy visibility).

        Returns
        -------
        bool
            ``True`` when the DAG was traversed in parallel. Parallel and
            serial runs produce identical values, so this is an audit stamp,
            not a behavioural flag.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self, orient: Literal["long", "wide"] = "long") -> pd.DataFrame:
        """
        Export results as a pandas DataFrame.

        Parameters
        ----------
        orient : {"long", "wide"}, default "long"
            ``"long"`` yields one row per (node, period) with columns
            ``node_id``, ``period``, ``value``, ``value_money``, ``currency``,
            ``value_type``, ``source``. ``"wide"`` yields node identifiers as
            rows and period identifiers as columns.

        Notes
        -----
        In long form the monetary columns are populated for nodes carrying
        currency information and are otherwise null. ``value_money`` is a
        float64 mirror of the monetary amount (f64, not fixed-point Decimal,
        precision); use ``to_json()`` or ``get_money()`` when full fixed-point
        precision is required. ``source`` is the evaluation layer that
        produced the cell (``value``, ``forecast``, ``formula`` or
        ``where_masked``, see :attr:`node_sources`); it is null when the
        result records no source for the cell.

        Returns
        -------
        pd.DataFrame
            Long- or wide-format frame.

        Raises
        ------
        ValueError
            If ``orient`` is not ``"long"`` or ``"wide"``.
        """
        ...

    def to_arrow_long(self) -> ArrowTable:
        """
        Export the long-format table via Arrow (zero-copy for consumers).

        Returns an `ArrowTable` implementing ``__arrow_c_stream__``; pass it
        to ``pyarrow.table(...)``, ``polars.DataFrame(...)``, or DuckDB.
        Column values and monetary-mirror semantics match ``to_dataframe("long")``,
        plus column roles and table metadata are preserved as Arrow
        field/schema metadata. One column name differs: the period column
        here is ``period_id`` (the table envelope's native name), whereas
        ``to_dataframe("long")`` renames it to ``period``.

        Returns
        -------
        ArrowTable
            Long-format Arrow table with one row per (node, period) pair.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_arrow_wide(self) -> ArrowTable:
        """
        Export the wide-format table via Arrow (zero-copy for consumers).

        Rows are periods (column ``period_id``), one ``float64`` column per
        node, matching ``to_dataframe("wide")`` before its transpose.

        Returns
        -------
        ArrowTable
            Wide-format Arrow table with one row per period.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise summary with node and period counts.
        Returns
        -------
        str
        """
        ...

    @property
    def cs_cashflows(self) -> CapitalStructureCashflows | None:
        """Return evaluated capital-structure cashflows when present.

        Returns
        -------
        CapitalStructureCashflows | None
            Per-instrument and reporting-currency totals, or None when capital structure was absent.

        Notes
        -----
        This accessor does not raise; it returns stored or derived model data.
        """
        ...

    def to_series(self, node_id: str) -> pd.Series:
        """Export one evaluated node as an ordered pandas Series.

        Parameters
        ----------
        node_id:
            Evaluated node identifier.

        Returns
        -------
        pd.Series
            Float64 node values indexed by period identifier, in the node's own units; use get_money for currency and fixed-point precision.

        Raises
        ------
        KeyError
            If the node is absent from the result.
        ImportError
            If pandas is unavailable.
        """
        ...

class Evaluator:
    """
    Evaluates a ``FinancialModelSpec`` into a ``StatementResult``.

    Examples
    --------
    >>> from finstack_quant.statements import Evaluator, ModelBuilder
    >>> builder = ModelBuilder("demo")
    >>> _ = builder.periods("2025Q1..Q1")
    >>> _ = builder.value("revenue", [("2025Q1", 100.0)])
    >>> Evaluator().evaluate(builder.build()).node_count
    1

    """

    def __init__(self) -> None:
        """
        Create a fresh evaluator with default configuration.

        Returns
        -------
        None

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.
        """
        ...

    def with_checks(self, suite_spec: CheckSuiteSpec) -> Evaluator:
        """
        Attach a check suite to run automatically after each evaluation.

        The suite spec is resolved (built-in and formula checks) and the
        resulting report is attached to
        :attr:`StatementResult.check_report` on every subsequent
        ``evaluate`` / ``evaluate_with_market`` call.

        Parameters
        ----------
        suite_spec:
            The check-suite specification to resolve and attach.

        Returns
        -------
        Evaluator
            This evaluator, for chaining.

        Raises
        ------
        ValueError
            If the suite spec cannot be resolved into runnable checks.

        Examples
        --------
        >>> from finstack_quant.statements import CheckSuiteSpec, Evaluator, ModelBuilder
        >>> spec = CheckSuiteSpec.from_json('{"name": "s", "builtin_checks": [], "formula_checks": []}')
        >>> builder = ModelBuilder("demo")
        >>> _ = builder.periods("2025Q1..Q1")
        >>> _ = builder.value("revenue", [("2025Q1", 100.0)])
        >>> evaluator = Evaluator().with_checks(spec)
        >>> evaluator.evaluate(builder.build()).check_report.passed
        True
        """
        ...

    def evaluate(self, model: FinancialModelSpec) -> StatementResult:
        """
        Evaluate ``model`` and return numeric results.

        A capital structure is evaluated from an empty market as of the first
        period's start, so fixed-rate debt yields ``cs_cashflows``;
        instruments that need market data (floating coupons, swaps, caps)
        need :meth:`evaluate_with_market`.

        Parameters
        ----------
        model:
            Specification produced by ``ModelBuilder.build`` or ``from_json``.

        Returns
        -------
        StatementResult
            Populated result object, with capital-structure cashflows when the
            model defines a capital structure.

        Raises
        ------
        ValueError
            If a formula is invalid or references an unknown identifier.
        KeyError
            If a referenced node or required input data is missing, or a
            capital-structure instrument needs market data.
        RuntimeError
            If the dependency graph has a cycle or a capital-structure
            computation fails.

        """
        ...

    def evaluate_with_market(
        self,
        model: FinancialModelSpec,
        market: MarketContext,
        as_of: date,
    ) -> StatementResult:
        """
        Evaluate ``model`` with market data and an as-of date.

        Use this when capital-structure instruments need market data
        (floating coupons, swaps, caps) and for as-of filtering of future
        actual periods.

        Parameters
        ----------
        model:
            Specification produced by ``ModelBuilder.build`` or ``from_json``.
        market:
            A :class:`MarketContext` with curves, FX, and vol surfaces.
        as_of:
            Valuation date for pricing, and the cutoff for explicit-value
            visibility: an actual whose availability date falls after
            ``as_of`` is hidden, so the node falls back to its forecast or
            formula (a value-only node then raises).

        Returns
        -------
        StatementResult
            Populated result object with market-aware valuations.

        Raises
        ------
        ValueError
            If ``as_of`` or a formula is invalid.
        KeyError
            If a referenced node, input data or required market data is missing.
        RuntimeError
            If the dependency graph has a cycle or a capital-structure
            computation fails.

        """
        ...

    def evaluate_monte_carlo(
        self,
        model: FinancialModelSpec,
        config: MonteCarloConfig,
    ) -> MonteCarloResults:
        """
        Run a Monte Carlo simulation of ``model`` under this evaluator.

        Parameters
        ----------
        model : FinancialModelSpec
            Model whose stochastic forecast nodes are sampled.
        config : MonteCarloConfig
            Path count, seed and requested percentiles.

        Returns
        -------
        MonteCarloResults
            Percentile summaries, warnings and optional retained paths.

        Raises
        ------
        ValueError
            If the model or configuration is rejected.
        RuntimeError
            If simulation fails.

        Examples
        --------
        >>> from finstack_quant.statements import Evaluator, ModelBuilder, MonteCarloConfig
        >>> builder = ModelBuilder("demo")
        >>> _ = builder.periods("2025Q1..Q2", "2025Q1")
        >>> _ = builder.value("revenue", [("2025Q1", 100.0), ("2025Q2", 110.0)])
        >>> Evaluator().evaluate_monte_carlo(builder.build(), MonteCarloConfig(2, 7, [0.5])).n_paths
        2

        """
        ...

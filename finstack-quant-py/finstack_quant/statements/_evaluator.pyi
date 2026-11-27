"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements/evaluator.rs``."""

from __future__ import annotations

from datetime import date
from typing import Any, Literal
import pandas as pd
import polars as pl
from finstack_quant.core.dates import Period
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.core.table import ArrowTable, LabeledSeries
from finstack_quant.statements._capital_structure import CapitalStructureCashflows
from finstack_quant.statements._checks import CheckReport, CheckSuiteSpec
from finstack_quant.statements._monte_carlo import MonteCarloConfig, MonteCarloResults
from finstack_quant.statements._rollup import RollupSpec
from finstack_quant.statements._types.model import (
    FinancialModelSpec,
    NumericMode,
)

class StatementResult:
    """
    Typed per-node, per-period results from evaluating a model.

    Every node has one cell per period: a value (a finite float, or exact
    tagged money for a monetary node), or a cell without a value that is
    ``missing``, ``masked`` or ``unavailable`` with a reason. A zero is a
    value; a cell without a value is never a zero. JSON and pickle preserve
    the cells exactly; non-finite numbers appear only in warnings, as
    ``"nan"``, ``"inf"`` and ``"-inf"`` strings.

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

        Returns the float view of a value cell: a scalar as stored, money as
        the nearest float. A cell without a value (missing, masked or
        unavailable) has no float view; use :meth:`get_cell` to see why.

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
            period is unknown, or the cell has no value.

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
        Return the value of a cell, with an explicit fallback for a missing observation.

        The fallback applies only to a ``missing`` cell at a node and period
        the result holds. It never stands in for a masked or unavailable
        cell, or for a node or period the result does not have.

        Parameters
        ----------
        node_id:
            Node identifier.
        period:
            Period label such as ``"2025Q1"``.
        default:
            Value returned for a missing observation, in the node's own units.

        Returns
        -------
        float
            The cell's value in the node's own units, or ``default`` when the
            cell is a missing observation.

        Raises
        ------
        ValueError
            If ``period`` cannot be parsed as a period id, or the cell is
            masked or unavailable.
        KeyError
            If the result has no cell for the node and period.

        Examples
        --------
        >>> from finstack_quant.statements import Evaluator, ModelBuilder
        >>> builder = ModelBuilder("demo")
        >>> _ = builder.periods("2025Q1..Q2")
        >>> _ = builder.value("revenue", [("2025Q1", 100.0)])
        >>> result = Evaluator().evaluate(builder.build())
        >>> (result.get_or("revenue", "2025Q1", 0.0), result.get_or("revenue", "2025Q2", 0.0))
        (100.0, 0.0)
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
            ``(period, value)`` pairs of the node's value cells in timeline
            order, in the node's own units. Cells without a value are left
            out. Empty when the node is not in the result.

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

    @property
    def periods(self) -> list[Period]:
        """
        The periods this result is keyed by, in timeline order.

        The model's periods for an evaluated result; the target periods for a
        rolled-up one. Empty only for a result deserialized with no periods.

        Returns
        -------
        list[Period]
            Each period's id, inclusive start, exclusive end and actual flag.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> from finstack_quant.statements import Evaluator, ModelBuilder
        >>> builder = ModelBuilder("demo")
        >>> _ = builder.periods("2025Q1..Q1")
        >>> _ = builder.value("revenue", [("2025Q1", 100.0)])
        >>> [str(period.id) for period in Evaluator().evaluate(builder.build()).periods]
        ['2025Q1']
        """
        ...

    def rollup(self, model: FinancialModelSpec, spec: RollupSpec) -> StatementResult:
        """
        Roll these results up to a coarser frequency, such as quarterly to annual.

        Each target period combines its source periods node by node by the
        node's method, from ``spec`` or else the node's declaration. ``"sum"``
        adds the source cells (money exactly, scalars with a compensated sum)
        and is unavailable when a source cell has no value. ``"period_end"``
        copies the last source cell with its status (balances, LTM metrics).
        ``"formula"`` re-evaluates the node's mask and formula on the rolled-up
        values (ratios); ``lag`` and other time-series functions then step in
        target periods. Nothing is summed by default: a node with no method
        fails the rollup. Every target period must be complete.

        Parameters
        ----------
        model : FinancialModelSpec
            The model these results were evaluated from. It supplies the period dates and the formulas of
            ``"formula"`` nodes.
        spec : RollupSpec
            Target frequency and per-node methods.

        Returns
        -------
        StatementResult
            Values keyed by the target periods (``"2025"``, ``"2025H1"``,
            ``"2025Q1"``; fiscal periods keep their ``FY`` prefix), with
            monetary values, value types and those periods in ``periods`` (each
            actual only when every source period is) and
            :attr:`rollup_provenance`. It is not an evaluation run: it carries
            no ``provenance``, per-cell ``source``, capital-structure cashflows
            or check report.

        Raises
        ------
        ValueError
            If these results are not a verified run of
            ``model`` (no provenance, or another model content, source snapshot
            or period plan), the target frequency is not a whole multiple of
            the model's, a target period is incomplete, a method names an
            unknown node, a node has no method, a ``"formula"`` node has no
            formula or reads ``cs.*`` values, a sum cannot be represented, or a
            re-evaluated formula fails.

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
        >>> spec = RollupSpec(
        ...     "annual",
        ...     {"revenue": "sum", "cash": "period_end", "ebitda": "sum", "margin": "formula"},
        ... )
        >>> annual = Evaluator().evaluate(model).rollup(model, spec)
        >>> (annual.get("revenue", "2025"), annual.get("cash", "2025"), annual.get("margin", "2025"))
        (400.0, 40.0, 0.25)
        """
        ...

    def to_dated_schedule(self, node_id: str, convention: Literal["end", "start"]) -> list[dict[str, Any]]:
        """
        Export every period of one node as dated cells.

        Mirrors Rust ``evaluator::node_to_dated_schedule`` (WASM
        ``statements.nodeToDatedSchedule``). Periods come from the result's
        own period plan in timeline order. A period without a value stays in
        the schedule with its status and reason, so a gap is visible. The
        dates are period anchors, not payment dates.

        Parameters
        ----------
        node_id : str
            Node identifier to export.
        convention : {"end", "start"}
            ``"end"`` anchors each period on its last included day
            (``end - 1 day``, since periods are half-open ``[start, end)``);
            ``"start"`` on its first day. There is no default.

        Returns
        -------
        list[dict[str, Any]]
            One row per result period: ``date`` (``datetime.date``),
            ``period_id`` and ``cell``, the typed cell of :attr:`cells` with
            its exact amount and currency.

        Raises
        ------
        KeyError
            If ``node_id`` is not in the result.
        ValueError
            If ``convention`` is not ``"end"`` or ``"start"``, or the result
            has no period plan.

        Examples
        --------
        >>> from finstack_quant.statements import Evaluator, ModelBuilder
        >>> builder = ModelBuilder("demo")
        >>> _ = builder.periods("2025Q1..Q2")
        >>> _ = builder.value("revenue", [("2025Q1", 100.0)])
        >>> rows = Evaluator().evaluate(builder.build()).to_dated_schedule("revenue", "end")
        >>> [(row["date"].isoformat(), row["cell"]["status"]) for row in rows]
        [('2025-03-31', 'value'), ('2025-06-30', 'missing')]
        """
        ...

    def to_pricing_flows(self, selection: dict[str, Any] | str) -> list[tuple[date, Money]]:
        """
        Read one monetary node as signed economic cash flows for pricing.

        Mirrors Rust ``evaluator::node_to_pricing_flows`` (WASM
        ``statements.nodeToPricingFlows``). Every requested period must hold
        an exact amount in the expected currency. Positive means cash
        received: an ``"outflow_magnitude"`` node is negated once. No FX and
        no sign inference is applied.

        Parameters
        ----------
        selection : dict[str, Any] | str
            A ``PricingFlowSelection`` mapping or its JSON: ``node_id``, the
            expected ``currency`` code, ``sign`` (``"inflow_positive"`` for a
            signed cash flow or ``"outflow_magnitude"`` for a non-negative
            outflow such as CapEx), the required ``periods`` and
            ``date_convention`` (``"start"`` or ``"end"``).

        Returns
        -------
        list[tuple[datetime.date, Money]]
            ``(date, amount)`` pairs in timeline order, one per requested
            period.

        Raises
        ------
        KeyError
            If the node is not in the result.
        ValueError
            If the selection is malformed, the result has no period plan, a
            requested period is repeated or not in the result, a cell is
            missing, masked, unavailable or a scalar, an amount is in another
            currency, or an outflow magnitude is negative.

        Examples
        --------
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.statements import Evaluator, ModelBuilder
        >>> builder = ModelBuilder("demo")
        >>> _ = builder.periods("2025M03..M05")
        >>> _ = builder.value_money("capex", [("2025M03", Money(125.01, "USD"))])
        >>> result = Evaluator().evaluate(builder.build())
        >>> selection = {
        ...     "node_id": "capex",
        ...     "currency": "USD",
        ...     "sign": "outflow_magnitude",
        ...     "periods": ["2025M03"],
        ...     "date_convention": "end",
        ... }
        >>> [(day.isoformat(), str(amount)) for day, amount in result.to_pricing_flows(selection)]
        [('2025-03-31', 'USD -125.01')]
        """
        ...

    @property
    def cells(self) -> dict[str, dict[str, dict[str, Any]]]:
        """
        The typed result: one cell per node and period.

        This property does not raise.

        Returns
        -------
        dict[str, dict[str, dict[str, Any]]]
            Node identifier → period identifier (``"2025Q1"``) → cell. A cell
            has ``status`` (``"value"``, ``"missing"``, ``"masked"`` or
            ``"unavailable"``); a value cell has ``value`` (a finite float for
            a scalar node, ``{"amount": "125.01", "currency": "USD"}`` with an
            exact decimal string for a monetary node); an unavailable cell
            has ``reason`` (for example ``"missing_mask"``,
            ``"hidden_by_cutoff"`` or ``"missing_input"``); and ``source``
            (``"value"``, ``"forecast"`` or ``"formula"``) names the value
            source selected for the cell, when one was. A zero is a value; a
            cell without a value is never a zero.
        """
        ...

    def get_cell(self, node_id: str, period: str) -> dict[str, Any] | None:
        """
        Get one typed cell with its status, value, reason and source.

        Parameters
        ----------
        node_id:
            Node identifier.
        period:
            Period label such as ``"2025Q1"``.

        Returns
        -------
        dict[str, Any] | None
            The cell in the shape described by :attr:`cells`, or ``None``
            when the result has no cell for the node and period.

        Raises
        ------
        ValueError
            If ``period`` cannot be parsed as a period id.

        Examples
        --------
        >>> from finstack_quant.statements import Evaluator, ModelBuilder
        >>> builder = ModelBuilder("demo")
        >>> _ = builder.periods("2025Q1..Q2")
        >>> _ = builder.value("revenue", [("2025Q1", 100.0)])
        >>> result = Evaluator().evaluate(builder.build())
        >>> result.get_cell("revenue", "2025Q1")
        {'status': 'value', 'value': 100.0, 'source': 'value'}
        >>> result.get_cell("revenue", "2025Q2")
        {'status': 'missing'}
        """
        ...

    @property
    def provenance(self) -> dict[str, Any] | None:
        """
        What the result was evaluated from.

        This property does not raise.

        Returns
        -------
        dict[str, Any] | None
            ``model_id``, ``model_content_hash``, optional
            ``source_snapshot``, ``observation_cutoff`` (``"full_snapshot"``
            or ``{"through": "2025-04-25"}``), optional ``valuation_date``,
            ``market_content_hash`` for an explicitly priced run with a coherent captured market, and
            ``forecast_streams`` when the model has stochastic forecasts
            (per node: ``node_id``, ``stream_version``, and the ``seed``,
            ``state`` and ``stream`` of its PCG64 generator as decimal
            strings, enough to replay the draws) and ``run_id`` (the
            content hash of the other fields). ``None``
            for a result that was not produced by an evaluator.
        """
        ...

    @property
    def rollup_provenance(self) -> dict[str, Any] | None:
        """
        How a rolled-up result was produced.

        This property does not raise.

        Returns
        -------
        dict[str, Any] | None
            ``source_run`` (the :attr:`provenance` of the result that was
            rolled up), ``frequency`` (the target frequency label),
            ``methods`` (node id to the ``"sum"``, ``"period_end"`` or
            ``"formula"`` method used, in model order) and ``source_periods``
            (target period id to its source period ids, in timeline order).
            ``None`` for a result that :meth:`rollup` did not produce.
        """
        ...

    @property
    def normalizations(self) -> dict[str, Any]:
        """
        The configuration of every adjusted metric published into this result.

        Returns
        -------
        dict[str, Any]
            Output node id to the ``NormalizationConfig`` (its serde form)
            that :func:`merge_into_results` calculated it from, in publication
            order; empty when nothing was published. The source run is this
            result's ``provenance`` or ``rollup_provenance``.

        Raises
        ------
        ValueError
            If a stored configuration cannot be serialized.
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

    def numeric_series(self, node_id: str) -> dict[str, float] | None:
        """
        Get every evaluated period for one node as a dict.

        Parameters
        ----------
        node_id:
            Node identifier.

        Returns
        -------
        dict[str, float] | None
            Period identifier string → value for the node's value cells, in
            timeline order, in the node's own units (currency amount for
            monetary nodes, unitless otherwise). Cells without a value are
            left out. ``None`` when the node is not in the result.

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

    def to_arrow(self, orient: Literal["long", "wide"] = "long") -> ArrowTable:
        """
        Export results as an ``ArrowTable`` (zero-copy via ``__arrow_c_stream__``).

        Parameters
        ----------
        orient : {"long", "wide"}, default "long"
            ``"long"`` is the exact table: one row per node and period of the
            result's plan, including cells without a value, with columns
            ``node_id``, ``period_id``, ``status``, ``value``, ``amount``,
            ``currency``, ``value_type``, ``reason``, ``source``. ``"wide"``
            puts one row per period (column ``period_id``) and one nullable
            ``float64`` column per node whose field metadata carries its units
            (``value_type`` and, for money, ``currency``).

        Notes
        -----
        In long form ``status`` is ``value``, ``missing``, ``masked`` or
        ``unavailable``; ``value`` is the float view of a value cell and null
        otherwise; ``amount`` is the exact decimal text of a monetary value
        cell and ``currency`` its ISO code (both null for scalar and
        value-less cells); ``reason`` says why an unavailable cell has no
        value; and ``source`` is the value source selected for the cell
        (``value``, ``forecast`` or ``formula``), null when none was. In wide
        form a cell without a value is null, never zero.

        Returns
        -------
        ArrowTable
            Long- or wide-format Arrow table with field/schema metadata preserved.

        Raises
        ------
        ValueError
            If ``orient`` is not ``"long"`` or ``"wide"``.
        """
        ...

    def to_pandas(self, orient: Literal["long", "wide"] = "long") -> pd.DataFrame:
        """
        Export results as a pandas ``DataFrame``.

        Parameters
        ----------
        orient : {"long", "wide"}, default "long"
            ``"long"`` returns one row per node and period with columns
            ``node_id``, ``period_id``, ``status``, ``value``, ``amount``,
            ``currency``, ``value_type``, ``reason``, ``source``. ``"wide"``
            is the approximate numeric view with node identifiers as rows and
            period identifiers as columns.

        Returns
        -------
        pandas.DataFrame
            Long- or wide-format frame.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.
        ValueError
            If ``orient`` is not ``"long"`` or ``"wide"``.
        """
        ...

    def to_polars(self, orient: Literal["long", "wide"] = "long") -> pl.DataFrame:
        """
        Export results as a Polars ``DataFrame``.

        Parameters
        ----------
        orient : {"long", "wide"}, default "long"
            ``"long"`` returns one row per (node, period) pair with column
            ``period_id``; ``"wide"`` returns one row per period (``period_id``)
            with one float column per node.

        Returns
        -------
        polars.DataFrame
            Long- or wide-format Polars frame.

        Raises
        ------
        ImportError
            If ``polars`` is not installed.
        ValueError
            If ``orient`` is not ``"long"`` or ``"wide"``.
        """
        ...

    def to_pyarrow(self, orient: Literal["long", "wide"] = "long") -> Any:
        """
        Export results as a ``pyarrow.Table``.

        Parameters
        ----------
        orient : {"long", "wide"}, default "long"
            ``"long"`` returns one row per (node, period) pair with column
            ``period_id``; ``"wide"`` returns one row per period (``period_id``)
            with one float column per node.

        Returns
        -------
        pyarrow.Table
            Long- or wide-format PyArrow table with schema metadata preserved.

        Raises
        ------
        ImportError
            If ``pyarrow`` is not installed.
        ValueError
            If ``orient`` is not ``"long"`` or ``"wide"``.
        """
        ...

    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...
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

    def to_series(self, node_id: str) -> LabeledSeries:
        """Export one evaluated node as an ordered ``LabeledSeries``.

        Parameters
        ----------
        node_id:
            Evaluated node identifier.

        Returns
        -------
        LabeledSeries
            Float64 node values indexed by ``period_id`` (null where a cell has
            no value), in the node's own units. Use ``get_money`` for currency
            and fixed-point precision.

        Raises
        ------
        KeyError
            If the node is absent from the result.
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
        >>> evaluator.evaluate(builder.build()).check_report.outcome
        'unevaluated'
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
        observation_cutoff: date | str,
    ) -> StatementResult:
        """
        Evaluate ``model`` with market data and an as-of date.

        Use this when capital-structure instruments need market data
        (floating coupons, swaps, caps) and for point-in-time evaluation.

        Parameters
        ----------
        model:
            Specification produced by ``ModelBuilder.build`` or ``from_json``.
        market:
            A :class:`MarketContext` with curves, FX, and vol surfaces.
        as_of:
            Valuation date for pricing the capital structure. It does not
            hide observations.
        observation_cutoff:
            ``"full_snapshot"`` reads every observation in the model. A date
            (or ISO ``YYYY-MM-DD`` string) reads an observation only when its
            availability date is on or before that date, inclusive; the model
            must then name its ``source_snapshot`` and give an availability
            date to every observation in an actual period. An undated
            explicit value in a forecast period is a forecast assumption and
            stays visible. A hidden value makes the node fall back to its
            forecast or formula (a value-only node then raises).

        Returns
        -------
        StatementResult
            Populated result object with market-aware valuations.

        Raises
        ------
        ValueError
            If ``as_of``, ``observation_cutoff`` or a formula is invalid, or a
            cutoff date is used on a model with no ``source_snapshot`` or
            with an undated observation in an actual period.
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
        market: MarketContext | None = None,
        as_of: date | str | None = None,
        observation_cutoff: date | str | None = None,
    ) -> MonteCarloResults:
        """
        Run a Monte Carlo simulation of ``model`` under this evaluator.

        Each path evaluates the complete model on that path's sampled
        forecasts with fresh state, so a capital structure and its waterfall
        run per path and ``cs.*`` references see that path's debt service and
        balances. The requested metrics are aggregated into R-7 sample
        quantiles per forecast period.

        Parameters
        ----------
        model : FinancialModelSpec
            Model whose stochastic forecast nodes are sampled. It needs at
            least one forecast period.
        config : MonteCarloConfig
            Path count, seed, the metrics to report and the percentiles.
        market : MarketContext or None, default None
            Market the capital structure is projected on, as for
            ``evaluate_with_market``. ``None`` evaluates as ``evaluate`` does:
            the full snapshot on an empty market as of the first period's
            start. Give ``market`` and ``as_of`` together.
        as_of : date or str or None, default None
            Valuation date of the capital structure's contractual flows;
            required with ``market``.
        observation_cutoff : date or str or None, default None
            ``"full_snapshot"`` (the default with a market) or the date
            through which explicit observations are visible.

        Returns
        -------
        MonteCarloResults
            Percentile summaries for a complete run. When a requested sample
            has no value on some path, no statistics are published and
            ``incomplete`` names the first such sample.

        Raises
        ------
        ValueError
            If the model or configuration is rejected (a path count of zero,
            no, unknown or repeated metrics, repeated or out-of-range
            percentiles, a run above the sample limit, no forecast period),
            only one of ``market`` and ``as_of`` is given, or a path fails.
        KeyError
            If a formula references an unknown node or required market data
            is missing.
        RuntimeError
            If a capital-structure calculation fails on a path.

        Examples
        --------
        >>> from finstack_quant.statements import Evaluator, ModelBuilder, MonteCarloConfig
        >>> builder = ModelBuilder("demo")
        >>> _ = builder.periods("2025Q1..Q2", "2025Q1")
        >>> _ = builder.value("revenue", [("2025Q1", 100.0), ("2025Q2", 110.0)])
        >>> config = MonteCarloConfig(2, 7, ["revenue"], [0.5])
        >>> Evaluator().evaluate_monte_carlo(builder.build(), config).n_paths
        2

        """
        ...

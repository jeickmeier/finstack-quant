"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements/builder.rs``."""

from __future__ import annotations

from datetime import date
from typing import Any
import pandas as pd
import polars as pl
from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import PeriodPlan
from finstack_quant.core.money import Money
from finstack_quant.statements._capital_structure import WaterfallSpec
from finstack_quant.statements._registry import Registry
from finstack_quant.statements._types import FinancialModelSpec, ForecastSpec, NodeSpec

class ModelBuilder:
    """
    Builder for a ``FinancialModelSpec``.

    Call ``periods`` once, then add nodes with ``value`` / ``compute``, and
    finish with ``build``.

    Note
    ----
    Configuration methods mutate the builder in place *and* return it, so they
    can be chained or called as separate statements. ``build`` and ``mixed``
    are terminal: they consume the builder, and any later configuration call
    raises ``ValueError``. A rejected configuration argument leaves the
    accumulated model unchanged, so a corrected setter can be retried.

    Examples
    --------
    Chained:

    >>> from finstack_quant.statements import ModelBuilder
    >>> builder = ModelBuilder("demo").periods("2025Q1..Q1")
    >>> model = builder.value("revenue", [("2025Q1", 100.0)]).build()
    >>> model.node_ids()
    ['revenue']

    Statement per line:

    >>> builder = ModelBuilder("demo")
    >>> _ = builder.periods("2025Q1..Q1")
    >>> _ = builder.value("revenue", [("2025Q1", 100.0)])
    >>> builder.build().node_ids()
    ['revenue']

    """

    def __init__(self, id: str) -> None:
        """
        Start a new builder for a model with the given id.

        Parameters
        ----------
        id:
            Model identifier assigned to the built ``FinancialModelSpec``.

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.

        Examples
        --------
        >>> from finstack_quant.statements import ModelBuilder
        >>> builder = ModelBuilder("demo")
        >>> _ = builder.periods("2025Q1..Q1")
        >>> _ = builder.value("revenue", [("2025Q1", 100.0)])
        >>> builder.build().id
        'demo'

        Notes
        -----
        This constructor does not raise; validation happens in ``build()``.
        """
        ...

    @staticmethod
    def from_spec(spec: FinancialModelSpec) -> ModelBuilder:
        """Reconstruct a ready builder from an existing model specification.

        Parameters
        ----------
        spec : FinancialModelSpec
            Typed model whose periods, nodes, metadata, and capital structure
            seed the builder.

        Returns
        -------
        ModelBuilder
            Ready builder that can accept additional transformations.

        Raises
        ------
        ValueError
            If ``spec`` has no periods.

        Examples
        --------
        >>> from finstack_quant.statements import ModelBuilder
        >>> original = ModelBuilder("demo").periods("2025Q1..Q1").value("revenue", [("2025Q1", 100.0)]).build()
        >>> rebuilt = ModelBuilder.from_spec(original).compute("double", "revenue * 2").build()
        >>> rebuilt.node_ids()
        ['revenue', 'double']
        """
        ...

    def periods(self, range: str, actuals_until: str | None = None) -> ModelBuilder:
        """
        Define the model's period lattice from a range expression.

        Parameters
        ----------
        range:
            Period range expression such as ``"2025Q1..Q4"``.
        actuals_until:
            Optional last actual period label; ``None`` if not used.

        Returns
        -------
        ModelBuilder
            This builder, for chaining.

        Raises
        ------
        ValueError
            If periods are already set, the range is invalid, or the builder was consumed.

        """
        ...

    def constant(self, node_id: str, value: Money | float) -> ModelBuilder:
        """Broadcast a scalar or monetary observation across the model timeline.

        Parameters
        ----------
        node_id : str
            Unique identifier for the new value node.
        value : Money or float
            Monetary amount in its explicit currency or finite unitless scalar.
            Every modeled period receives the same value.

        Returns
        -------
        ModelBuilder
            The builder with the constant node added.

        Raises
        ------
        ValueError
            If periods are unset, the scalar is nonfinite, or the builder was consumed.
        TypeError
            If the value is neither Money nor numeric.
        """
        ...

    def value(self, node_id: str, values: list[tuple[str, float]]) -> ModelBuilder:
        """
        Add a value node with explicit per-period scalars.

        Parameters
        ----------
        node_id:
            Identifier for the new node.
        values:
            ``(period_id, value)`` pairs, for example ``[("2025Q1", 1.0)]``.

        Returns
        -------
        ModelBuilder
            This builder, for chaining.

        Raises
        ------
        ValueError
            If periods were not configured, a period id is invalid, or the builder was consumed.

        """
        ...

    def value_scalar(self, node_id: str, values: list[tuple[str, float]]) -> ModelBuilder:
        """
        Add a scalar value node with explicit per-period values.

        Parameters
        ----------
        node_id:
            Identifier for the new node.
        values:
            ``(period_id, value)`` pairs, for example ``[("2025Q1", 1.0)]``.

        Returns
        -------
        ModelBuilder
            This builder, for chaining.

        Raises
        ------
        ValueError
            If periods were not configured, a period id is invalid, or the builder was consumed.

        """
        ...

    def value_money(self, node_id: str, values: list[tuple[str, Money]]) -> ModelBuilder:
        """
        Add a monetary value node with explicit per-period values.

        Parameters
        ----------
        node_id:
            Identifier for the new node.
        values:
            ``(period_id, Money)`` pairs, for example ``[("2025Q1", Money(100.0, "USD"))]``.

        Returns
        -------
        ModelBuilder
            This builder, for chaining.

        Raises
        ------
        ValueError
            If periods were not configured, a period id is invalid, or the builder was consumed.

        """
        ...

    def availability_dates(
        self,
        node_id: str,
        availability_dates: list[tuple[str, date | str]],
    ) -> ModelBuilder:
        """
        Set when explicit observations became available point-in-time.

        Parameters
        ----------
        node_id:
            Existing value or mixed node.
        availability_dates:
            ``(period_id, available_on)`` pairs. Unspecified observations
            default to the reporting period's exclusive end date.

        Returns
        -------
        ModelBuilder
            This builder, for chaining.

        Raises
        ------
        ValueError
            If a period/date is invalid, the node does not exist, or the
            builder was consumed.
        """
        ...

    def rollup(self, node_id: str, rollup: str) -> ModelBuilder:
        """
        Declare how an existing line rolls up to a coarser reporting frequency.

        Call it after the method that creates the line, such as :meth:`value`
        or :meth:`compute`. Redefining the node discards the declaration.

        Parameters
        ----------
        node_id:
            Existing node to declare the method on.
        rollup:
            ``"sum"`` for flows, ``"period_end"`` for balances and trailing
            metrics, ``"formula"`` for ratios. A rollup rejects ``"formula"``
            on a node without a formula, and a :class:`RollupSpec` entry for
            the node overrides this declaration.

        Returns
        -------
        ModelBuilder
            This builder, for chaining.

        Raises
        ------
        ValueError
            If ``rollup`` is not a recognized method, the node does not exist,
            or the builder was consumed.

        Examples
        --------
        >>> from finstack_quant.statements import ModelBuilder
        >>> builder = ModelBuilder("demo")
        >>> _ = builder.periods("2025Q1..Q1")
        >>> _ = builder.value("cash", [("2025Q1", 40.0)])
        >>> builder.rollup("cash", "period_end").build().get_node("cash").rollup
        'period_end'
        """
        ...

    def compute(self, node_id: str, formula: str) -> ModelBuilder:
        """
        Add a calculated node from a DSL formula.

        Parameters
        ----------
        node_id:
            Identifier for the computed node.
        formula:
            Expression in the statements DSL (for example ``"revenue - cogs"``).

        Returns
        -------
        ModelBuilder
            This builder, for chaining.

        Raises
        ------
        ValueError
            If the formula fails to compile or the builder state is invalid.

        """
        ...

    def mixed(self, node_id: str) -> MixedNodeBuilder:
        """
        Start configuring a mixed node and consume this builder until ``build`` returns.

        Parameters
        ----------
        node_id:
            Identifier for the new mixed node.

        Returns
        -------
        MixedNodeBuilder
            A builder for the mixed node.  Call ``build`` on the returned
            builder to attach the node and resume this builder.

        Raises
        ------
        ValueError
            If periods have not been configured or the builder has already been consumed.

        """
        ...

    def forecast(self, node_id: str, forecast_spec: ForecastSpec) -> ModelBuilder:
        """
        Attach a forecast to an existing node or create a forecast-only mixed node.

        Parameters
        ----------
        node_id:
            Identifier for the node to forecast.
        forecast_spec:
            A :class:`ForecastSpec` describing the projection method and parameters.

        Returns
        -------
        ModelBuilder
            This builder, for chaining.

        Raises
        ------
        ValueError
            If periods have not been configured or the builder has already been consumed.

        """
        ...

    def where_clause(self, node_id: str, where_clause: str) -> ModelBuilder:
        """
        Attach a scalar predicate to a named node.

        The predicate is evaluated before the node's value source. Where it
        is false for a period, the node's value, forecast and formula are not
        consulted.

        Parameters
        ----------
        node_id:
            Identifier of an existing node the predicate masks.
        where_clause:
            DSL expression evaluated per period as a scalar predicate.

        Returns
        -------
        ModelBuilder
            This builder, for chaining.

        Raises
        ------
        ValueError
            If the predicate does not parse and compile, ``node_id`` is not
            defined, or the builder has already been consumed. A rejected
            call leaves the builder unchanged.

        """
        ...

    def currency(self, currency: Currency | str) -> ModelBuilder:
        """Set the statement model currency used by statement analytics.

        Parameters
        ----------
        currency : Currency | str
            Currency handle or ISO 4217 code for the model's amounts.

        Returns
        -------
        ModelBuilder
            This builder, for fluent construction.

        Raises
        ------
        ValueError
            If the currency code is invalid or the builder has been consumed.
        TypeError
            If the value is neither a currency handle nor a string.

        Examples
        --------
        >>> from finstack_quant.statements import ModelBuilder
        >>> builder = ModelBuilder("income").currency("USD")
        >>> builder is builder.currency("EUR")
        True
        """
        ...

    def as_money(self, node_id: str, source: str, currency: Currency | str | None = None) -> ModelBuilder:
        """Denominate a scalar expression as money in the stated currency.

        Parameters
        ----------
        node_id : str
            Unique identifier of the new calculated node.
        source : str
            Scalar formula or source node in units of the selected currency.
        currency : Currency or str or None
            Explicit ISO denomination, or None to use the stored model currency.

        Returns
        -------
        ModelBuilder
            The current builder with the new monetary expression.

        Raises
        ------
        ValueError
            If IDs or syntax are invalid, or no valid currency is available.
            Build rejects a source that already has money or Boolean units.
        """
        ...

    def with_meta(self, key: str, value: Any) -> ModelBuilder:
        """
        Add model-level metadata from a Python value.

        Parameters
        ----------
        key:
            Namespaced model-metadata key used to identify the supplied JSON
            value in serialized model output. ``currency`` selects the model's
            reporting currency.
        value:
            JSON-serializable metadata value. Strings are stored verbatim. For
            ``currency``, supply a supported
            three-letter currency-code string such as ``"USD"``; ``build()``
            raises ``ValueError`` for malformed currency metadata.

        Returns
        -------
        ModelBuilder
            This builder, for chaining.

        Raises
        ------
        ValueError
            If ``value`` contains a non-finite float, periods are not configured, or
            the builder has already been consumed.
        TypeError
            If ``value`` cannot be serialized as JSON.

        """
        ...

    def with_builtin_metrics(self) -> ModelBuilder:
        """
        Add all built-in statement metrics to the model.

        The catalogue reads many input nodes and :meth:`build` resolves every
        formula reference, so the model must define all of them. Use
        :meth:`add_metric_from_registry` to add a subset. The import is
        atomic. A node that already holds a metric's unchanged template is
        shared.

        Returns
        -------
        ModelBuilder
            This builder, for chaining.

        Raises
        ------
        ValueError
            If a node with a built-in metric's name is already defined
            differently. Import the metric first and change it with
            :meth:`replace_node`.
        """
        ...

    def add_metric_from_registry(self, qualified_id: str, registry: Registry) -> ModelBuilder:
        """
        Add one metric and its dependency closure from a metric registry.

        The import is atomic, with dependencies first. A node that already
        holds the same unchanged template is shared; a node of that name with
        another definition is a conflict. Override a metric with
        :meth:`replace_node` after importing it.

        Parameters
        ----------
        qualified_id:
            Fully qualified metric identifier.
        registry:
            A :class:`Registry` containing the metric definition.

        Returns
        -------
        ModelBuilder
            This builder, for chaining.

        Raises
        ------
        ValueError
            If the builder has already been consumed, or a node of the
            metric's or a dependency's name is already defined differently.
        KeyError
            If qualified_id is not in registry.

        """
        ...

    def add_debt(self, id: str, instrument: Any | str) -> ModelBuilder:
        """
        Attach a canonical debt or hedge instrument to the capital structure.

        This is the one way an instrument enters a model. Build the
        instrument through its own Valuations API (``Bond.fixed``,
        ``Bond.with_convention``, ``InterestRateSwap.from_conventions``,
        ``TermLoan`` ...) with fully authored terms or a deliberately selected
        convention; the model keeps its resolved terms and schedule unchanged.
        Supported instrument types are bonds, convertible bonds, revolving
        credit facilities, term loans, interest-rate swaps, caps/floors and
        swaptions.

        Parameters
        ----------
        id:
            Capital-structure identifier of the instrument, used by
            ``cs.<component>.<id>`` references and waterfall classes.
            Market-aware evaluation rejects duplicate identifiers.
        instrument:
            A typed instrument object, or its canonical
            ``finstack_quant.instrument/1`` envelope as a dict or JSON text.

        Returns
        -------
        ModelBuilder
            This builder, for chaining.

        Raises
        ------
        ValueError
            If the envelope is invalid or its instrument type is not supported
            by financial statement capital structures.

        Examples
        --------
        >>> from datetime import date
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.statements import ModelBuilder
        >>> from finstack_quant.valuations.instruments import Bond
        >>> bond = Bond.with_convention(
        ...     "UST-5Y",
        ...     Money(1_000_000.0, Currency("USD")),
        ...     0.03,
        ...     date(2025, 1, 15),
        ...     date(2030, 1, 15),
        ...     "us_treasury",
        ...     "USD-OIS",
        ... )
        >>> builder = ModelBuilder("deal")
        >>> _ = builder.add_debt("UST-5Y", bond)
        >>> _ = builder.periods("2025Q1..Q2", None).value_scalar("revenue", {"2025Q1": 1.0})
        >>> [entry["id"] for entry in builder.build().capital_structure["debt_instruments"]]
        ['UST-5Y']
        """
        ...

    def reporting_currency(self, currency: Currency | str) -> ModelBuilder:
        """
        Set the reporting currency used for capital-structure totals.

        Parameters
        ----------
        currency:
            Currency handle or ISO 4217 currency code such as ``"USD"``.

        Returns
        -------
        ModelBuilder
            This builder, for chaining.

        Raises
        ------
        ValueError
            If the builder has already been consumed or the code is unknown.
        TypeError
            If the currency is neither a handle nor a string.

        """
        ...

    def fx_policy(self, policy: str) -> ModelBuilder:
        """
        Choose how capital-structure totals are consolidated across currencies.

        Parameters
        ----------
        policy:
            ``"period_end"``, the only policy: each period's native-currency
            totals convert at the rate of the inclusive period-close date. It
            must be set before instruments of more than one currency are
            consolidated into the reporting currency; there is no default.

        Returns
        -------
        ModelBuilder
            This builder, for chaining.

        Raises
        ------
        ValueError
            If policy is unknown or the builder has already been consumed.

        """
        ...

    def waterfall(self, waterfall_spec: WaterfallSpec) -> ModelBuilder:
        """
        Attach a waterfall specification (priorities, ECF sweep, PIK toggle,
        payment classes, and optional prepay nodes).

        Parameters
        ----------
        waterfall_spec : WaterfallSpec
            A :class:`WaterfallSpec` defining cash distribution priorities.
            Bond / ConvertibleBond plus a sweep is rejected at model build.

        Returns
        -------
        ModelBuilder
            This builder, for chaining.

        Raises
        ------
        ValueError
            If the builder has already been consumed.

        """
        ...

    def build(self) -> FinancialModelSpec:
        """
        Materialize the ``FinancialModelSpec`` and consume the builder.

        Runs the one runnable-model validation: a canonical contiguous
        timeline with actuals as a prefix, node identifiers that follow the
        formula identifier rules, node kinds with only their permitted
        sources (a ``value`` node takes observations only; a ``calculated``
        node needs a formula), finite observations inside the timeline,
        forecasts, formula dimensions, resolved formula references and no
        same-period dependency cycle. A missing cell is left out of a series;
        it is never stored as NaN. ``period_id`` is reserved for the timeline
        column in result exports.

        Returns
        -------
        FinancialModelSpec
            Completed specification.

        Raises
        ------
        ValueError
            If the builder was already consumed, the model has no node and no capital-structure instrument, or the model violates any rule above, including a formula that references an unknown node.

        """
        ...

    def periods_explicit(self, plan: PeriodPlan) -> ModelBuilder:
        """Set the model timeline from an explicit period plan.

        Parameters
        ----------
        plan:
            Non-empty PeriodPlan (for example from ``build_periods`` or ``build_fiscal_periods``) whose calendar context the model retains. Its periods must be one kind, each with the exact bounds of its identifier, and contiguous. Build rejects a gapped or misordered timeline and actuals that do not form a prefix.

        Returns
        -------
        ModelBuilder
            This builder with its timeline configured.

        Raises
        ------
        ValueError
            If the plan has no periods, the timeline was already set, or this builder was consumed.
        """
        ...

    def from_dataframe(
        self,
        df: pd.DataFrame | pl.DataFrame | Any,
        node_id_column: str | None = None,
    ) -> ModelBuilder:
        """Import scalar value nodes from a wide pandas, Polars, or Arrow table.

        The model timeline must already be set; an import never infers it.
        Columns map to periods by label in any order. The import is atomic.

        Parameters
        ----------
        df : pd.DataFrame | pl.DataFrame | Any
            One row per new value node and one column per period label (for
            example ``"2025Q1"``). Labels must be unique and belong to the model
            timeline, including all-null columns. A numeric cell is a scalar
            observation in the node's own units; a ``None`` / ``null`` / NaN
            cell is a missing observation and is left out; ``0`` is kept as a
            known zero.
        node_id_column : str | None, optional
            Column containing node identifiers; ``None`` takes identifiers from
            the pandas frame index (or ``"node_id"`` / ``"id"`` / first column
            on Polars/Arrow inputs).

        Returns
        -------
        ModelBuilder
            This builder with one value node added per row.

        Raises
        ------
        ValueError
            If the timeline is not set, a period label is invalid, repeated or outside the timeline, a node identifier is invalid, repeated or already defined, a cell is not numeric or is infinite, or the builder was consumed. Rejected input leaves the builder unchanged.
        KeyError
            If node_id_column names an absent frame column.
        """
        ...

    def insert_node(self, node: NodeSpec) -> ModelBuilder:
        """Insert a complete node specification under a new identifier.

        Parameters
        ----------
        node:
            Fully configured node; semantic validity is checked together with the model at build. A node whose identifier is already defined is a duplicate that ``build`` rejects; use ``replace_node`` to change an existing node.

        Returns
        -------
        ModelBuilder
            This builder with the supplied node inserted.

        Raises
        ------
        ValueError
            If this builder has already been consumed.
        """
        ...

    def replace_node(self, node: NodeSpec) -> ModelBuilder:
        """Replace an existing node with a complete new definition.

        Parameters
        ----------
        node:
            Complete replacement, stored under its own identifier in the original node's position.

        Returns
        -------
        ModelBuilder
            This builder with the node replaced.

        Raises
        ------
        ValueError
            If no node with that identifier exists or the builder was consumed. The builder is unchanged.
        """
        ...

    def remove_node(self, node_id: str) -> ModelBuilder:
        """Remove an existing node.

        Parameters
        ----------
        node_id:
            Identifier of the node to remove. Formulas that still reference it fail ``build``.

        Returns
        -------
        ModelBuilder
            This builder without the node.

        Raises
        ------
        ValueError
            If no node with that identifier exists or the builder was consumed.
        """
        ...

    def source_snapshot(self, snapshot_id: str) -> ModelBuilder:
        """Name the source snapshot the model's observations were taken from.

        An evaluation with an observation cutoff needs this identity. Keep an
        original filing and its later revision as two models with different
        snapshot identities, so a dated evaluation can replay its day.

        Parameters
        ----------
        snapshot_id:
            Nonempty identifier of the source snapshot, for example ``"FY2025-10K-original"``.

        Returns
        -------
        ModelBuilder
            This builder, for chaining.

        Raises
        ------
        ValueError
            If the builder was consumed. A blank identifier is rejected by ``build``.
        """
        ...

class MixedNodeBuilder:
    """
    Builder for a mixed statement node.

    A mixed node combines explicit values, a forecast spec, and/or a fallback
    formula.  Obtain an instance via :meth:`ModelBuilder.mixed`.

    Note
    ----
    Setters mutate this builder and return it for chaining. A rejected
    formula leaves its values, forecast, name, and parent model unchanged.
    ``build`` consumes the mixed-node builder and returns its parent model
    builder; further calls on the consumed mixed-node builder raise ``ValueError``.

    Examples
    --------
    >>> from finstack_quant.statements import ModelBuilder
    >>> builder = ModelBuilder("demo")
    >>> _ = builder.periods("2025Q1..Q1")
    >>> _ = builder.value("revenue", [("2025Q1", 100.0)])
    >>> _ = builder.value("cost", [("2025Q1", 60.0)])
    >>> mixed = builder.mixed("profit")
    >>> _ = mixed.values([("2025Q1", 40.0)])
    >>> _ = mixed.formula("revenue - cost")
    >>> mixed.build().build().has_node("profit")
    True

    """

    def values(self, values: list[tuple[str, float]]) -> MixedNodeBuilder:
        """
        Set scalar explicit values.

        Parameters
        ----------
        values:
            ``(period_id, value)`` pairs for periods where an explicit scalar
            overrides the formula or forecast.

        Returns
        -------
        MixedNodeBuilder
            This builder, for chaining.

        Raises
        ------
        ValueError
            If a period label is invalid or the mixed-node builder has been consumed.

        """
        ...

    def values_money(self, values: list[tuple[str, Money]]) -> MixedNodeBuilder:
        """
        Set monetary explicit values.

        Parameters
        ----------
        values:
            ``(period_id, Money)`` pairs for periods where an explicit monetary
            value overrides the formula or forecast.

        Returns
        -------
        MixedNodeBuilder
            This builder, for chaining.

        Raises
        ------
        ValueError
            If a period label is invalid or the mixed-node builder has been consumed.

        """
        ...

    def forecast(self, forecast_spec: ForecastSpec) -> MixedNodeBuilder:
        """
        Set how this mixed node is projected in forecast periods.

        Parameters
        ----------
        forecast_spec:
            A :class:`ForecastSpec` describing the projection method.

        Returns
        -------
        MixedNodeBuilder
            This builder, for chaining.

        Raises
        ------
        ValueError
            If the mixed-node builder has already been consumed.

        """
        ...

    def formula(self, formula: str) -> MixedNodeBuilder:
        """
        Set the fallback formula.

        Parameters
        ----------
        formula:
            DSL expression used when no explicit value or forecast is available.

        Returns
        -------
        MixedNodeBuilder
            This builder, for chaining.

        Raises
        ------
        ValueError
            If formula is empty or invalid, or the mixed-node builder has been consumed.

        """
        ...

    def name(self, name: str) -> MixedNodeBuilder:
        """
        Set the statement node's display name in reports.

        Parameters
        ----------
        name:
            Human-readable node name.

        Returns
        -------
        MixedNodeBuilder
            This builder, for chaining.

        Raises
        ------
        ValueError
            If the mixed-node builder has already been consumed.

        """
        ...

    def rollup(self, rollup: str) -> MixedNodeBuilder:
        """
        Declare how this line rolls up to a coarser reporting frequency.

        Parameters
        ----------
        rollup:
            ``"sum"`` for flows, ``"period_end"`` for balances and trailing
            metrics, ``"formula"`` for ratios. A rollup rejects ``"formula"``
            on a node without a formula, and a :class:`RollupSpec` entry for
            the node overrides this declaration.

        Returns
        -------
        MixedNodeBuilder
            This builder, for chaining.

        Raises
        ------
        ValueError
            If ``rollup`` is not a recognized method or the mixed-node builder
            has already been consumed.

        """
        ...

    def build(self) -> ModelBuilder:
        """
        Attach the mixed node and return a ready model builder.

        Returns
        -------
        ModelBuilder
            The parent :class:`ModelBuilder` with the mixed node attached.

        Raises
        ------
        ValueError
            If required builder fields are missing or fail validation.
        """
        ...

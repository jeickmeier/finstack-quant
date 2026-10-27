"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements/builder.rs``."""

from __future__ import annotations

from datetime import date
from typing import Any
import pandas as pd
from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import BusinessDayConvention, DayCount, Period, Tenor
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
        >>> original = ModelBuilder("demo").periods("2025Q1..Q1").build()
        >>> rebuilt = ModelBuilder.from_spec(original).value("revenue", [("2025Q1", 100.0)]).build()
        >>> rebuilt.node_ids()
        ['revenue']
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

    def where_clause(self, where_clause: str) -> ModelBuilder:
        """
        Attach a conditional expression to the last added node.

        Parameters
        ----------
        where_clause:
            DSL expression evaluated per period to gate the node's value.

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

    def with_meta(self, key: str, value: Any) -> ModelBuilder:
        """
        Add model-level metadata from a JSON payload.

        Parameters
        ----------
        key:
            Namespaced model-metadata key used to identify the supplied JSON
            value in serialized model output. ``currency`` selects the model's
            reporting currency.
        value:
            JSON-serializable metadata value. For ``currency``, supply a supported
            three-letter currency-code string such as ``"USD"``; ``build()``
            raises ``ValueError`` for malformed currency metadata.

        Returns
        -------
        ModelBuilder
            This builder, for chaining.

        Raises
        ------
        ValueError
            If ``value`` is not JSON-serialisable, periods are not configured, or the builder
            has already been consumed.

        """
        ...

    def with_builtin_metrics(self) -> ModelBuilder:
        """
        Add all built-in statement metrics to the model.

        Returns
        -------
        ModelBuilder
            This builder, for chaining.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def add_metric_from_registry(self, qualified_id: str, registry: Registry) -> ModelBuilder:
        """
        Add one metric and its dependencies from a metric registry.

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
            If periods have not been configured or the builder has already been consumed.
        KeyError
            If qualified_id or one of its dependencies cannot be resolved in registry.

        """
        ...

    def add_bond(
        self,
        id: str,
        notional: Money,
        coupon_rate: float,
        issue_date: date | str,
        maturity_date: date | str,
        discount_curve_id: str,
    ) -> ModelBuilder:
        """

        Add a fixed-rate bond to the capital structure (US 30/360 semi-annual).

        Rejected inputs leave this builder usable with its accumulated model intact.

        For non-USD conventions, use :meth:`add_debt` with a pre-built
        ``Bond`` JSON specification.

        Parameters
        ----------
        id:
            Bond identifier.
        notional:
            Face value as a :class:`Money` amount.
        coupon_rate:
            Annual coupon rate as a decimal (e.g. ``0.05`` for 5%).
        issue_date:
            Contractual issue and accrual start as datetime.date or ISO YYYY-MM-DD;
            must precede maturity.
        maturity_date:
            Final contractual repayment as datetime.date or ISO YYYY-MM-DD;
            must follow the issue date.
        discount_curve_id:
            Curve ID for discounting (e.g. ``"USD-OIS"``).

        Returns
        -------
        ModelBuilder
            This builder, for chaining.

        Raises
        ------
        ValueError
            If the bond constructor rejects its rate, notional, curve configuration,
            or dates, or the builder has already been consumed.

        """
        ...

    def add_swap(
        self,
        id: str,
        notional: Money,
        fixed_rate: float,
        start_date: date | str,
        maturity_date: date | str,
        discount_curve_id: str,
        forward_curve_id: str,
    ) -> ModelBuilder:
        """

        Add an interest rate swap to the capital structure (US conventions).

        Rejected inputs leave this builder usable with its accumulated model intact.

        Parameters
        ----------
        id:
            Swap identifier.
        notional:
            Notional amount as a :class:`Money` value.
        fixed_rate:
            Fixed leg rate as a decimal (e.g. ``0.04`` for 4%).
        start_date:
            Contractual accrual start for both legs as datetime.date or ISO
            YYYY-MM-DD; must precede maturity.
        maturity_date:
            Final contractual accrual end as datetime.date or ISO YYYY-MM-DD;
            must follow the start date.
        discount_curve_id:
            Curve ID for discounting.
        forward_curve_id:
            Curve ID for forward rates.

        Returns
        -------
        ModelBuilder
            This builder, for chaining.

        Raises
        ------
        ValueError
            If the swap constructor rejects its rate, notional, curve configuration,
            or dates, or the builder has already been consumed.

        """
        ...

    def add_bond_with_convention(
        self,
        id: str,
        notional: Money,
        coupon_rate: float,
        issue_date: date | str,
        maturity_date: date | str,
        convention: str,
        discount_curve_id: str,
    ) -> ModelBuilder:
        """

        Add a fixed-rate bond with a market convention preset.

        Rejected inputs leave this builder usable with its accumulated model intact.

        Applies regional day-count, coupon-frequency, and calendar
        conventions automatically; :meth:`add_bond` uses US corporate
        conventions instead.

        Parameters
        ----------
        id:
            Bond identifier.
        notional:
            Face value as a :class:`Money` amount.
        coupon_rate:
            Annual coupon rate as a decimal (e.g. ``0.03`` for 3%).
        issue_date:
            Contractual issue and accrual start as datetime.date or ISO YYYY-MM-DD;
            must precede maturity.
        maturity_date:
            Final contractual repayment as datetime.date or ISO YYYY-MM-DD;
            must follow the issue date.
        convention:
            Regional preset as the canonical snake_case identifier:
            ``"us_treasury"``, ``"us_agency"``, ``"german_bund"``,
            ``"uk_gilt"``, ``"french_oat"``, ``"jgb"``, ``"us_corporate"``,
            or ``"eur_corporate"``.
        discount_curve_id:
            Curve ID for discounting (e.g. ``"EUR-OIS"``).

        Returns
        -------
        ModelBuilder
            This builder, for chaining.

        Raises
        ------
        ValueError
            If the convention is unknown, the bond constructor rejects its rate,
            notional, curve configuration, or dates, or the builder was consumed.

        Examples
        --------
        >>> from datetime import date
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.statements import ModelBuilder
        >>> builder = ModelBuilder("cs-model")
        >>> _ = builder.add_bond_with_convention(
        ...     "BUND-001",
        ...     Money(10_000_000.0, Currency("EUR")),
        ...     0.03,
        ...     date(2025, 1, 15),
        ...     date(2030, 1, 15),
        ...     "german_bund",
        ...     "EUR-OIS",
        ... )
        """
        ...

    def add_swap_with_conventions(
        self,
        id: str,
        notional: Money,
        fixed_rate: float,
        start_date: date | str,
        maturity_date: date | str,
        discount_curve_id: str,
        forward_curve_id: str,
        fixed_frequency: Tenor | str,
        fixed_day_count: DayCount,
        float_frequency: Tenor | str,
        float_day_count: DayCount,
        business_day_convention: BusinessDayConvention | str | None = None,
    ) -> ModelBuilder:
        """

        Add an interest rate swap with custom leg conventions.

        Rejected inputs leave this builder usable with its accumulated model intact.

        Exposes day-count, frequency, and business-day-convention parameters
        for non-USD swaps (e.g. EUR annual ACT/360 fixed legs);
        :meth:`add_swap` uses US conventions instead.

        Parameters
        ----------
        id:
            Swap identifier.
        notional:
            Notional amount as a :class:`Money` value.
        fixed_rate:
            Fixed leg rate as a decimal (e.g. ``0.04`` for 4%).
        start_date:
            Contractual accrual start for both legs as datetime.date or ISO
            YYYY-MM-DD; must precede maturity.
        maturity_date:
            Final contractual accrual end as datetime.date or ISO YYYY-MM-DD;
            must follow the start date.
        discount_curve_id:
            Curve ID for discounting.
        forward_curve_id:
            Curve ID for floating-leg forward rates.
        fixed_frequency:
            Payment frequency of the fixed leg (e.g. ``"1Y"``).
        fixed_day_count:
            Day-count convention applied to the fixed leg.
        float_frequency:
            Payment / fixing frequency of the floating leg (e.g. ``"3M"``).
        float_day_count:
            Day-count convention applied to the floating leg.
        business_day_convention:
            Schedule-date rolling convention (default Modified Following).

        Returns
        -------
        ModelBuilder
            This builder, for chaining.

        Raises
        ------
        ValueError
            If a tenor, day count, convention, rate, notional, curve configuration,
            or date is rejected, or the builder has already been consumed.

        Examples
        --------
        >>> from datetime import date
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.dates import DayCount
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.statements import ModelBuilder
        >>> builder = ModelBuilder("cs-model")
        >>> _ = builder.add_swap_with_conventions(
        ...     "SWAP-EUR",
        ...     Money(5_000_000.0, Currency("EUR")),
        ...     0.03,
        ...     date(2025, 1, 15),
        ...     date(2030, 1, 15),
        ...     "EUR-OIS",
        ...     "EUR-EURIBOR-6M",
        ...     "1Y",
        ...     DayCount.ACT_360,
        ...     "6M",
        ...     DayCount.ACT_360,
        ...     "modified_following",
        ... )
        """
        ...

    def add_debt(self, id: str, instrument: Any | str) -> ModelBuilder:
        """
        Add a debt instrument via its canonical v1 instrument envelope.

        Supported instrument types are bonds, convertible bonds, revolving
        credit facilities, term loans, interest-rate swaps, caps/floors, and
        swaptions.

                Parameters
                ----------
                id:
                    Instrument identifier.
                instrument:
                    ``finstack_quant.instrument/1`` envelope containing the debt instrument.

                Returns
        -------
        ModelBuilder
            This builder, for chaining.

        Raises
                ------
                ValueError
                    If the envelope is invalid or its instrument type is not supported by
                    financial statement capital structures.

        """
        ...

    def reporting_currency(self, currency: Currency) -> ModelBuilder:
        """
        Set the reporting currency used for capital-structure totals.

        Parameters
        ----------
        currency:
            A :class:`Currency` instance. A bare ISO-4217 string is not
            accepted; construct ``Currency("USD")`` first.

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

    def fx_policy(self, policy: str) -> ModelBuilder:
        """
        Set the FX policy (``cashflow_date``/``period_end``/``period_average``).

        Parameters
        ----------
        policy:
            FX conversion policy label.

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

        Validates node identities, explicit-value periods, forecasts, formula
        dimensions and timeline intervals. ``period_id`` is reserved for the
        timeline column in result exports.

        Returns
        -------
        FinancialModelSpec
            Completed specification.

        Raises
        ------
        ValueError
            If the builder was already consumed or the model (including its timeline) violates its semantic invariants.

        """
        ...

    def periods_explicit(self, periods: list[Period]) -> ModelBuilder:
        """Set the model timeline from explicit calendar periods.

        Parameters
        ----------
        periods:
            Non-empty ordered Period objects with positive, nonoverlapping intervals; gaps are allowed. Build rejects a non-increasing timeline or actuals that do not form a prefix.

        Returns
        -------
        ModelBuilder
            This builder with its timeline configured.

        Raises
        ------
        ValueError
            If periods is empty, the timeline was already set, or this builder was consumed.
        """
        ...

    def from_dataframe(
        self, df: pd.DataFrame, actuals_until: str | None = None, node_id_column: str | None = None
    ) -> ModelBuilder:
        """Load scalar value nodes from a wide pandas DataFrame.

        Parameters
        ----------
        df:
            Node rows with period-identifier columns; every label must belong to the model timeline, including all-null columns. Numeric cells use the node's own units and NaN/None cells are skipped.
        actuals_until:
            Inclusive actuals cutoff used when deriving an unset timeline from the first and last period columns; ignored if periods are already set.
        node_id_column:
            Column containing node identifiers; None takes identifiers from the frame index.

        Returns
        -------
        ModelBuilder
            This builder populated with one value node per row.

        Raises
        ------
        ValueError
            If any period label is invalid or outside the model timeline, a node identifier is reserved, numeric cells are invalid, or the builder was consumed. Rejected input leaves the builder unchanged.
        KeyError
            If node_id_column names an absent frame column.
        """
        ...

    def insert_node(self, node: NodeSpec) -> ModelBuilder:
        """Insert a complete node specification, replacing a node with the same identifier.

        Parameters
        ----------
        node:
            Fully configured node; semantic validity is checked together with the model at build.

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

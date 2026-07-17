"""Financial statement modeling: builders, evaluators, forecasts, DSL, adjustments.

Python bindings for the ``finstack-quant-statements`` Rust crate: model specifications,
``ModelBuilder``, ``Evaluator``, formula parsing/validation, and EBITDA-style
normalization helpers.
"""

from __future__ import annotations

from datetime import date

import pandas as pd

from finstack_quant.core.currency import Currency
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money

__all__ = [
    "ForecastMethod",
    "ForecastSpec",
    "NodeType",
    "NodeId",
    "NumericMode",
    "FinancialModelSpec",
    "ModelBuilder",
    "MixedNodeBuilder",
    "MetricRegistry",
    "StatementResult",
    "Evaluator",
    "parse_formula",
    "validate_formula",
    "NormalizationConfig",
    "normalize",
    "CheckSuiteSpec",
    "CheckReport",
    "EcfSweepSpec",
    "PikToggleSpec",
    "WaterfallSpec",
]

class ForecastMethod:
    """Available forecast methods for projecting node values.

    Construct variants via static factory methods (e.g. ``growth_pct()``).

    Example
    -------
    >>> from finstack_quant.statements import ForecastMethod
    >>> ForecastMethod.forward_fill()
    ForecastMethod(...)
    """

    @staticmethod
    def forward_fill() -> ForecastMethod:
        """Carry the last observed value forward into future periods.

        Returns
        -------
        ForecastMethod
            Forward-fill forecast method.
        """
        ...

    @staticmethod
    def growth_pct() -> ForecastMethod:
        """Apply compound percentage growth between periods.

        Returns
        -------
        ForecastMethod
            Growth-percentage forecast method.
        """
        ...

    @staticmethod
    def curve_pct() -> ForecastMethod:
        """Apply period-specific percentage growth from a curve.

        Returns
        -------
        ForecastMethod
            Curve-percentage forecast method.
        """
        ...

    @staticmethod
    def normal() -> ForecastMethod:
        """Normal-distribution sampling (deterministic under a fixed seed).

        Returns
        -------
        ForecastMethod
            Normal distribution forecast method.
        """
        ...

    @staticmethod
    def log_normal() -> ForecastMethod:
        """Log-normal distribution sampling (deterministic under a fixed seed).

        Returns
        -------
        ForecastMethod
            Log-normal forecast method.
        """
        ...

    @staticmethod
    def override_method() -> ForecastMethod:
        """Use explicit period overrides instead of a statistical rule.

        Returns
        -------
        ForecastMethod
            Override forecast method.
        """
        ...

    @staticmethod
    def time_series() -> ForecastMethod:
        """Reference an external time series as the forecast source.

        Returns
        -------
        ForecastMethod
            External time-series forecast method.
        """
        ...

    @staticmethod
    def seasonal() -> ForecastMethod:
        """Apply a seasonal pattern (additive or multiplicative).

        Returns
        -------
        ForecastMethod
            Seasonal forecast method.
        """
        ...

    def __eq__(self, other: object) -> bool:
        """Return whether two forecast method tokens are equal."""
        ...

    def __repr__(self) -> str:
        """Return a debug representation of this forecast method.
        Returns
        -------
        str
        """
        ...

class ForecastSpec:
    """Forecast configuration for a statement node.

    Example
    -------
    >>> from finstack_quant.statements import ForecastSpec
    >>> spec = ForecastSpec.forward_fill()  # doctest: +SKIP
    """

    def __init__(self, method: ForecastMethod, params_json: str | None = None) -> None:
        """Create a forecast spec from a method and optional JSON params.

        Parameters
        ----------
        method:
            A :class:`ForecastMethod` describing the projection approach.
        params_json:
            Optional JSON string with method-specific parameters.

        Example
        -------
        >>> from finstack_quant.statements import ForecastMethod
        >>> spec = ForecastSpec(ForecastMethod.forward_fill())  # doctest: +SKIP
        """
        ...

    @staticmethod
    def forward_fill() -> ForecastSpec:
        """Carry the last observed value forward.

        Returns
        -------
        ForecastSpec
            A forward-fill forecast specification.

        Example
        -------
        >>> spec = ForecastSpec.forward_fill()  # doctest: +SKIP
        """
        ...

    @staticmethod
    def growth(rate: float) -> ForecastSpec:
        """Compound each future period by ``rate``.

        Parameters
        ----------
        rate:
            Period-over-period growth rate as a decimal (e.g. ``0.05`` for 5%).

        Returns
        -------
        ForecastSpec
            A constant-growth forecast specification.

        Example
        -------
        >>> spec = ForecastSpec.growth(0.05)  # doctest: +SKIP
        """
        ...

    @staticmethod
    def curve(curve: list[float]) -> ForecastSpec:
        """Use period-specific growth rates.

        Parameters
        ----------
        curve:
            Per-period growth rates as decimals, aligned to future periods.

        Returns
        -------
        ForecastSpec
            A curve-based forecast specification.

        Example
        -------
        >>> spec = ForecastSpec.curve([0.03, 0.05, 0.07])  # doctest: +SKIP
        """
        ...

    @staticmethod
    def normal(mean: float, std_dev: float, seed: int) -> ForecastSpec:
        """Use deterministic additive normal draws.

        Parameters
        ----------
        mean:
            Mean of the normal distribution.
        std_dev:
            Standard deviation of the normal distribution.
        seed:
            Random seed for deterministic reproducibility.

        Returns
        -------
        ForecastSpec
            A normal-draw forecast specification.

        Example
        -------
        >>> spec = ForecastSpec.normal(0.0, 0.1, 42)  # doctest: +SKIP
        """
        ...

    @staticmethod
    def lognormal(mean: float, std_dev: float, seed: int) -> ForecastSpec:
        """Use deterministic multiplicative log-normal draws.

        Parameters
        ----------
        mean:
            Mean of the underlying normal distribution.
        std_dev:
            Standard deviation of the underlying normal distribution.
        seed:
            Random seed for deterministic reproducibility.

        Returns
        -------
        ForecastSpec
            A log-normal-draw forecast specification.

        Example
        -------
        >>> spec = ForecastSpec.lognormal(0.0, 0.1, 42)  # doctest: +SKIP
        """
        ...

    @staticmethod
    def from_json(json: str) -> ForecastSpec:
        """Deserialize a forecast spec from JSON.

        Parameters
        ----------
        json:
            JSON document matching the forecast spec schema.

        Returns
        -------
        ForecastSpec
            Parsed forecast specification.

        Raises
        ------
        ValueError
            If JSON parsing or schema validation fails.

        Example
        -------
        >>> spec = ForecastSpec.from_json('{"method":"forward_fill"}')  # doctest: +SKIP
        """
        ...

    def to_json(self) -> str:
        """Serialize this forecast spec to JSON.

        Returns
        -------
        str
            JSON text.

        Example
        -------
        >>> spec = ForecastSpec.forward_fill()  # doctest: +SKIP
        >>> spec.to_json()  # doctest: +SKIP
        '{...}'
        """
        ...

    def __repr__(self) -> str:
        """Return a debug representation of this forecast spec.
        Returns
        -------
        str
        """
        ...

class NodeType:
    """How a node combines explicit values, forecasts, and formulas.

    Example
    -------
    >>> from finstack_quant.statements import NodeType
    >>> NodeType.calculated()
    NodeType(...)
    """

    @staticmethod
    def value() -> NodeType:
        """Node holds only explicit values (actuals or assumptions).

        Returns
        -------
        NodeType
            Value-only node type.
        """
        ...

    @staticmethod
    def calculated() -> NodeType:
        """Node is derived entirely from a formula.

        Returns
        -------
        NodeType
            Calculated node type.
        """
        ...

    @staticmethod
    def mixed() -> NodeType:
        """Node may combine values, forecasts, and formulas with precedence rules.

        Returns
        -------
        NodeType
            Mixed node type.
        """
        ...

    def __eq__(self, other: object) -> bool:
        """Return whether two node type tokens are equal."""
        ...

    def __repr__(self) -> str:
        """Return a debug representation of this node type.
        Returns
        -------
        str
        """
        ...

class NodeId:
    """Type-safe identifier for a node in a financial model.

    Example
    -------
    >>> from finstack_quant.statements import NodeId
    >>> str(NodeId("revenue"))
    'revenue'
    """

    def __init__(self, id: str) -> None:
        """Create a node identifier from a string.

        Parameters
        ----------
        id:
            Raw node identifier (for example ``\"revenue\"``).

        Example
        -------
        >>> NodeId("ebitda").as_str()
        'ebitda'
        """
        ...

    def as_str(self) -> str:
        """Return the underlying identifier string.

        Returns
        -------
        str
            Node id string.

        Example
        -------
        >>> NodeId("cogs").as_str()
        'cogs'
        """
        ...

    def __repr__(self) -> str:
        """Return a Python-literal style representation.
        Returns
        -------
        str
        """
        ...

    def __str__(self) -> str:
        """Return the identifier as a plain string.
        Returns
        -------
        str
        """
        ...

class NumericMode:
    """Numeric evaluation mode for statement evaluation.

    Example
    -------
    >>> from finstack_quant.statements import NumericMode
    >>> NumericMode.float64()
    NumericMode(...)
    >>> NumericMode.decimal()
    NumericMode(...)
    """

    @staticmethod
    def float64() -> NumericMode:
        """Use 64-bit floating point arithmetic.

        Returns
        -------
        NumericMode
            IEEE-754 double-precision mode.
        """
        ...

    @staticmethod
    def decimal() -> NumericMode:
        """Reserved decimal-arithmetic mode.

        This variant exists so saved result metadata can evolve, but statement
        evaluation always runs in ``float64``; selecting it does not change the
        arithmetic today.

        Returns
        -------
        NumericMode
            Decimal arithmetic mode (reserved).
        """
        ...

    def __eq__(self, other: object) -> bool:
        """Return whether two numeric mode tokens are equal."""
        ...

    def __repr__(self) -> str:
        """Return a debug representation of this numeric mode.
        Returns
        -------
        str
        """
        ...

class FinancialModelSpec:
    """Top-level financial model specification (wire format).

    Typically built with ``ModelBuilder`` or loaded from JSON.

    Example
    -------
    >>> from finstack_quant.statements import FinancialModelSpec
    >>> doc = (
    ...     '{"id":"x","periods":[{"id":"2025Q1","start":"2025-01-01",'
    ...     '"end":"2025-04-01","is_actual":false}],"nodes":{}}'
    ... )
    >>> spec = FinancialModelSpec.from_json(doc)
    >>> spec.id
    'x'

    A model must have at least one period; ``"periods":[]`` raises
    ``ValueError``.
    """

    @staticmethod
    def from_json(json: str) -> FinancialModelSpec:
        """Deserialize a model specification from JSON text.

        Parameters
        ----------
        json:
            JSON document matching the statements model schema.

        Returns
        -------
        FinancialModelSpec
            Parsed specification.

        Raises
        ------
        ValueError
            If ``json`` is not valid JSON or fails schema validation.

        Example
        -------
        >>> FinancialModelSpec.from_json(
        ...     '{"id":"m","periods":[{"id":"2025Q1","start":"2025-01-01","end":"2025-04-01","is_actual":false}],"nodes":{}}'
        ... ).node_count
        0
        """
        ...

    def to_json(self) -> str:
        """Serialize this specification to compact JSON.

        Returns
        -------
        str
            JSON text.

        Raises
        ------
        ValueError
            If serialization fails.

        Example
        -------
        >>> m = FinancialModelSpec.from_json(
        ...     '{"id":"m","periods":[{"id":"2025Q1","start":"2025-01-01","end":"2025-04-01","is_actual":false}],"nodes":{}}'
        ... )
        >>> '"id"' in m.to_json()
        True
        """
        ...

    @property
    def id(self) -> str:
        """Model identifier string.
        Returns
        -------
        str
        """
        ...

    @property
    def period_count(self) -> int:
        """Number of periods defined on the model.
        Returns
        -------
        int
        """
        ...

    @property
    def node_count(self) -> int:
        """Number of nodes defined on the model.
        Returns
        -------
        int
        """
        ...

    def node_ids(self) -> list[str]:
        """List node identifiers in declaration order.

        Returns
        -------
        list[str]
            Ordered node id strings.

        Example
        -------
        >>> FinancialModelSpec.from_json(
        ...     '{"id":"m","periods":[{"id":"2025Q1","start":"2025-01-01","end":"2025-04-01","is_actual":false}],"nodes":{}}'
        ... ).node_ids()
        []
        """
        ...

    def has_node(self, node_id: str) -> bool:
        """Return whether a node with the given id exists.

        Parameters
        ----------
        node_id:
            Node identifier to test.

        Returns
        -------
        bool
            ``True`` if present.

        Example
        -------
        >>> FinancialModelSpec.from_json(
        ...     '{"id":"m","periods":[{"id":"2025Q1","start":"2025-01-01","end":"2025-04-01","is_actual":false}],"nodes":{}}'
        ... ).has_node("x")
        False
        """
        ...

    @property
    def schema_version(self) -> int:
        """Wire-format schema version of this specification.
        Returns
        -------
        int
        """
        ...

    def __repr__(self) -> str:
        """Return a concise summary including id, period count, and node count.
        Returns
        -------
        str
        """
        ...

class ModelBuilder:
    """Builder for a ``FinancialModelSpec``.

    Call ``periods`` once, then add nodes with ``value`` / ``compute``, and
    finish with ``build``.

    Note
    ----
    Methods on this class mutate the builder in place and return ``None``.
    Call them sequentially rather than chaining.

    Example
    -------
    >>> from finstack_quant.statements import ModelBuilder
    >>> b = ModelBuilder("co")
    >>> b.periods("2025Q1..Q2", None)  # doctest: +SKIP
    >>> b.value("revenue", [("2025Q1", 100.0)])  # doctest: +SKIP
    >>> b.compute("cogs", "revenue * 0.6")  # doctest: +SKIP
    >>> spec = b.build()  # doctest: +SKIP
    """

    def __init__(self, id: str) -> None:
        """Start a new builder for a model with the given id.

        Parameters
        ----------
        id:
            Model identifier assigned to the built ``FinancialModelSpec``.

        Example
        -------
        >>> ModelBuilder("Acme")  # doctest: +ELLIPSIS
        <finstack_quant.statements.ModelBuilder ...>
        """
        ...

    def periods(self, range: str, actuals_until: str | None = None) -> None:
        """Define the model's period lattice from a range expression.

        Parameters
        ----------
        range:
            Period range expression such as ``\"2025Q1..Q4\"``.
        actuals_until:
            Optional last actual period label; ``None`` if not used.

        Raises
        ------
        ValueError
            If periods are already set, the range is invalid, or the builder was consumed.

        Example
        -------
        >>> b = ModelBuilder("x")
        >>> b.periods("2025Q1..Q2", None)  # doctest: +SKIP
        """
        ...

    def value(self, node_id: str, values: list[tuple[str, float]]) -> None:
        """Add a value node with explicit per-period scalars.

        Parameters
        ----------
        node_id:
            Identifier for the new node.
        values:
            ``(period_id, value)`` pairs, for example ``[(\"2025Q1\", 1.0)]``.

        Raises
        ------
        ValueError
            If periods were not configured, a period id is invalid, or the builder was consumed.

        Example
        -------
        >>> b = ModelBuilder("x")
        >>> b.periods("2025Q1..Q1", None)  # doctest: +SKIP
        >>> b.value("rev", [("2025Q1", 10.0)])  # doctest: +SKIP
        """
        ...

    def value_scalar(self, node_id: str, values: list[tuple[str, float]]) -> None:
        """Add a scalar value node with explicit per-period values.

        Parameters
        ----------
        node_id:
            Identifier for the new node.
        values:
            ``(period_id, value)`` pairs, for example ``[("2025Q1", 1.0)]``.

        Raises
        ------
        ValueError
            If periods were not configured, a period id is invalid, or the builder was consumed.

        Example
        -------
        >>> b = ModelBuilder("x")
        >>> b.periods("2025Q1..Q1", None)  # doctest: +SKIP
        >>> b.value_scalar("margin_pct", [("2025Q1", 0.15)])  # doctest: +SKIP
        """
        ...

    def value_money(self, node_id: str, values: list[tuple[str, Money]]) -> None:
        """Add a monetary value node with explicit per-period values.

        Parameters
        ----------
        node_id:
            Identifier for the new node.
        values:
            ``(period_id, Money)`` pairs, for example ``[("2025Q1", Money(100.0, "USD"))]``.

        Raises
        ------
        ValueError
            If periods were not configured, a period id is invalid, or the builder was consumed.

        Example
        -------
        >>> from finstack_quant.core.money import Money
        >>> b = ModelBuilder("x")
        >>> b.periods("2025Q1..Q1", None)  # doctest: +SKIP
        >>> b.value_money("revenue", [("2025Q1", Money(100.0, "USD"))])  # doctest: +SKIP
        """
        ...

    def compute(self, node_id: str, formula: str) -> None:
        """Add a calculated node from a DSL formula.

        Parameters
        ----------
        node_id:
            Identifier for the computed node.
        formula:
            Expression in the statements DSL (for example ``"revenue - cogs"``).

        Raises
        ------
        ValueError
            If the formula fails to compile or the builder state is invalid.

        Example
        -------
        >>> b = ModelBuilder("x")
        >>> b.periods("2025Q1..Q1", None)  # doctest: +SKIP
        >>> b.compute("margin", "revenue - cogs")  # doctest: +SKIP
        """
        ...

    def mixed(self, node_id: str) -> MixedNodeBuilder:
        """Start configuring a mixed node and consume this builder until ``build`` returns.

        Parameters
        ----------
        node_id:
            Identifier for the new mixed node.

        Returns
        -------
        MixedNodeBuilder
            A builder for the mixed node.  Call ``build`` on the returned
            builder to attach the node and resume this builder.

        Example
        -------
        >>> b = ModelBuilder("x")
        >>> b.periods("2025Q1..Q1", None)  # doctest: +SKIP
        >>> mb = b.mixed("hybrid")  # doctest: +SKIP
        >>> mb.values([("2025Q1", 10.0)])  # doctest: +SKIP
        >>> mb.formula("revenue * 0.1")  # doctest: +SKIP
        >>> b = mb.build()  # doctest: +SKIP
        """
        ...

    def forecast(self, node_id: str, forecast_spec: ForecastSpec) -> None:
        """Attach a forecast to an existing node or create a forecast-only mixed node.

        Parameters
        ----------
        node_id:
            Identifier for the node to forecast.
        forecast_spec:
            A :class:`ForecastSpec` describing the projection method and parameters.

        Example
        -------
        >>> b = ModelBuilder("x")
        >>> b.periods("2025Q1..Q4", None)  # doctest: +SKIP
        >>> b.forecast("revenue", ForecastSpec.from_json("..."))  # doctest: +SKIP
        """
        ...

    def where_clause(self, where_clause: str) -> None:
        """Attach a conditional expression to the last added node.

        Parameters
        ----------
        where_clause:
            DSL expression evaluated per period to gate the node's value.

        Example
        -------
        >>> b = ModelBuilder("x")
        >>> b.periods("2025Q1..Q1", None)  # doctest: +SKIP
        >>> b.value("rev", [("2025Q1", 10.0)])  # doctest: +SKIP
        >>> b.where_clause('period == "2025Q1"')  # doctest: +SKIP
        """
        ...

    def with_meta(self, key: str, value_json: str) -> None:
        """Add model-level metadata from a JSON payload.

        Parameters
        ----------
        key:
            Metadata key.
        value_json:
            JSON-serialized metadata value.

        Example
        -------
        >>> b = ModelBuilder("x")
        >>> b.with_meta("sector", '""healthcare""')  # doctest: +SKIP
        """
        ...

    def with_name_normalization(self) -> None:
        """Enable standard accounting term alias normalization.

        Example
        -------
        >>> b = ModelBuilder("x")
        >>> b.with_name_normalization()  # doctest: +SKIP
        """
        ...

    def with_builtin_metrics(self) -> None:
        """Add all built-in statement metrics to the model.

        Example
        -------
        >>> b = ModelBuilder("x")
        >>> b.with_builtin_metrics()  # doctest: +SKIP
        """
        ...

    def add_metric_from_registry(self, qualified_id: str, registry: MetricRegistry) -> None:
        """Add one metric and its dependencies from a metric registry.

        Parameters
        ----------
        qualified_id:
            Fully qualified metric identifier.
        registry:
            A :class:`MetricRegistry` containing the metric definition.

        Example
        -------
        >>> reg = MetricRegistry.with_builtins()  # doctest: +SKIP
        >>> b = ModelBuilder("x")
        >>> b.add_metric_from_registry("ebitda", reg)  # doctest: +SKIP
        """
        ...

    def add_bond(
        self,
        id: str,
        notional: Money,
        coupon_rate: float,
        issue_date: date,
        maturity_date: date,
        discount_curve_id: str,
    ) -> None:
        """Add a fixed-rate bond to the capital structure (US 30/360 semi-annual).

        For non-USD conventions, use :meth:`add_custom_debt` with a pre-built
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
            Bond issue date.
        maturity_date:
            Bond maturity date.
        discount_curve_id:
            Curve ID for discounting (e.g. ``"USD-OIS"``).

        Example
        -------
        >>> from finstack_quant.core.money import Money
        >>> import datetime
        >>> b = ModelBuilder("x")
        >>> b.add_bond(
        ...     "bond_a", Money(1_000_000, "USD"), 0.05, datetime.date(2025, 1, 1), datetime.date(2030, 1, 1), "USD-OIS"
        ... )  # doctest: +SKIP
        """
        ...

    def add_swap(
        self,
        id: str,
        notional: Money,
        fixed_rate: float,
        start_date: date,
        maturity_date: date,
        discount_curve_id: str,
        forward_curve_id: str,
    ) -> None:
        """Add an interest rate swap to the capital structure (US conventions).

        Parameters
        ----------
        id:
            Swap identifier.
        notional:
            Notional amount as a :class:`Money` value.
        fixed_rate:
            Fixed leg rate as a decimal (e.g. ``0.04`` for 4%).
        start_date:
            Swap start date.
        maturity_date:
            Swap maturity date.
        discount_curve_id:
            Curve ID for discounting.
        forward_curve_id:
            Curve ID for forward rates.

        Example
        -------
        >>> from finstack_quant.core.money import Money
        >>> import datetime
        >>> b = ModelBuilder("x")
        >>> b.add_swap(
        ...     "swap_a",
        ...     Money(10_000_000, "USD"),
        ...     0.04,
        ...     datetime.date(2025, 1, 1),
        ...     datetime.date(2030, 1, 1),
        ...     "USD-OIS",
        ...     "USD-SOFR-3M",
        ... )  # doctest: +SKIP
        """
        ...

    def add_custom_debt(self, id: str, spec_json: str) -> None:
        """Add an arbitrary debt instrument via its serde JSON representation.

        Parameters
        ----------
        id:
            Instrument identifier.
        spec_json:
            JSON-serialized instrument specification (e.g. a bond or term loan).

        Example
        -------
        >>> b = ModelBuilder("x")
        >>> b.add_custom_debt("loan_a", '{"type":"term_loan",...}')  # doctest: +SKIP
        """
        ...

    def reporting_currency(self, currency: Currency) -> None:
        """Set the reporting currency used for capital-structure totals.

        Parameters
        ----------
        currency:
            A :class:`Currency` instance. A bare ISO-4217 string is not
            accepted; construct ``Currency("USD")`` first.

        Example
        -------
        >>> b = ModelBuilder("x")
        >>> b.reporting_currency(Currency.USD)  # doctest: +SKIP
        """
        ...

    def fx_policy(self, policy: str) -> None:
        """Set the FX policy (``cashflow_date``/``period_end``/``period_average``/``custom``).

        Parameters
        ----------
        policy:
            FX conversion policy label.

        Example
        -------
        >>> b = ModelBuilder("x")
        >>> b.fx_policy("period_end")  # doctest: +SKIP
        """
        ...

    def waterfall(self, waterfall_spec: WaterfallSpec) -> None:
        """Attach a waterfall specification (PIK toggle + ECF sweep + priorities).

        Parameters
        ----------
        waterfall_spec:
            A :class:`WaterfallSpec` defining cash distribution priorities.

        Example
        -------
        >>> b = ModelBuilder("x")
        >>> b.waterfall(WaterfallSpec.from_json("..."))  # doctest: +SKIP
        """
        ...

    def build(self) -> FinancialModelSpec:
        """Materialize the ``FinancialModelSpec`` and consume the builder.

        Returns
        -------
        FinancialModelSpec
            Completed specification.

        Raises
        ------
        ValueError
            If the builder is not ready or was already consumed.

        Example
        -------
        >>> b = ModelBuilder("x")
        >>> b.periods("2025Q1..Q1", None)  # doctest: +SKIP
        >>> spec = b.build()  # doctest: +SKIP
        """
        ...

class MixedNodeBuilder:
    """Builder for a mixed statement node.

    A mixed node combines explicit values, a forecast spec, and/or a fallback
    formula.  Obtain an instance via :meth:`ModelBuilder.mixed`.

    Note
    ----
    Methods on this class mutate the builder in place and return ``None``.
    Call them sequentially rather than chaining.

    Example
    -------
    >>> b = ModelBuilder("x")
    >>> b.periods("2025Q1..Q2", None)  # doctest: +SKIP
    >>> mb = b.mixed("hybrid")  # doctest: +SKIP
    >>> mb.values([("2025Q1", 10.0)])  # doctest: +SKIP
    >>> mb.formula("revenue * 0.1")  # doctest: +SKIP
    >>> b = mb.build()  # doctest: +SKIP
    """

    def values(self, values: list[tuple[str, float]]) -> None:
        """Set scalar explicit values.

        Parameters
        ----------
        values:
            ``(period_id, value)`` pairs for periods where an explicit scalar
            overrides the formula or forecast.

        Example
        -------
        >>> mb.values([("2025Q1", 10.0)])  # doctest: +SKIP
        """
        ...

    def values_money(self, values: list[tuple[str, Money]]) -> None:
        """Set monetary explicit values.

        Parameters
        ----------
        values:
            ``(period_id, Money)`` pairs for periods where an explicit monetary
            value overrides the formula or forecast.

        Example
        -------
        >>> from finstack_quant.core.money import Money
        >>> mb.values_money([("2025Q1", Money(100.0, "USD"))])  # doctest: +SKIP
        """
        ...

    def forecast(self, forecast_spec: ForecastSpec) -> None:
        """Set the forecast spec.

        Parameters
        ----------
        forecast_spec:
            A :class:`ForecastSpec` describing the projection method.

        Example
        -------
        >>> mb.forecast(ForecastSpec.from_json("..."))  # doctest: +SKIP
        """
        ...

    def formula(self, formula: str) -> None:
        """Set the fallback formula.

        Parameters
        ----------
        formula:
            DSL expression used when no explicit value or forecast is available.

        Example
        -------
        >>> mb.formula("revenue * 0.1")  # doctest: +SKIP
        """
        ...

    def name(self, name: str) -> None:
        """Set the display name.

        Parameters
        ----------
        name:
            Human-readable node name.

        Example
        -------
        >>> mb.name("Hybrid Revenue")  # doctest: +SKIP
        """
        ...

    def build(self) -> ModelBuilder:
        """Attach the mixed node and return a ready model builder.

        Returns
        -------
        ModelBuilder
            The parent :class:`ModelBuilder` with the mixed node attached.

        Example
        -------
        >>> b = mb.build()  # doctest: +SKIP
        """
        ...

class MetricRegistry:
    """Reusable statement metric registry.

    Example
    -------
    >>> from finstack_quant.statements import MetricRegistry
    >>> reg = MetricRegistry.with_builtins()  # doctest: +SKIP
    >>> reg.has("ebitda")  # doctest: +SKIP
    True
    """

    def __init__(self) -> None:
        """Create an empty registry.

        Example
        -------
        >>> reg = MetricRegistry()  # doctest: +SKIP
        """
        ...

    @staticmethod
    def with_builtins() -> MetricRegistry:
        """Create a registry preloaded with built-in metrics.

        Returns
        -------
        MetricRegistry
            A registry containing all built-in statement metrics.

        Example
        -------
        >>> reg = MetricRegistry.with_builtins()  # doctest: +SKIP
        """
        ...

    def load_builtins(self) -> None:
        """Load built-in metrics into this registry.

        Example
        -------
        >>> reg = MetricRegistry()
        >>> reg.load_builtins()  # doctest: +SKIP
        """
        ...

    def load_from_json_str(self, json: str) -> None:
        """Load metrics from a JSON document.

        Parameters
        ----------
        json:
            JSON string containing metric definitions.

        Example
        -------
        >>> reg = MetricRegistry()
        >>> reg.load_from_json_str('[{"id":"custom_metric",...}]')  # doctest: +SKIP
        """
        ...

    def load_from_json(self, path: str) -> None:
        """Load metrics from a JSON file path.

        Parameters
        ----------
        path:
            Filesystem path to a JSON document containing metric definitions.

        Example
        -------
        >>> reg = MetricRegistry()
        >>> reg.load_from_json("metrics.json")  # doctest: +SKIP
        """
        ...

    def has(self, qualified_id: str) -> bool:
        """Return whether a fully qualified metric exists.

        Parameters
        ----------
        qualified_id:
            Fully qualified metric identifier.

        Returns
        -------
        bool
            ``True`` if the metric is registered.

        Example
        -------
        >>> reg = MetricRegistry.with_builtins()  # doctest: +SKIP
        >>> reg.has("ebitda")  # doctest: +SKIP
        True
        """
        ...

    def __len__(self) -> int:
        """Return the number of metrics.
        Returns
        -------
        int
        """
        ...

class StatementResult:
    """Per-node, per-period numeric results from evaluating a model.

    Example
    -------
    >>> from finstack_quant.statements import StatementResult, Evaluator, ModelBuilder
    >>> b = ModelBuilder("demo")
    >>> b.periods("2025Q1..Q1", None)  # doctest: +SKIP
    >>> b.value("x", [("2025Q1", 2.0)])  # doctest: +SKIP
    >>> r = Evaluator().evaluate(b.build())  # doctest: +SKIP
    >>> r.get("x", "2025Q1")  # doctest: +SKIP
    2.0
    """

    @staticmethod
    def from_json(json: str) -> StatementResult:
        """Deserialize evaluation results from JSON.

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

        Example
        -------
        >>> # Round-trip: StatementResult.to_json() from an evaluated model
        >>> StatementResult.from_json  # doctest: +ELLIPSIS
        <staticmethod(...)>
        """
        ...

    def to_json(self) -> str:
        """Serialize these results to compact JSON.

        Returns
        -------
        str
            JSON text.

        Raises
        ------
        ValueError
            If serialization fails.

        Example
        -------
        >>> # r = Evaluator().evaluate(spec); r.to_json()  # doctest: +SKIP
        """
        ...

    def get(self, node_id: str, period: str) -> float | None:
        """Return the scalar for ``node_id`` at ``period``, if present.

        Parameters
        ----------
        node_id:
            Node identifier.
        period:
            Period label such as ``\"2025Q1\"``.

        Returns
        -------
        float | None
            Value when found, otherwise ``None``.

        Raises
        ------
        ValueError
            If ``period`` cannot be parsed as a period id.

        Example
        -------
        >>> # r = Evaluator().evaluate(spec); r.get("revenue", "2025Q1")  # doctest: +SKIP
        """
        ...

    def get_money(self, node_id: str, period: str) -> Money | None:
        """Return the currency-tagged ``Money`` value for a monetary node.

        Preserves fixed-point precision and currency. Returns ``None`` when
        the node is not monetary or has no value for this period.

        Parameters
        ----------
        node_id:
            Node identifier.
        period:
            Period label such as ``\"2025Q1\"``.

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
        """Return the scalar value for a non-monetary node.

        Returns ``None`` when the node is monetary or has no value for this
        period.

        Parameters
        ----------
        node_id:
            Node identifier.
        period:
            Period label such as ``\"2025Q1\"``.

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

    def get_node(self, node_id: str) -> dict[str, float] | None:
        """Return all period values for a node as a mapping.

        Parameters
        ----------
        node_id:
            Node identifier.

        Returns
        -------
        dict[str, float] | None
            Mapping from period string to float, or ``None`` if the node is missing.

        Example
        -------
        >>> # m = r.get_node("revenue")  # doctest: +SKIP
        """
        ...

    def node_ids(self) -> list[str]:
        """Return every node id present in this result set.

        Returns
        -------
        list[str]
            Node identifiers.

        Example
        -------
        >>> # ids = r.node_ids()  # doctest: +SKIP
        """
        ...

    @property
    def node_count(self) -> int:
        """Number of nodes in the result.
        Returns
        -------
        int
        """
        ...

    @property
    def num_periods(self) -> int:
        """Number of periods covered by the evaluation metadata.
        Returns
        -------
        int
        """
        ...

    @property
    def eval_time_ms(self) -> int | None:
        """Wall-clock evaluation time in milliseconds, if recorded.
        Returns
        -------
        int or None
        """
        ...

    @property
    def warning_count(self) -> int:
        """Count of evaluation warnings attached to metadata.
        Returns
        -------
        int
        """
        ...

    @property
    def warnings(self) -> list[str]:
        """Evaluation warnings as human-readable strings.

        Returns
        -------
        list[str]
        """
        ...

    @property
    def numeric_mode(self) -> NumericMode:
        """Numeric mode stamped into the result envelope (policy visibility).

        Returns
        -------
        NumericMode
        """
        ...

    @property
    def parallel(self) -> bool:
        """Whether the evaluation ran in parallel (policy visibility).

        Returns
        -------
        bool
        """
        ...

    def to_pandas_long(self) -> pd.DataFrame:
        """Export results as a pandas DataFrame in long (tidy) form.

        Columns: ``node_id``, ``period``, ``value``, ``value_money``,
        ``currency``, ``value_type``. The monetary columns are populated for
        nodes carrying currency information and are otherwise null.
        ``value_money`` is a float64 mirror of the monetary amount (f64, not
        fixed-point Decimal, precision); use ``to_json()`` or ``get_money()``
        when full fixed-point precision is required.

        Returns
        -------
        pd.DataFrame
            Long-format frame with one row per (node, period) pair.
        """
        ...

    def to_pandas_wide(self) -> pd.DataFrame:
        """Export results as a pandas DataFrame in wide form.

        Rows are node identifiers, columns are period identifiers.

        Returns
        -------
        pd.DataFrame
            Wide-format frame with node ids as index.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise summary with node and period counts.
        Returns
        -------
        str
        """
        ...

class Evaluator:
    """Evaluates a ``FinancialModelSpec`` into a ``StatementResult``.

    Example
    -------
    >>> from finstack_quant.statements import Evaluator
    >>> Evaluator()
    <finstack_quant.statements.Evaluator ...>
    """

    def __init__(self) -> None:
        """Create a fresh evaluator with default configuration.

        Example
        -------
        >>> ev = Evaluator()
        >>> ev.evaluate  # doctest: +ELLIPSIS
        <built-in method evaluate ...>

        Returns
        -------
        None
        """
        ...

    def evaluate(self, model: FinancialModelSpec) -> StatementResult:
        """Evaluate ``model`` and return numeric results.

        Parameters
        ----------
        model:
            Specification produced by ``ModelBuilder.build`` or ``from_json``.

        Returns
        -------
        StatementResult
            Populated result object.

        Raises
        ------
        ValueError
            If evaluation fails (for example cyclic dependencies or bad formulas).

        Example
        -------
        >>> ev = Evaluator()
        >>> # ev.evaluate(spec)  # doctest: +SKIP
        >>> True
        True
        """
        ...

    def evaluate_with_market(
        self,
        model: FinancialModelSpec,
        market: MarketContext,
        as_of: date,
    ) -> StatementResult:
        """Evaluate ``model`` with market data and an as-of date.

        Use this for capital-structure-aware models and as-of filtering of
        future actual periods.

        Parameters
        ----------
        model:
            Specification produced by ``ModelBuilder.build`` or ``from_json``.
        market:
            A :class:`MarketContext` with curves, FX, and vol surfaces.
        as_of:
            Valuation date for discounting and period filtering.

        Returns
        -------
        StatementResult
            Populated result object with market-aware valuations.

        Raises
        ------
        ValueError
            If evaluation fails or required market data is missing.

        Example
        -------
        >>> ev = Evaluator()
        >>> # r = ev.evaluate_with_market(spec, mkt, datetime.date(2025, 1, 1))  # doctest: +SKIP
        """
        ...

def parse_formula(formula: str) -> str:
    """Parse a DSL formula and return a debug string for its AST.

    Parameters
    ----------
    formula:
        Source expression in the statements DSL.

    Returns
    -------
    str
        Debug representation of the parsed abstract syntax tree.

    Raises
    ------
    ValueError
        If parsing fails.

    Example
    -------
    >>> parse_formula("revenue - cogs")  # doctest: +ELLIPSIS
    '...'
    """
    ...

def validate_formula(formula: str) -> bool:
    """Return ``True`` if ``formula`` parses and compiles successfully.

    Parameters
    ----------
    formula:
        DSL expression to validate.

    Returns
    -------
    bool
        Always ``True`` when no error is raised.

    Raises
    ------
    ValueError
        If parsing or compilation fails.

    Example
    -------
    >>> validate_formula("a + b")
    True
    """
    ...

class NormalizationConfig:
    """Configuration for normalizing a target metric (for example EBITDA).

    Example
    -------
    >>> from finstack_quant.statements import NormalizationConfig
    >>> NormalizationConfig("ebitda").target_node
    'ebitda'
    """

    def __init__(self, target_node: str) -> None:
        """Create an empty configuration for ``target_node``.

        Parameters
        ----------
        target_node:
            Node id whose values will be adjusted.

        Example
        -------
        >>> cfg = NormalizationConfig("adjusted_ebitda")
        >>> cfg.adjustment_count
        0
        """
        ...

    @staticmethod
    def from_json(json: str) -> NormalizationConfig:
        """Load normalization rules from JSON.

        Parameters
        ----------
        json:
            JSON document for ``NormalizationConfig``.

        Returns
        -------
        NormalizationConfig
            Parsed configuration.

        Raises
        ------
        ValueError
            If JSON is invalid.

        Example
        -------
        >>> NormalizationConfig.from_json('{"target_node":"x","adjustments":[]}').target_node
        'x'
        """
        ...

    def to_json(self) -> str:
        """Serialize this configuration to pretty-printed JSON.

        Returns
        -------
        str
            JSON text.

        Raises
        ------
        ValueError
            If serialization fails.

        Example
        -------
        >>> NormalizationConfig("n").to_json()  # doctest: +ELLIPSIS
        '{...'
        """
        ...

    @property
    def target_node(self) -> str:
        """Node id being normalized.
        Returns
        -------
        str
        """
        ...

    @property
    def adjustment_count(self) -> int:
        """Number of adjustment line items configured.
        Returns
        -------
        int
        """
        ...

    def __repr__(self) -> str:
        """Return a concise summary including target node and adjustment count.
        Returns
        -------
        str
        """
        ...

def normalize(results: StatementResult, config: NormalizationConfig) -> str:
    """Run normalization and return a JSON list of ``NormalizationResult`` objects.

    Parameters
    ----------
    results:
        Evaluated statement output.
    config:
        Target node and adjustment definitions.

    Returns
    -------
    str
        JSON array encoding normalization results.

    Raises
    ------
    ValueError
        If the engine fails.

    Example
    -------
    >>> # payload = normalize(evaluator_output, NormalizationConfig("ebitda"))  # doctest: +SKIP
    >>> NormalizationConfig("ebitda").target_node
    'ebitda'
    """
    ...

class CheckSuiteSpec:
    """A serializable suite specification describing which checks to run.

    Load from JSON (e.g. a team-wide check policy file) and inspect its
    composition (``builtin_check_count`` / ``formula_check_count``). Note:
    running a suite is not yet exposed through the Python bindings; this type is
    currently for loading and inspecting a policy definition only.

    Example
    -------
    >>> from finstack_quant.statements import CheckSuiteSpec
    >>> spec = CheckSuiteSpec.from_json('{"name":"basic","builtin_checks":[],"formula_checks":[]}')
    >>> spec.name
    'basic'
    """

    @staticmethod
    def from_json(json: str) -> CheckSuiteSpec:
        """Deserialize a suite specification from JSON text.

        Parameters
        ----------
        json:
            JSON document matching the ``CheckSuiteSpec`` schema.

        Returns
        -------
        CheckSuiteSpec
            Parsed specification.

        Raises
        ------
        ValueError
            If ``json`` is not valid or fails schema validation.
        """
        ...

    def to_json(self) -> str:
        """Serialize this specification to pretty-printed JSON.

        Returns
        -------
        str
            JSON text.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    @property
    def name(self) -> str:
        """Suite name.
        Returns
        -------
        str
        """
        ...

    @property
    def builtin_check_count(self) -> int:
        """Number of built-in checks in the suite spec.
        Returns
        -------
        int
        """
        ...

    @property
    def formula_check_count(self) -> int:
        """Number of formula checks in the suite spec.
        Returns
        -------
        int
        """
        ...

    def __repr__(self) -> str:
        """Return a concise summary of the suite spec.
        Returns
        -------
        str
        """
        ...

class CheckReport:
    """Validation check report aggregating results and summary statistics.

    Loaded from JSON (``from_json``) produced by the Rust checks framework,
    then inspected via properties or rendered to text/HTML.

    Example
    -------
    >>> from finstack_quant.statements import CheckReport
    >>> report = CheckReport.from_json(
    ...     '{"results":[],"summary":{"total_checks":0,"passed":0,"failed":0,"errors":0,"warnings":0,"infos":0}}'
    ... )
    >>> report.passed
    True
    """

    @staticmethod
    def from_json(json: str) -> CheckReport:
        """Deserialize a check report from JSON text.

        Parameters
        ----------
        json:
            JSON document matching the ``CheckReport`` schema.

        Returns
        -------
        CheckReport
            Parsed report.

        Raises
        ------
        ValueError
            If ``json`` is not valid or fails schema validation.
        """
        ...

    def to_json(self) -> str:
        """Serialize this report to pretty-printed JSON.

        Returns
        -------
        str
            JSON text.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    @property
    def passed(self) -> bool:
        """Whether all checks passed (no error-severity findings).
        Returns
        -------
        bool
        """
        ...

    @property
    def total_checks(self) -> int:
        """Number of individual check results in the report.
        Returns
        -------
        int
        """
        ...

    @property
    def total_findings(self) -> int:
        """Total number of findings across all checks.
        Returns
        -------
        int
        """
        ...

    @property
    def total_errors(self) -> int:
        """Number of error-severity findings.
        Returns
        -------
        int
        """
        ...

    @property
    def total_warnings(self) -> int:
        """Number of warning-severity findings.
        Returns
        -------
        int
        """
        ...

    def __repr__(self) -> str:
        """Return a concise summary of the check report.
        Returns
        -------
        str
        """
        ...

class EcfSweepSpec:
    """Excess Cash Flow sweep specification.

    Configures how ECF is computed (EBITDA minus taxes/capex/WC/cash interest)
    and what fraction sweeps to debt paydown.
    """

    def __init__(
        self,
        ebitda_node: str,
        sweep_percentage: float,
        taxes_node: str | None = None,
        capex_node: str | None = None,
        working_capital_node: str | None = None,
        cash_interest_node: str | None = None,
        target_instrument_id: str | None = None,
    ) -> None: ...
    @staticmethod
    def from_json(json: str) -> EcfSweepSpec: ...
    def to_json(self) -> str: ...
    @property
    def ebitda_node(self) -> str: ...
    @property
    def sweep_percentage(self) -> float: ...
    @property
    def target_instrument_id(self) -> str | None: ...
    def __repr__(self) -> str: ...

class PikToggleSpec:
    """PIK toggle specification.

    Controls when interest accrues as PIK versus cash based on a liquidity
    signal crossing ``threshold``, with optional hysteresis.
    """

    def __init__(
        self,
        liquidity_metric: str,
        threshold: float,
        target_instrument_ids: list[str] | None = None,
        min_periods_in_pik: int = 0,
    ) -> None: ...
    @staticmethod
    def from_json(json: str) -> PikToggleSpec: ...
    def to_json(self) -> str: ...
    @property
    def liquidity_metric(self) -> str: ...
    @property
    def threshold(self) -> float: ...
    @property
    def min_periods_in_pik(self) -> int: ...
    def __repr__(self) -> str: ...

class WaterfallSpec:
    """Waterfall specification for dynamic cash flow allocation.

    Combines priority-of-payments with optional ECF sweep and PIK toggle.
    Call :meth:`validate` before passing to a builder to surface inconsistent
    configurations (for example ``Sweep`` ordered after ``Equity``).
    """

    def __init__(
        self,
        priority_of_payments: list[str] | None = None,
        available_cash_node: str | None = None,
        ecf_sweep: EcfSweepSpec | None = None,
        pik_toggle: PikToggleSpec | None = None,
    ) -> None: ...
    @staticmethod
    def from_json(json: str) -> WaterfallSpec: ...
    def to_json(self) -> str: ...
    def validate(self) -> None: ...
    @property
    def priority_of_payments(self) -> list[str]: ...
    @property
    def available_cash_node(self) -> str | None: ...
    @property
    def has_ecf_sweep(self) -> bool: ...
    @property
    def has_pik_toggle(self) -> bool: ...
    def __repr__(self) -> str: ...

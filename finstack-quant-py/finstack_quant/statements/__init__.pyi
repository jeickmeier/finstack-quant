"""Financial statement modeling: builders, evaluators, forecasts, DSL, adjustments.

Python bindings for the ``finstack-quant-statements`` Rust crate: model specifications,
``ModelBuilder``, ``Evaluator``, formula parsing/validation, and EBITDA-style
normalization helpers.
"""

from __future__ import annotations

from datetime import date
from typing import Any

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
        """Return a debug representation of this forecast method."""
        ...

class ForecastSpec:
    """Forecast configuration for a statement node."""

    def __init__(self, method: ForecastMethod, params_json: str | None = None) -> None:
        """Create a forecast spec from a method and optional JSON params."""
        ...

    @staticmethod
    def forward_fill() -> ForecastSpec:
        """Carry the last observed value forward."""
        ...

    @staticmethod
    def growth(rate: float) -> ForecastSpec:
        """Compound each future period by ``rate``."""
        ...

    @staticmethod
    def curve(curve: list[float]) -> ForecastSpec:
        """Use period-specific growth rates."""
        ...

    @staticmethod
    def normal(mean: float, std_dev: float, seed: int) -> ForecastSpec:
        """Use deterministic additive normal draws."""
        ...

    @staticmethod
    def lognormal(mean: float, std_dev: float, seed: int) -> ForecastSpec:
        """Use deterministic multiplicative log-normal draws."""
        ...

    @staticmethod
    def from_json(json: str) -> ForecastSpec:
        """Deserialize a forecast spec from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this forecast spec to JSON."""
        ...

    def __repr__(self) -> str:
        """Return a debug representation of this forecast spec."""
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
        """Return a debug representation of this node type."""
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
        """Return a Python-literal style representation."""
        ...

    def __str__(self) -> str:
        """Return the identifier as a plain string."""
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
        """Use decimal arithmetic for evaluation.

        Slower than ``float64`` but avoids floating-point rounding error;
        prefer for currency-sensitive calculations.

        Returns
        -------
        NumericMode
            Decimal arithmetic mode.
        """
        ...

    def __eq__(self, other: object) -> bool:
        """Return whether two numeric mode tokens are equal."""
        ...

    def __repr__(self) -> str:
        """Return a debug representation of this numeric mode."""
        ...

class FinancialModelSpec:
    """Top-level financial model specification (wire format).

    Typically built with ``ModelBuilder`` or loaded from JSON.

    Example
    -------
    >>> from finstack_quant.statements import FinancialModelSpec
    >>> spec = FinancialModelSpec.from_json('{"id":"x","periods":[],"nodes":{}}')
    >>> spec.id
    'x'
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
        >>> FinancialModelSpec.from_json('{"id":"m","periods":[],"nodes":{}}').node_count
        0
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

        Example
        -------
        >>> m = FinancialModelSpec.from_json('{"id":"m","periods":[],"nodes":{}}')
        >>> '"id"' in m.to_json()
        True
        """
        ...

    @property
    def id(self) -> str:
        """Model identifier string."""
        ...

    @property
    def period_count(self) -> int:
        """Number of periods defined on the model."""
        ...

    @property
    def node_count(self) -> int:
        """Number of nodes defined on the model."""
        ...

    def node_ids(self) -> list[str]:
        """List node identifiers in declaration order.

        Returns
        -------
        list[str]
            Ordered node id strings.

        Example
        -------
        >>> FinancialModelSpec.from_json('{"id":"m","periods":[],"nodes":{}}').node_ids()
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
        >>> FinancialModelSpec.from_json('{"id":"m","periods":[],"nodes":{}}').has_node("x")
        False
        """
        ...

    @property
    def schema_version(self) -> int:
        """Wire-format schema version of this specification."""
        ...

    def __repr__(self) -> str:
        """Return a concise summary including id, period count, and node count."""
        ...

class ModelBuilder:
    """Fluent builder for a ``FinancialModelSpec``.

    Call ``periods`` once, then add nodes with ``value`` / ``compute``, and
    finish with ``build``.

    Example
    -------
    >>> from finstack_quant.statements import ModelBuilder
    >>> b = ModelBuilder("co")
    >>> b.periods("2025Q1..Q2", None)  # doctest: +SKIP
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
        """Add a scalar value node with explicit per-period values."""
        ...

    def value_money(self, node_id: str, values: list[tuple[str, Money]]) -> None:
        """Add a monetary value node with explicit per-period values."""
        ...

    def compute(self, node_id: str, formula: str) -> None:
        """Add a calculated node from a DSL formula.

        Parameters
        ----------
        node_id:
            Identifier for the computed node.
        formula:
            Expression in the statements DSL (for example ``\"revenue - cogs\"``).

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
        """Start configuring a mixed node and consume this builder until ``build`` returns."""
        ...

    def forecast(self, node_id: str, forecast_spec: ForecastSpec) -> None:
        """Attach a forecast to an existing node or create a forecast-only mixed node."""
        ...

    def where_clause(self, where_clause: str) -> None:
        """Attach a conditional expression to the last added node."""
        ...

    def with_meta(self, key: str, value_json: str) -> None:
        """Add model-level metadata from a JSON payload."""
        ...

    def with_name_normalization(self) -> None:
        """Enable standard accounting term alias normalization."""
        ...

    def with_builtin_metrics(self) -> None:
        """Add all built-in statement metrics to the model."""
        ...

    def add_metric_from_registry(self, qualified_id: str, registry: MetricRegistry) -> None:
        """Add one metric and its dependencies from a metric registry."""
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
        """Add an interest rate swap to the capital structure (US conventions)."""
        ...

    def add_custom_debt(self, id: str, spec_json: str) -> None:
        """Add an arbitrary debt instrument via its serde JSON representation."""
        ...

    def reporting_currency(self, currency: Currency) -> None:
        """Set the reporting currency used for capital-structure totals."""
        ...

    def fx_policy(self, policy: str) -> None:
        """Set the FX policy (``cashflow_date``/``period_end``/``period_average``/``custom``)."""
        ...

    def waterfall(self, waterfall_spec: WaterfallSpec) -> None:
        """Attach a waterfall specification (PIK toggle + ECF sweep + priorities)."""
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
    """Fluent builder for a mixed statement node."""

    def values(self, values: list[tuple[str, float]]) -> None:
        """Set scalar explicit values."""
        ...

    def values_money(self, values: list[tuple[str, Money]]) -> None:
        """Set monetary explicit values."""
        ...

    def forecast(self, forecast_spec: ForecastSpec) -> None:
        """Set the forecast spec."""
        ...

    def formula(self, formula: str) -> None:
        """Set the fallback formula."""
        ...

    def name(self, name: str) -> None:
        """Set the display name."""
        ...

    def build(self) -> ModelBuilder:
        """Attach the mixed node and return a ready model builder."""
        ...

class MetricRegistry:
    """Reusable statement metric registry."""

    def __init__(self) -> None:
        """Create an empty registry."""
        ...

    @staticmethod
    def with_builtins() -> MetricRegistry:
        """Create a registry preloaded with built-in metrics."""
        ...

    def load_builtins(self) -> None:
        """Load built-in metrics into this registry."""
        ...

    def load_from_json_str(self, json: str) -> None:
        """Load metrics from a JSON document."""
        ...

    def load_from_json(self, path: str) -> None:
        """Load metrics from a JSON file path."""
        ...

    def has(self, qualified_id: str) -> bool:
        """Return whether a fully qualified metric exists."""
        ...

    def __len__(self) -> int:
        """Return the number of metrics."""
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
        """Serialize these results to pretty-printed JSON.

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
        >>> FinancialModelSpec.from_json('{"id":"m","periods":[],"nodes":{}}').to_json()[0]
        '{'
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
        >>> parse_formula("revenue - cogs")  # doctest: +ELLIPSIS
        '...'
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
        >>> validate_formula("revenue / 2")
        True
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
        >>> sorted(FinancialModelSpec.from_json('{"id":"m","periods":[],"nodes":{}}').node_ids())
        []
        """
        ...

    @property
    def node_count(self) -> int:
        """Number of nodes in the result."""
        ...

    @property
    def num_periods(self) -> int:
        """Number of periods covered by the evaluation metadata."""
        ...

    @property
    def eval_time_ms(self) -> int | None:
        """Wall-clock evaluation time in milliseconds, if recorded."""
        ...

    @property
    def warning_count(self) -> int:
        """Count of evaluation warnings attached to metadata."""
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
        """Return a concise summary with node and period counts."""
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
        """Node id being normalized."""
        ...

    @property
    def adjustment_count(self) -> int:
        """Number of adjustment line items configured."""
        ...

    def __repr__(self) -> str:
        """Return a concise summary including target node and adjustment count."""
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
    composition before passing to ``run_checks``.

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
        """Suite name."""
        ...

    @property
    def builtin_check_count(self) -> int:
        """Number of built-in checks in the suite spec."""
        ...

    @property
    def formula_check_count(self) -> int:
        """Number of formula checks in the suite spec."""
        ...

    def __repr__(self) -> str:
        """Return a concise summary of the suite spec."""
        ...

class CheckReport:
    """Validation check report aggregating results and summary statistics.

    Typically produced by ``run_checks`` or similar analytics functions,
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
        """Whether all checks passed (no error-severity findings)."""
        ...

    @property
    def total_checks(self) -> int:
        """Number of individual check results in the report."""
        ...

    @property
    def total_findings(self) -> int:
        """Total number of findings across all checks."""
        ...

    @property
    def total_errors(self) -> int:
        """Number of error-severity findings."""
        ...

    @property
    def total_warnings(self) -> int:
        """Number of warning-severity findings."""
        ...

    def __repr__(self) -> str:
        """Return a concise summary of the check report."""
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

"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements/types.rs``."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from datetime import date
from typing import Any
import pandas as pd

class ForecastMethod:
    """
    Available forecast methods for projecting node values.

    Immutable, hashable enum-style type. Construct variants via static
    factory methods (e.g. ``growth_pct()``).

    Examples
    --------
    >>> from finstack_quant.statements import ForecastMethod
    >>> ForecastMethod.forward_fill() == ForecastMethod.forward_fill()
    True

    """

    @staticmethod
    def forward_fill() -> ForecastMethod:
        """
        Carry the last observed value forward unchanged: ``v[t] = v[t-1]``.

        Takes no parameters and never changes the level, so it is the safe
        default for stock-like nodes (balances) that should hold flat when no
        explicit assumption is supplied.

        Returns
        -------
        ForecastMethod
            Forward-fill forecast method.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.statements import ForecastMethod
        >>> ForecastMethod.forward_fill() == ForecastMethod.forward_fill()
        True
        """
        ...

    @staticmethod
    def growth_pct() -> ForecastMethod:
        """
        Compound growth at a single rate: ``v[t] = v[t-1] * (1 + rate)``.

        ``rate`` is a **decimal fraction per period**, not a percentage — 0.05
        means 5% growth per period, and the period is whatever cadence the
        model's period list uses (quarters, months, years). Rates above 100%
        per period raise an evaluation warning.

        Returns
        -------
        ForecastMethod
            Growth-percentage forecast method.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.statements import ForecastMethod
        >>> ForecastMethod.growth_pct() == ForecastMethod.growth_pct()
        True
        """
        ...

    @staticmethod
    def curve_pct() -> ForecastMethod:
        """
        Period-specific compound growth: ``v[t] = v[t-1] * (1 + curve[t])``.

        ``curve`` supplies one growth rate per forecast period, each a decimal
        fraction per period (0.05 = 5%), consumed in forecast-period order.

        Returns
        -------
        ForecastMethod
            Curve-percentage forecast method.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.statements import ForecastMethod
        >>> ForecastMethod.curve_pct() == ForecastMethod.curve_pct()
        True
        """
        ...

    @staticmethod
    def normal() -> ForecastMethod:
        """
        Additive normal random walk: ``v[t] = v[t-1] + mean + std_dev * z[t]``.

        ``mean`` and ``std_dev`` are per-period **level** quantities in the
        node's own units (currency amounts for ``Money`` nodes, unitless for
        scalar nodes), not rates. ``z[t]`` is a deterministic standard-normal
        draw derived from the configured seed, so runs are reproducible.
        ``std_dev`` must be non-negative.

        Returns
        -------
        ForecastMethod
            Normal distribution forecast method.

        Notes
        -----
        This factory does not raise; it returns a new instance with the documented defaults.

        Examples
        --------
        >>> from finstack_quant.statements import ForecastMethod
        >>> ForecastMethod.normal() == ForecastMethod.normal()
        True
        """
        ...

    @staticmethod
    def log_normal() -> ForecastMethod:
        """
        Multiplicative log-normal path:
        ``v[t] = v[t-1] * exp(mean - 0.5 * std_dev**2 + std_dev * z[t])``.

        ``mean`` is the per-period continuously compounded expected-growth
        drift: the expected growth factor is ``exp(mean)``, while the expected
        log increment is ``mean - 0.5 * std_dev**2``. ``std_dev`` is log-return
        volatility, expressed as a decimal fraction per period. Evaluation
        requires a finite, strictly positive base; zero and negative bases
        raise ``ValueError``. Use an additive normal forecast for those series.

        Returns
        -------
        ForecastMethod
            Log-normal forecast method.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.statements import ForecastMethod
        >>> ForecastMethod.log_normal() == ForecastMethod.log_normal()
        True
        """
        ...

    @staticmethod
    def override() -> ForecastMethod:
        """
        Explicit per-period overrides, forward-filling the periods in between.

        The method reads an ``overrides`` parameter mapping period-identifier
        strings (e.g. ``"2025Q2"``) to values; any forecast period without an
        entry inherits the most recent value. Mirrors the Rust ``Override``
        variant; the wire form is ``"override"``.

        Returns
        -------
        ForecastMethod
            Override forecast method.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.statements import ForecastMethod
        >>> ForecastMethod.override() == ForecastMethod.override()
        True
        """
        ...

    @staticmethod
    def time_series() -> ForecastMethod:
        """
        Project an externally supplied historical series forward with trend
        detection.

        The history is passed through the spec's parameters rather than read
        from the model graph, so the same series can drive several nodes.

        Returns
        -------
        ForecastMethod
            External time-series forecast method.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.statements import ForecastMethod
        >>> ForecastMethod.time_series() == ForecastMethod.time_series()
        True
        """
        ...

    @staticmethod
    def seasonal() -> ForecastMethod:
        """
        Seasonal decomposition forecast (additive or multiplicative).

        Requires a ``season_length`` parameter counted in **periods** (4 for a
        quarterly model, 12 for monthly) and at least two full seasonal cycles
        of history. Use additive mode when seasonal swings are constant in
        absolute terms and multiplicative when they scale with the level;
        prefer additive for series that cross zero.

        Returns
        -------
        ForecastMethod
            Seasonal forecast method.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.statements import ForecastMethod
        >>> ForecastMethod.seasonal() == ForecastMethod.seasonal()
        True
        """
        ...

    @staticmethod
    def fade_to_target() -> ForecastMethod:
        """
        Glide from the base value to a ``target`` level over the forecast
        horizon.

        Shapes (``shape`` parameter, default ``"linear"``): ``"linear"``
        reaches the target exactly at the final period; ``"geometric"`` is
        constant compound growth (CAGR-to-terminal, requires base and target
        non-zero with the same sign); ``"exponential"`` decays the remaining
        gap by half every ``half_life`` periods and never quite reaches the
        target. The workhorse for fading margins/ratios to a long-run
        assumption (NIM normalization, cost-income convergence, ROE fade).

        Returns
        -------
        ForecastMethod
            Fade-to-target forecast method.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.statements import ForecastMethod
        >>> ForecastMethod.fade_to_target() == ForecastMethod.fade_to_target()
        True
        """
        ...

    @staticmethod
    def mean_reverting() -> ForecastMethod:
        """
        Mean-reverting AR(1) path:
        ``v[t] = v[t-1] + reversion_speed * (long_run_mean - v[t-1]) + std_dev * z[t]``.

        ``long_run_mean`` and ``std_dev`` are in the node's own units;
        ``reversion_speed`` is the fraction of the gap closed each period, in
        ``(0, 1]``. ``z[t]`` is a deterministic standard-normal draw derived
        from the configured seed. Use for autocorrelated series that revert
        toward a through-the-cycle level (spreads, charge-off rates, NIM).

        Returns
        -------
        ForecastMethod
            Mean-reverting forecast method.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.statements import ForecastMethod
        >>> ForecastMethod.mean_reverting() == ForecastMethod.mean_reverting()
        True
        """
        ...

    @staticmethod
    def bootstrap() -> ForecastMethod:
        """
        Historical bootstrap: resample observed per-period changes with
        replacement (deterministic with seed).

        ``mode = "growth"`` (default) resamples period-over-period growth
        rates from a strictly positive ``historical`` series and compounds
        them; ``mode = "diff"`` resamples additive level changes and works
        for series that cross zero. Reproduces the empirical distribution of
        changes — including fat tails — without a normality assumption.

        Returns
        -------
        ForecastMethod
            Bootstrap forecast method.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.statements import ForecastMethod
        >>> ForecastMethod.bootstrap() == ForecastMethod.bootstrap()
        True
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

    @property
    def kind(self) -> str:
        """
        Return the canonical snake_case forecast discriminant.

        Returns
        -------
        str
            Wire method name, such as "growth_pct" or "log_normal".

        Raises
        ------
        RuntimeError
            If the stored enum cannot be rendered as its canonical Rust wire label.
        """
        ...

class ForecastSpec:
    """
    Forecast configuration for a statement node.

    Examples
    --------
    >>> from finstack_quant.statements import ForecastSpec
    >>> spec = ForecastSpec.growth(0.05)
    >>> ForecastSpec.from_json(spec.to_json()).to_json() == spec.to_json()
    True

    """

    def __init__(self, method: ForecastMethod, params: Mapping[str, Any] | str | None = None) -> None:
        """
        Build a forecast spec from a method plus its method-specific parameters.

        Parameters
        ----------
        method:
            A :class:`ForecastMethod` describing the projection rule applied
            to forecast periods.
        params:
            Mapping (or JSON object string) of method-specific parameters (``rate``, ``curve``,
            ``mean``, ``std_dev``, ``seed``, ``overrides``, ``season_length``,
            ``target``, ``shape``, ``half_life``, ``long_run_mean``,
            ``reversion_speed``, ``historical``, ``mode``, ``phi``, ...).
            Rates are decimal fractions per period. Every method also accepts
            optional ``min`` / ``max`` bounds that clamp generated values to a
            band (in the node's own units). ``None`` means no parameters,
            which is only valid for ``forward_fill``.

        Raises
        ------
        ValueError
            If ``params`` is not a valid parameter mapping for the method.

        """
        ...

    @staticmethod
    def forward_fill() -> ForecastSpec:
        """
        Forward-fill spec: hold the last observed value flat, no parameters.

        Returns
        -------
        ForecastSpec
            A forward-fill forecast specification.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.statements import ForecastSpec
        >>> spec = ForecastSpec.forward_fill()
        >>> ForecastSpec.from_json(spec.to_json()).to_json() == spec.to_json()
        True
        """
        ...

    @staticmethod
    def growth(rate: float) -> ForecastSpec:
        """
        Constant compound-growth spec: ``v[t] = v[t-1] * (1 + rate)``.

        Parameters
        ----------
        rate:
            Growth rate as a **decimal fraction per period** (0.05 = 5% per
            period, on the model's own period cadence). Negative values decay
            the series.

        Returns
        -------
        ForecastSpec
            A constant-growth forecast specification.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.statements import ForecastSpec
        >>> spec = ForecastSpec.growth(0.05)
        >>> ForecastSpec.from_json(spec.to_json()).to_json() == spec.to_json()
        True
        """
        ...

    @staticmethod
    def curve(curve: list[float]) -> ForecastSpec:
        """
        Period-by-period growth spec: ``v[t] = v[t-1] * (1 + curve[t])``.

        Parameters
        ----------
        curve:
            One growth rate per forecast period, each a decimal fraction per
            period (0.05 = 5%), consumed in forecast-period order.

        Returns
        -------
        ForecastSpec
            A curve-based forecast specification.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.statements import ForecastSpec
        >>> spec = ForecastSpec.curve([0.03, 0.04])
        >>> ForecastSpec.from_json(spec.to_json()).to_json() == spec.to_json()
        True
        """
        ...

    @staticmethod
    def normal(mean: float, std_dev: float, seed: int) -> ForecastSpec:
        """
        Additive normal random-walk spec:
        ``v[t] = v[t-1] + mean + std_dev * z[t]``.

        Parameters
        ----------
        mean:
            Per-period additive drift, in the node's own units (currency
            amount for ``Money`` nodes, unitless for scalar nodes).
        std_dev:
            Per-period additive volatility in the same units; must be
            non-negative. Zero gives a deterministic drift path.
        seed:
            Seed for the deterministic standard-normal draws. The evaluator
            mixes a stable hash of the node id into it, so two nodes sharing a
            seed still receive independent shocks.

        Returns
        -------
        ForecastSpec
            A normal-draw forecast specification.

        Notes
        -----
        This factory does not raise; it returns a new instance with the documented defaults.

        Examples
        --------
        >>> from finstack_quant.statements import ForecastSpec
        >>> spec = ForecastSpec.normal(0.0, 0.1, 7)
        >>> ForecastSpec.from_json(spec.to_json()).to_json() == spec.to_json()
        True
        """
        ...

    @staticmethod
    def log_normal(mean: float, std_dev: float, seed: int) -> ForecastSpec:
        """
        Multiplicative log-normal spec:
        ``v[t] = v[t-1] * exp(mean - 0.5 * std_dev**2 + std_dev * z[t])``.

        Parameters
        ----------
        mean:
            Per-period continuously compounded expected-growth drift. The
            expected growth factor is ``exp(mean)`` and expected log increment
            is ``mean - 0.5 * std_dev**2``. For example, ``mean=0.02`` gives
            expected simple growth ``exp(0.02) - 1``, approximately 2.0201%.
        std_dev:
            Per-period log-return volatility as a decimal fraction; must be
            non-negative.
        seed:
            Seed for the deterministic standard-normal draws.

        Returns
        -------
        ForecastSpec
            A log-normal-draw forecast specification.

        Notes
        -----
        This constructor does not raise; it only stores the supplied parameters.
        Evaluation requires a finite, strictly positive base value and finite
        parameters; invalid inputs raise ``ValueError`` when the model is
        validated or evaluated.

        Examples
        --------
        >>> from finstack_quant.statements import ForecastSpec
        >>> spec = ForecastSpec.log_normal(0.0, 0.1, 7)
        >>> ForecastSpec.from_json(spec.to_json()).to_json() == spec.to_json()
        True
        """
        ...

    @staticmethod
    def fade_to_target(target: float) -> ForecastSpec:
        """
        Linear fade-to-target spec:
        ``v[t] = base + (target - base) * t / N``.

        Values glide from the last observed value to ``target`` in equal
        steps, reaching it exactly at the final forecast period. For the
        ``"geometric"`` (CAGR-to-terminal) or ``"exponential"`` (half-life)
        shapes, construct ``ForecastSpec(ForecastMethod.fade_to_target(),
        params)`` with ``shape`` and, for exponential, ``half_life``.

        Parameters
        ----------
        target:
            Terminal level in the node's own units (a currency amount for
            monetary nodes, a decimal ratio for scalar nodes such as margins).

        Returns
        -------
        ForecastSpec
            A linear fade-to-target forecast specification.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.statements import ForecastSpec
        >>> spec = ForecastSpec.fade_to_target(0.02)
        >>> ForecastSpec.from_json(spec.to_json()).to_json() == spec.to_json()
        True
        """
        ...

    @staticmethod
    def mean_reverting(long_run_mean: float, reversion_speed: float, std_dev: float, seed: int) -> ForecastSpec:
        """
        Mean-reverting AR(1) spec:
        ``v[t] = v[t-1] + reversion_speed * (long_run_mean - v[t-1]) + std_dev * z[t]``.

        Parameters
        ----------
        long_run_mean:
            Level the series reverts toward, in the node's own units
            (currency amount for monetary nodes, decimal ratio for scalars).
        reversion_speed:
            Fraction of the gap closed each period, in ``(0, 1]``; 1.0
            reverts fully every period, values near 0 revert slowly.
        std_dev:
            Per-period additive shock volatility in the node's own units;
            must be non-negative. Zero gives deterministic geometric decay of
            the gap.
        seed:
            Seed for the deterministic standard-normal draws. The evaluator
            mixes a stable hash of the node id into it, so two nodes sharing
            a seed still receive independent shocks.

        Returns
        -------
        ForecastSpec
            A mean-reverting forecast specification.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.statements import ForecastSpec
        >>> spec = ForecastSpec.mean_reverting(0.05, 0.25, 0.01, 7)
        >>> ForecastSpec.from_json(spec.to_json()).to_json() == spec.to_json()
        True
        """
        ...

    @staticmethod
    def bootstrap(historical: list[float], seed: int) -> ForecastSpec:
        """
        Growth-mode bootstrap spec: resample historical growth rates and
        compound them from the base value.

        The history must be strictly positive in growth mode. For additive
        resampling of level changes (series that cross zero), construct
        ``ForecastSpec(ForecastMethod.bootstrap(), params)`` with
        ``mode = "diff"``.

        Parameters
        ----------
        historical:
            At least 2 historical values in the node's own units, oldest
            first; consecutive pairs define the resampled growth rates.
        seed:
            Seed for the deterministic resampling draws.

        Returns
        -------
        ForecastSpec
            A growth-mode bootstrap forecast specification.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.statements import ForecastSpec
        >>> spec = ForecastSpec.bootstrap([100.0, 105.0, 110.0], 7)
        >>> ForecastSpec.from_json(spec.to_json()).to_json() == spec.to_json()
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> ForecastSpec:
        """
        Deserialize a forecast spec from JSON.

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

        Examples
        --------
        >>> from finstack_quant.statements import ForecastSpec
        >>> spec = ForecastSpec.growth(0.05)
        >>> ForecastSpec.from_json(spec.to_json()).to_json() == spec.to_json()
        True

        """
        ...

    def to_json(self) -> str:
        """
        Serialize this forecast spec to JSON.

        Returns
        -------
        str
            Canonical JSON representation of this forecast specification.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def __repr__(self) -> str:
        """Return a debug representation of this forecast spec.
        Returns
        -------
        str
        """
        ...

    @property
    def method(self) -> ForecastMethod:
        """Return the projection rule used by this specification.

        Returns
        -------
        ForecastMethod
            Forecast method applied to non-actual model periods.

        Notes
        -----
        This accessor does not raise; it returns stored or derived model data.
        """
        ...

    @property
    def params(self) -> dict[str, Any]:
        """Return method-specific forecast parameters as plain containers.

        Returns
        -------
        dict[str, Any]
            Parameter mapping in insertion order; rates are decimal fractions per model period and levels use the node's units.

        Notes
        -----
        This accessor does not raise; it returns stored or derived model data.
        """
        ...

    @staticmethod
    def overrides(overrides: Mapping[str, float] | Sequence[tuple[str, float]] | pd.Series) -> ForecastSpec:
        """Pin selected forecast periods and forward-fill the periods between them.

        Parameters
        ----------
        overrides:
            Period identifier to value in the node's units; identifiers must belong to the eventual model timeline.

        Returns
        -------
        ForecastSpec
            Per-period override specification.

        Raises
        ------
        ValueError
            If a period identifier cannot be parsed.
        TypeError
            If a supplied cell cannot be converted to a float.

        Examples
        --------
        >>> from finstack_quant.statements import ForecastSpec
        >>> ForecastSpec.overrides({"2025Q2": 120.0}).method.kind
        'override'
        """
        ...

    @staticmethod
    def seasonal(historical: list[float], season_length: int, mode: str) -> ForecastSpec:
        """Forecast an external history using seasonal decomposition.

        Parameters
        ----------
        historical:
            Oldest-first levels in the node's units; eventual evaluation requires at least two complete seasons.
        season_length:
            Positive number of model periods per seasonal cycle, such as 4 for quarterly observations.
        mode:
            Required. "additive" for constant absolute swings or "multiplicative" for swings scaling with the level.

        Returns
        -------
        ForecastSpec
            Seasonal specification validated with its eventual model.

        Raises
        ------
        ValueError
            If mode is not additive or multiplicative.
        OverflowError
            If season_length cannot be represented as a non-negative machine integer.

        Examples
        --------
        >>> from finstack_quant.statements import ForecastSpec
        >>> ForecastSpec.seasonal([10.0, 12.0, 11.0, 13.0], 2, "additive").method.kind
        'seasonal'
        """
        ...

    @staticmethod
    def time_series(historical: list[float]) -> ForecastSpec:
        """Forecast an external historical series using its estimated linear trend.

        Parameters
        ----------
        historical:
            Oldest-first levels in the node's units; eventual evaluation requires at least two observations.

        Returns
        -------
        ForecastSpec
            Linear time-series forecast configuration.

        Notes
        -----
        This accessor does not raise; it returns stored or derived model data.

        Examples
        --------
        >>> from finstack_quant.statements import ForecastSpec
        >>> ForecastSpec.time_series([100.0, 105.0, 110.0]).method.kind
        'time_series'
        """
        ...

class NodeType:
    """
    How a node combines explicit values, forecasts, and formulas.

    Examples
    --------
    >>> from finstack_quant.statements import NodeType
    >>> NodeType.calculated() == NodeType.calculated()
    True

    """

    @staticmethod
    def value() -> NodeType:
        """
        Value node: explicit per-period values only (actuals, assumptions).

        The node has no formula and no forecast, so periods without an
        explicit value stay unset.

        Returns
        -------
        NodeType
            Value-only node type.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> from finstack_quant.statements import NodeType
        >>> NodeType.value() == NodeType.value()
        True
        """
        ...

    @staticmethod
    def calculated() -> NodeType:
        """
        Calculated node: derived from its formula in every period.

        Returns
        -------
        NodeType
            Calculated node type.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.statements import NodeType
        >>> NodeType.calculated() == NodeType.calculated()
        True
        """
        ...

    @staticmethod
    def mixed() -> NodeType:
        """
        Mixed node: explicit value, else forecast, else formula.

        This is the crate's core precedence invariant — **Value > Forecast >
        Formula** — resolved independently for each period, so a node can be
        an actual in historical periods, a forecast in projected periods, and
        a formula wherever neither is supplied.

        Returns
        -------
        NodeType
            Mixed node type.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.statements import NodeType
        >>> NodeType.mixed() == NodeType.mixed()
        True
        """
        ...

    @property
    def kind(self) -> str:
        """
        Canonical snake-case wire discriminant for this node type.

        Returns
        -------
        str
            ``"value"``, ``"calculated"``, or ``"mixed"``, matching the JSON
            schema rename for :class:`NodeType`.

        Raises
        ------
        RuntimeError
            If the stored enum cannot be rendered as its canonical Rust wire label.
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
    """
    Type-safe identifier for a node in a financial model.

    Examples
    --------
    >>> from finstack_quant.statements import NodeId
    >>> str(NodeId("revenue"))
    'revenue'

    """

    def __init__(self, id: str) -> None:
        """
        Wrap a raw node identifier string.

        Parameters
        ----------
        id:
            Node identifier as it appears in the model graph and in formulas
            (for example ``"revenue"``). The value is stored verbatim — no
            case folding or trimming — so identifiers are matched exactly.

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.

        Examples
        --------
        >>> from finstack_quant.statements import NodeId
        >>> NodeId("ebitda").as_str()
        'ebitda'
        """
        ...

    def as_str(self) -> str:
        """
        Return the underlying identifier string.

        Returns
        -------
        str
            The identifier exactly as supplied at construction.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.

        Examples
        --------
        >>> from finstack_quant.statements import NodeId
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
    """
    Numeric evaluation mode for statement evaluation.

    Examples
    --------
    >>> from finstack_quant.statements import NumericMode
    >>> NumericMode.float64() == NumericMode.float64()
    True

    """

    @staticmethod
    def float64() -> NumericMode:
        """
        Use 64-bit floating point arithmetic.

        Returns
        -------
        NumericMode
            IEEE-754 double-precision mode.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.statements import NumericMode
        >>> NumericMode.float64() == NumericMode.float64()
        True
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
    """
    Top-level financial model specification (wire format).

    Typically built with ``ModelBuilder`` or loaded from JSON.

    Examples
    --------
    >>> from finstack_quant.statements import ModelBuilder
    >>> builder = ModelBuilder("demo")
    >>> _ = builder.periods("2025Q1..Q1")
    >>> builder.build().id
    'demo'

    """

    @staticmethod
    def from_json(json: str) -> FinancialModelSpec:
        """
        Deserialize a model specification from JSON text.

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

        Examples
        --------
        >>> from finstack_quant.statements import FinancialModelSpec
        >>> payload = (
        ...     '{"id":"demo","periods":[{"id":"2025Q1","start":"2025-01-01",'
        ...     '"end":"2025-04-01","is_actual":false}],"nodes":{},"schema_version":1}'
        ... )
        >>> (FinancialModelSpec.from_json(payload).id, FinancialModelSpec.from_json(payload).node_count)
        ('demo', 0)

        """
        ...

    def to_json(self) -> str:
        """
        Serialize this specification to compact JSON.

        Returns
        -------
        str
            JSON text.

            Canonical JSON representation of this `FinancialModelSpec`, suitable for a matching `from_json` call.
        Raises
        ------
        ValueError
            If serialization fails.

        Examples
        --------
        >>> from finstack_quant.statements import ModelBuilder
        >>> builder = ModelBuilder("demo")
        >>> _ = builder.periods("2025Q1..Q1")
        >>> '"id":"demo"' in builder.build().to_json()
        True

        """
        ...

    def content_hash(self) -> str:
        """
        Versioned SHA-256 content hash of the canonical model JSON.

        Mirrors Rust ``FinancialModelSpec::content_hash`` (WASM
        ``statements.financialModelContentHash``). The hash is taken over the
        canonical JSON, so it does not depend on key order or on how typed
        numbers were spelled in the source document.

        Returns
        -------
        str
            ``"sha256:<hex>"`` content hash.

        Raises
        ------
        ValueError
            If the model contains a non-finite number.

        Examples
        --------
        >>> from finstack_quant.statements import ModelBuilder
        >>> builder = ModelBuilder("demo")
        >>> _ = builder.periods("2025Q1..Q1")
        >>> builder.build().content_hash().startswith("sha256:")
        True
        """
        ...

    @property
    def id(self) -> str:
        """
        Model identifier string.
        Returns
        -------
        str
            Unique model identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def period_count(self) -> int:
        """
        Number of periods in the model timeline.

        Returns
        -------
        int
            Count in **periods** on the model's own cadence (quarters, months,
            years), not months. Their declared order *is* the evaluation
            timeline.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def node_count(self) -> int:
        """
        Number of nodes defined on the model.
        Returns
        -------
        int
            Number of nodes (line items / metrics) declared in the model.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def node_ids(self) -> list[str]:
        """
        List node identifiers in declaration order.

        Returns
        -------
        list[str]
            Ordered node id strings.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.

        Examples
        --------
        >>> from finstack_quant.statements import ModelBuilder
        >>> builder = ModelBuilder("demo")
        >>> _ = builder.periods("2025Q1..Q1")
        >>> builder.build().node_ids()
        []
        """
        ...

    def has_node(self, node_id: str) -> bool:
        """
        Return whether a node with the given id exists.

        Parameters
        ----------
        node_id:
            Node identifier to test.

        Returns
        -------
        bool
            ``True`` if present.

        Notes
        -----
        This method does not raise; it returns ``True`` or ``False``.

        Examples
        --------
        >>> from finstack_quant.statements import ModelBuilder
        >>> builder = ModelBuilder("demo")
        >>> _ = builder.periods("2025Q1..Q1")
        >>> builder.build().has_node("revenue")
        False
        """
        ...

    @property
    def schema_version(self) -> int:
        """
        Wire-format schema version of this model spec.

        Returns
        -------
        int
            Only version ``1`` is accepted today; the field exists so persisted
            models can be migrated rather than silently misread.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise summary including id, period count, and node count.
        Returns
        -------
        str
        """
        ...

    @property
    def periods(self) -> list[str]:
        """Return periods from the model timeline.

        Returns
        -------
        list[str]
            All model period identifiers in timeline order.

        Notes
        -----
        This accessor does not raise; it returns stored or derived model data.
        """
        ...

    @property
    def actual_periods(self) -> list[str]:
        """Return actual periods from the model timeline.

        Returns
        -------
        list[str]
            Period identifiers in the historical prefix, in timeline order.

        Notes
        -----
        This accessor does not raise; it returns stored or derived model data.
        """
        ...

    @property
    def forecast_periods(self) -> list[str]:
        """Return forecast periods from the model timeline.

        Returns
        -------
        list[str]
            Non-actual period identifiers in evaluation order.

        Notes
        -----
        This accessor does not raise; it returns stored or derived model data.
        """
        ...

    @property
    def nodes(self) -> list[NodeSpec]:
        """Return copies of all node specifications in declaration order.

        Returns
        -------
        list[NodeSpec]
            Configured node specifications; changing a returned copy does not mutate the model.

        Notes
        -----
        This accessor does not raise; it returns stored or derived model data.
        """
        ...

    @property
    def meta(self) -> dict[str, Any]:
        """Return model metadata as plain Python containers.

        Returns
        -------
        dict[str, Any]
            Metadata in its JSON shape, including any reporting-currency hint.

        Notes
        -----
        This accessor does not raise; it returns stored or derived model data.
        """
        ...

    @property
    def capital_structure(self) -> dict[str, Any] | None:
        """Return the capital-structure specification in its canonical JSON shape.

        Returns
        -------
        dict[str, Any] | None
            Debt instruments, reporting currency, FX policy, waterfall, and metadata; None when the model has no capital structure.

        Notes
        -----
        This accessor does not raise; it returns stored or derived model data.
        """
        ...

    def get_node(self, node_id: str) -> NodeSpec | None:
        """Look up a configured statement node.

        Parameters
        ----------
        node_id:
            Exact node identifier declared in this model.

        Returns
        -------
        NodeSpec | None
            A copy of the node configuration, or None if the identifier is absent.

        Notes
        -----
        This accessor does not raise; it returns stored or derived model data.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export one descriptive row for each configured model node.

        Returns
        -------
        pd.DataFrame
            Rows in declaration order with node_id, node_type, name, formula_text, forecast_method, value_type, currency, and where_text columns; optional fields are null.

        Raises
        ------
        ImportError
            If pandas is unavailable.
        ValueError
            If a node description cannot be converted to the documented column schema.

        RuntimeError
            If the stored enum cannot be rendered as its canonical Rust wire label.
        """
        ...

class NodeSpec:
    """One node of a :class:`FinancialModelSpec`: type, values, formula, forecast.

    Examples
    --------
    >>> from finstack_quant.statements import ModelBuilder
    >>> b = ModelBuilder("demo")
    >>> _ = b.periods("2025Q1..Q2", "2025Q1")
    >>> _ = b.value("revenue", [("2025Q1", 100.0)])
    >>> b.build().get_node("revenue").node_type.kind
    'value'

    """

    def __init__(
        self,
        node_id: str,
        node_type: NodeType,
        name: str | None = None,
        values: Mapping[str, float] | None = None,
        formula_text: str | None = None,
        forecast: ForecastSpec | None = None,
    ) -> None:
        """Construct a node specification.

        Parameters
        ----------
        node_id : str
            Node identifier; must not use a reserved prefix, DSL keyword, or the export timeline name ``period_id``.
        node_type : NodeType
            ``value``, ``calculated`` or ``mixed``.
        name : str, optional
            Human-readable label.
        values : Mapping[str, float], optional
            Explicit observations keyed by period id (e.g. ``"2025Q1"``).
        formula_text : str, optional
            Statements DSL expression for calculated and mixed nodes.
        forecast : ForecastSpec, optional
            Forecast rule applied to forecast periods.

        Raises
        ------
        ValueError
            If the node id, formula or period ids are rejected.

        """
        ...

    @property
    def node_id(self) -> str:
        """
        Identifier of this node.

        This property does not raise.

        Returns
        -------
        str
            Identifier of this node.
        """
        ...

    @property
    def name(self) -> str | None:
        """
        Human-readable label, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Human-readable label, or ``None``.
        """
        ...

    @property
    def node_type(self) -> NodeType:
        """
        Whether the node is ``value``, ``calculated`` or ``mixed``.

        This property does not raise.

        Returns
        -------
        NodeType
            Whether the node is ``value``, ``calculated`` or ``mixed``.
        """
        ...

    @property
    def values(self) -> dict[str, float] | None:
        """
        Explicit observations keyed by period id, or ``None``.

        This property does not raise.

        Returns
        -------
        dict[str, float] | None
            Explicit observations keyed by period id, or ``None``.
        """
        ...

    @property
    def availability_dates(self) -> dict[str, date]:
        """
        Publication date of each explicit observation, keyed by period id.

        This property does not raise.

        Returns
        -------
        dict[str, date]
            Publication date of each explicit observation, keyed by period id.
        """
        ...

    @property
    def forecast(self) -> ForecastSpec | None:
        """
        Forecast rule applied to forecast periods, or ``None``.

        This property does not raise.

        Returns
        -------
        ForecastSpec | None
            Forecast rule applied to forecast periods, or ``None``.
        """
        ...

    @property
    def formula_text(self) -> str | None:
        """
        Statements DSL expression, or ``None`` for pure value nodes.

        This property does not raise.

        Returns
        -------
        str | None
            Statements DSL expression, or ``None`` for pure value nodes.
        """
        ...

    @property
    def where_text(self) -> str | None:
        """
        DSL guard restricting the periods the node applies to, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            DSL guard restricting the periods the node applies to, or ``None``.
        """
        ...

    @property
    def tags(self) -> list[str]:
        """
        Free-form classification tags.

        This property does not raise.

        Returns
        -------
        list[str]
            Free-form classification tags.
        """
        ...

    @property
    def value_type(self) -> str | None:
        """
        ``"scalar"`` or ``"money"``, or ``None`` when unset.

        This property does not raise.

        Returns
        -------
        str | None
            ``"scalar"`` or ``"money"``, or ``None`` when unset.
        """
        ...

    @property
    def currency(self) -> str | None:
        """
        ISO-4217 code for money-typed nodes, otherwise ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            ISO-4217 code for money-typed nodes, otherwise ``None``.
        """
        ...

    @property
    def meta(self) -> dict[str, object]:
        """
        Arbitrary JSON metadata attached to the node.

        This property does not raise.

        Returns
        -------
        dict[str, object]
            Arbitrary JSON metadata attached to the node.
        """
        ...

    def to_json(self) -> str:
        """Serialize to the canonical JSON wire form.

        Returns
        -------
        str
            Payload accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If serialization fails.

        """
        ...

    @staticmethod
    def from_json(json: str, /) -> NodeSpec:
        """
        Deserialize from the canonical JSON wire form.

        Parameters
        ----------
        json : str
            Payload produced by :meth:`to_json`.

        Returns
        -------
        NodeSpec
            Reconstructed node specification.

        Raises
        ------
        ValueError
            If the payload is malformed.

        Examples
        --------
        >>> from finstack_quant.statements import NodeSpec
        >>> NodeSpec.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """
        ...

    def __repr__(self) -> str: ...

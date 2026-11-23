"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements/types/forecast.rs``."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from datetime import date
from typing import Any, Literal
import pandas as pd
import polars as pl
from finstack_quant.core.money import Money
from finstack_quant.core.currency import Currency
from finstack_quant.core.table import ArrowTable
from finstack_quant.statements._builder import ModelBuilder

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
            optional ``min`` / ``max`` bounds that clamp the published values
            to a band (in the node's own units) without changing the levels
            later steps build on; each clipped cell is reported as a
            ``forecast_clipped`` warning. Methods that step from a base
            (``growth_pct``, ``curve_pct``, ``fade_to_target``, ``normal``,
            ``log_normal``, ``mean_reverting``, ``bootstrap``) accept an
            optional ``start`` level in the node's own units; without it
            the base is the visible value in the period just before the
            first forecast period, and a forecast with no base is
            unavailable (``missing_forecast_base``). ``None`` means no parameters,
            which is only valid for ``forward_fill``. ``seed`` is an exact
            unsigned 64-bit ``int`` (a decimal string in a JSON object
            string). The ``time_series`` ``method``, the ``bootstrap``
            ``mode``, the ``seasonal`` ``mode`` and the ``fade_to_target``
            ``shape`` have no default and must be given.

        Raises
        ------
        ValueError
            If ``params`` is not a JSON object / dict, names a parameter the
            method does not accept, omits a required one, sets one to
            ``null``/``None``, carries a value of the wrong type (a string
            ``rate``, a non-list ``curve``), or fails a static requirement of
            the method (a non-finite number, ``min`` above ``max``, a negative
            ``std_dev``, a ``half_life`` on a non-exponential fade).

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
    def fade_to_target(target: float, shape: Literal["linear", "geometric", "exponential"]) -> ForecastSpec:
        """
        Fade-to-target spec with the chosen shape.

        Values glide from the last observed value to ``target``. The
        ``"linear"`` shape moves in equal steps,
        ``v[t] = base + (target - base) * t / N``, and reaches the target at
        the final forecast period; ``"geometric"`` compounds toward it;
        ``"exponential"`` closes the gap with a ``half_life`` set through
        ``ForecastSpec(ForecastMethod.fade_to_target(), params)``.

        Parameters
        ----------
        target:
            Terminal level in the node's own units (a currency amount for
            monetary nodes, a decimal ratio for scalar nodes such as margins).
        shape:
            Path of the fade. There is no default.

        Returns
        -------
        ForecastSpec
            A fade-to-target forecast specification.

        Raises
        ------
        ValueError
            If ``shape`` is not one of the three shapes.

        Examples
        --------
        >>> from finstack_quant.statements import ForecastSpec
        >>> spec = ForecastSpec.fade_to_target(0.02, "linear")
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
    def bootstrap(historical: list[float], mode: Literal["growth", "diff"], seed: int) -> ForecastSpec:
        """
        Bootstrap spec: resample historical changes and apply them from the
        base value.

        ``"growth"`` resamples growth rates and compounds them, which needs a
        strictly positive history; ``"diff"`` resamples level changes and
        suits series that cross zero.

        Parameters
        ----------
        historical:
            At least 2 historical values in the node's own units, oldest
            first; consecutive pairs define the resampled changes.
        mode:
            What is resampled. There is no default.
        seed:
            Exact unsigned 64-bit seed for the resampling draws.

        Returns
        -------
        ForecastSpec
            A bootstrap forecast specification.

        Raises
        ------
        ValueError
            If ``mode`` is not ``"growth"`` or ``"diff"``.

        Examples
        --------
        >>> from finstack_quant.statements import ForecastSpec
        >>> spec = ForecastSpec.bootstrap([100.0, 105.0, 110.0], "growth", 7)
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
            Parameter mapping in insertion order; rates are decimal fractions per model period, levels use the node's units and ``seed`` is an exact ``int``.

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
    def time_series(
        historical: list[float], method: Literal["linear", "exponential", "moving_average"]
    ) -> ForecastSpec:
        """Forecast an external historical series with the chosen trend method.

        Tuning parameters (``alpha``, ``beta``, ``phi`` for ``"exponential"``;
        ``window`` for ``"moving_average"``) are set through
        ``ForecastSpec(ForecastMethod.time_series(), params)``.

        Parameters
        ----------
        historical:
            Oldest-first levels in the node's units; evaluation requires at least two observations.
        method:
            Trend method. There is no default.

        Returns
        -------
        ForecastSpec
            Time-series forecast configuration.

        Raises
        ------
        ValueError
            If ``method`` is not one of the three trend methods.

        Examples
        --------
        >>> from finstack_quant.statements import ForecastSpec
        >>> ForecastSpec.time_series([100.0, 105.0, 110.0], "linear").method.kind
        'time_series'
        """
        ...

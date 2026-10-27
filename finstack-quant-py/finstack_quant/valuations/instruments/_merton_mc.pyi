"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/merton_mc.rs``."""

from __future__ import annotations

from typing import Any, Literal
import pandas as pd
from finstack_quant.models.credit import DynamicRecoverySpec, EndogenousHazardSpec, MertonModel, ToggleExerciseModel

class BarrierCrossing:
    """
    Barrier-crossing detection policy for first-passage default simulation.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import BarrierCrossing
    >>> BarrierCrossing.brownian_bridge().to_json()
    '"brownian_bridge"'
    """

    @staticmethod
    def discrete() -> BarrierCrossing:
        """
        Discrete monitoring at simulation grid points.

        Returns
        -------
        BarrierCrossing
            Discrete policy: default is declared only when an asset value
            sampled on the simulation grid sits below the barrier. Fast, but it
            misses excursions between steps and so understates default risk on
            coarse grids. The default for terminal-barrier models.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import BarrierCrossing
        >>> BarrierCrossing.discrete().to_json()
        '"discrete"'
        """
        ...

    @staticmethod
    def brownian_bridge() -> BarrierCrossing:
        """
        Brownian-bridge correction for continuous monitoring.

        Returns
        -------
        BarrierCrossing
            Brownian-bridge policy: between two surviving grid points it draws
            against the analytic crossing probability, which removes the
            discretisation bias. The default for first-passage models and the
            more expensive of the two.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import BarrierCrossing
        >>> BarrierCrossing.brownian_bridge().to_json()
        '"brownian_bridge"'
        """
        ...

    @staticmethod
    def from_json(json: str) -> BarrierCrossing:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON-encoded barrier-crossing policy.

        Returns
        -------
        BarrierCrossing
            The decoded policy.

        Raises
        ------
        ValueError
            If ``json`` is not valid JSON for a barrier-crossing policy.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import BarrierCrossing
        >>> BarrierCrossing.from_json('"discrete"').to_json()
        '"discrete"'
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            JSON-encoded barrier-crossing policy.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def __repr__(self) -> str:
        """Return a Python-style constructor form of this value.

        Returns
        -------
        str
            ``BarrierCrossing.brownian_bridge()`` text.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            Reconstruction callable and canonical JSON arguments.

        Raises
        ------
        ValueError
            If serialization of the stored value fails.
        """
        ...

class MertonMcCalibrationSpec:
    """
    Market calibration of a Merton Monte Carlo configuration.

    Set on a configuration with :meth:`MertonMcConfig.calibration`. The
    ``merton_mc`` pricing model (``Bond.price(..., model="merton_mc")`` with
    the configuration in the bond's
    ``instrument_pricing_overrides.model_config.merton_mc_config``) first
    solves ``parameter`` by low-path bisection so the cash base-case price
    matches ``target``, then re-prices with full paths and reports the solved
    value as the ``calibrated_debt_barrier`` / ``calibrated_asset_vol``
    measures. ``Bond.price_merton_mc`` prices the configuration as given and
    does not calibrate.

    Parameters
    ----------
    target : dict[str, float] | str
        Market quote to match, as a ``BondQuoteInput`` dict or JSON string
        with one key: ``clean_price_pct`` (percent of par),
        ``dirty_price_currency``, ``ytm``, ``ytw``, ``z_spread``,
        ``discount_margin``, ``oas``, ``asw_market`` ... Spreads and
        yields are decimals (``0.02`` = 200bp).
    parameter : {"debt_barrier", "asset_vol"}
        Structural parameter solved for.
    low_paths : int, optional
        Independent MC estimators per calibration iteration; ``None`` uses
        the Rust default (2,000).
    max_iterations : int, optional
        Maximum bisection iterations; ``None`` uses the Rust default (40).
    tolerance_pv : float, optional
        Absolute tolerance on the PV residual in currency units at the
        valuation date; ``None`` uses the Rust default (``1e-4``).
    bracket : tuple[float, float], optional
        ``(low, high)`` search bracket for ``parameter``; ``None``
        auto-brackets from the parameter type.
    seed : int, optional
        Seed override for the calibration run; ``None`` uses the pricing
        seed.

    Raises
    ------
    ValueError
        If ``target`` is not a ``BondQuoteInput`` or ``parameter`` is not
        ``"debt_barrier"`` or ``"asset_vol"``.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import MertonMcCalibrationSpec
    >>> spec = MertonMcCalibrationSpec({"z_spread": 0.03}, "asset_vol", low_paths=500)
    >>> spec.parameter, spec.low_paths, spec.max_iterations, spec.target
    ('asset_vol', 500, 40, {'z_spread': 0.03})
    """

    def __init__(
        self,
        target: dict[str, float] | str,
        parameter: Literal["debt_barrier", "asset_vol"],
        low_paths: int | None = None,
        max_iterations: int | None = None,
        tolerance_pv: float | None = None,
        bracket: tuple[float, float] | None = None,
        seed: int | None = None,
    ) -> None:
        """
        Create a calibration specification; omitted settings take the Rust defaults.

        Parameters
        ----------
        target : dict[str, float] | str
            Market quote to match, as a ``BondQuoteInput`` dict or JSON string
            with one key: ``clean_price_pct`` (percent of par),
            ``dirty_price_currency``, ``ytm``, ``ytw``, ``z_spread``,
            ``discount_margin``, ``oas``, ``asw_market`` ... Spreads and
            yields are decimals (``0.02`` = 200bp).
        parameter : {"debt_barrier", "asset_vol"}
            Structural parameter solved for.
        low_paths : int, optional
            Independent MC estimators per calibration iteration; ``None`` uses
            the Rust default (2,000).
        max_iterations : int, optional
            Maximum bisection iterations; ``None`` uses the Rust default (40).
        tolerance_pv : float, optional
            Absolute tolerance on the PV residual in currency units at the
            valuation date; ``None`` uses the Rust default (``1e-4``).
        bracket : tuple[float, float], optional
            ``(low, high)`` search bracket for ``parameter``; ``None``
            auto-brackets from the parameter type.
        seed : int, optional
            Seed override for the calibration run; ``None`` uses the pricing
            seed.

        Raises
        ------
        ValueError
            If ``target`` is not a ``BondQuoteInput`` or ``parameter`` is not
            ``"debt_barrier"`` or ``"asset_vol"``.
        """
        ...

    @property
    def target(self) -> dict[str, float]:
        """
        Market quote to match as its ``BondQuoteInput`` dict.

        Returns
        -------
        dict[str, float]
            One-key dict such as ``{"z_spread": 0.03}``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def parameter(self) -> str:
        """
        Structural parameter solved for.

        Returns
        -------
        str
            ``"debt_barrier"`` or ``"asset_vol"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def low_paths(self) -> int:
        """
        Independent MC estimators per calibration iteration.

        Returns
        -------
        int
            The estimator count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def max_iterations(self) -> int:
        """
        Maximum bisection iterations.

        Returns
        -------
        int
            The iteration cap.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def tolerance_pv(self) -> float:
        """
        Absolute tolerance on the PV residual.

        Returns
        -------
        float
            Tolerance in currency units at the valuation date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def bracket(self) -> tuple[float, float] | None:
        """
        Search bracket for the calibrated parameter.

        Returns
        -------
        tuple[float, float] | None
            ``(low, high)``, or ``None`` to auto-bracket.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def seed(self) -> int | None:
        """
        Seed override for the calibration run.

        Returns
        -------
        int | None
            The seed, or ``None`` to use the pricing seed.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @staticmethod
    def from_json(json: str) -> MertonMcCalibrationSpec:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``MertonMcCalibrationSpec`` (the shape ``to_json`` writes).

        Returns
        -------
        MertonMcCalibrationSpec
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import MertonMcCalibrationSpec
        >>> try:
        ...     MertonMcCalibrationSpec.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            JSON-encoded ``MertonMcCalibrationSpec``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust value.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(MertonMcCalibrationSpec.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            A one-line summary of the value.
        """
        ...

class MertonMcConfig:
    """
    Configuration for Merton Monte Carlo PIK bond pricing.

    Carries the structural model, PIK schedule, recovery and time grid. The
    independent-estimator count, antithetic flag and seed label come from the
    bond's ``instrument_pricing_overrides.model_config`` (``mc_paths``,
    ``mc_antithetic``, ``mc_seed_scenario``).

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import MertonMcConfig, PikMode, PikSchedule
    >>> from finstack_quant.models.credit import MertonModel
    >>> config = MertonMcConfig(MertonModel(100.0, 0.25, 80.0, 0.04), 0.40).steps_per_year(24)
    >>> isinstance(config.pik_schedule(PikSchedule.uniform(PikMode.cash())), MertonMcConfig)
    True
    """

    def __init__(self, merton: MertonModel, recovery_rate: float) -> None:
        """
        Create a configuration with the registry-sourced time grid.

        Parameters
        ----------
        merton : MertonModel
            Structural credit model driving asset dynamics and default.
        recovery_rate : float
            Required recovery on default as a decimal fraction in ``[0, 1]``.

        Raises
        ------
        ValueError
            If ``recovery_rate`` is non-finite or outside ``[0, 1]``.
        """
        ...

    def pik_schedule(self, s: PikSchedule) -> MertonMcConfig:
        """
        Set the payment-in-kind schedule for the Merton MC config.

        Parameters
        ----------
        s : PikSchedule
            PIK schedule applied across coupon dates.

        Returns
        -------
        MertonMcConfig
            Updated configuration (fluent).

        Raises
        ------
        ValueError
            If ``r`` is non-finite or outside ``[0, 1]``.
        """
        ...

    def steps_per_year(self, n: int) -> MertonMcConfig:
        """
        Set simulation grid density.

        Parameters
        ----------
        n : int
            Time steps per year.

        Returns
        -------
        MertonMcConfig
            Updated configuration (fluent).

        Notes
        -----
        This method does not raise; it returns the same instance for chaining.
        """
        ...

    def barrier_crossing(self, p: BarrierCrossing) -> MertonMcConfig:
        """
        Set barrier-crossing policy for first-passage monitoring.

        Parameters
        ----------
        p : BarrierCrossing
            Discrete or Brownian-bridge policy.

        Returns
        -------
        MertonMcConfig
            Updated configuration (fluent).

        Notes
        -----
        This method does not raise; it returns the same instance for chaining.
        """
        ...

    def recovery_rate(self, r: float) -> MertonMcConfig:
        """
        Set flat recovery used when no dynamic recovery model is configured.

        Parameters
        ----------
        r : float
            Recovery rate as a decimal in ``[0, 1]``.

        Returns
        -------
        MertonMcConfig
            Updated configuration (fluent).

        Notes
        -----
        This method does not raise; it returns the same instance for chaining.
        """
        ...

    def endogenous_hazard(self, h: EndogenousHazardSpec) -> MertonMcConfig:
        """
        Set an endogenous hazard model.

        Parameters
        ----------
        h : EndogenousHazardSpec
            Endogenous hazard specification.

        Returns
        -------
        MertonMcConfig
            Updated configuration (fluent).

        Notes
        -----
        This method does not raise; it returns the same instance for chaining.
        """
        ...

    def dynamic_recovery(self, r: DynamicRecoverySpec) -> MertonMcConfig:
        """
        Set a dynamic recovery model.

        Parameters
        ----------
        r : DynamicRecoverySpec
            Dynamic recovery specification.

        Returns
        -------
        MertonMcConfig
            Updated configuration (fluent).

        Notes
        -----
        This method does not raise; it returns the same instance for chaining.
        """
        ...

    def toggle_model(self, t: ToggleExerciseModel) -> MertonMcConfig:
        """
        Set toggle exercise model for PIK/cash decisions.

        Parameters
        ----------
        t : ToggleExerciseModel
            Toggle exercise model.

        Returns
        -------
        MertonMcConfig
            Updated configuration (fluent).

        Notes
        -----
        This method does not raise; it returns the same instance for chaining.
        """
        ...

    def calibration(self, spec: MertonMcCalibrationSpec) -> MertonMcConfig:
        """
        Set the market-calibration specification.

        Honoured by the ``merton_mc`` pricing model (``Bond.price(...,
        model="merton_mc")`` with this configuration in the bond's
        ``instrument_pricing_overrides.model_config.merton_mc_config``), which
        solves the structural parameter to the target quote before the
        full-path price. ``Bond.price_merton_mc`` prices the configuration as
        given and does not calibrate.

        Parameters
        ----------
        spec : MertonMcCalibrationSpec
            Calibration target, solved parameter and solver settings.

        Returns
        -------
        MertonMcConfig
            A new configuration; the receiver is unchanged.

        Notes
        -----
        This method does not raise; it returns a new configuration.
        """
        ...

    def cashflow_dfs(self, dfs: list[tuple[float, float]]) -> MertonMcConfig:
        """
        Set term-structure discount factors for cashflow discounting.

        Cashflows are then discounted by log-linear interpolation of these
        factors instead of the flat ``discount_rate``; the flat rate still
        drives the risk-neutral asset drift. The ``merton_mc`` pricing model
        fills them from the discount curve when they are not set.

        Parameters
        ----------
        dfs : list[tuple[float, float]]
            ``(year_fraction, discount_factor)`` pairs sorted by time, with
            year fractions measured from the valuation date.

        Returns
        -------
        MertonMcConfig
            A new configuration; the receiver is unchanged.

        Raises
        ------
        TypeError
            If ``dfs`` is not a sequence of float pairs.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every configuration field as a plain ``dict``.

        Returns
        -------
        dict[str, Any]
            Canonical serde shape: ``merton``, ``pik_schedule``,
            ``steps_per_year``, ``barrier_crossing``, ``recovery_rate``,
            ``calibration``, ``cashflow_dfs`` and the optional hazard,
            recovery and toggle models.

        Raises
        ------
        ValueError
            If the configuration cannot be serialized.
        """
        ...

    @staticmethod
    def from_json(json: str) -> MertonMcConfig:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON-encoded configuration.

        Returns
        -------
        MertonMcConfig
            The decoded configuration.

        Raises
        ------
        ValueError
            If ``json`` is not valid JSON for a Merton MC configuration.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import MertonMcConfig
        >>> from finstack_quant.models.credit import MertonModel
        >>> config = MertonMcConfig(MertonModel(100.0, 0.25, 80.0, 0.04), 0.40).steps_per_year(24)
        >>> MertonMcConfig.from_json(config.to_json()).to_json() == config.to_json()
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            JSON-encoded configuration.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def __repr__(self) -> str:
        """Return a Python-style constructor form of this value.

        Returns
        -------
        str
            ``MertonMcConfig(steps_per_year=12, barrier_crossing=..., recovery_rate=0.4, ...)`` text.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            Reconstruction callable and canonical JSON arguments.

        Raises
        ------
        ValueError
            If serialization of the stored value fails.
        """
        ...

class MertonMcResult:
    """
    Result from Merton Monte Carlo PIK bond pricing.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.dates import StubKind
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.core.types import Rate
    >>> from finstack_quant.valuations.instruments import (
    ...     BarrierCrossing,
    ...     Bond,
    ...     MertonMcConfig,
    ...     PikMode,
    ...     PikSchedule,
    ... )
    >>> from finstack_quant.models.credit import MertonModel
    >>> config = (
    ...     MertonMcConfig(MertonModel(100.0, 0.25, 60.0, 0.04), 0.40)
    ...     .pik_schedule(PikSchedule.uniform(PikMode.pik()))
    ...     .barrier_crossing(BarrierCrossing.discrete())
    ... )
    >>> bond = Bond.fixed(
    ...     "PIK-1",
    ...     Money(100.0, Currency("USD")),
    ...     Rate(0.08),
    ...     datetime.date(2024, 1, 15),
    ...     datetime.date(2029, 1, 15),
    ...     StubKind.NONE,
    ...     "USD-OIS",
    ... )
    >>> bond.price_merton_mc(config, 0.04, datetime.date(2024, 1, 15)).clean_price_pct > 0.0
    True
    """

    @property
    def clean_price_pct(self) -> float:
        """
        Clean price as a percentage of par.

        Returns
        -------
        float
            :attr:`dirty_price_pct` less the accrued interest at the valuation
            date, as a percentage of par (``98.7`` means 98.7% of par).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def dirty_price_pct(self) -> float:
        """
        Dirty price as a percentage of par.

        Returns
        -------
        float
            Mean discounted path value divided by notional, times 100, at the
            valuation date. The simulation pays every remaining coupon in full,
            so this includes accrued interest; :attr:`clean_price_pct` removes it.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def expected_loss(self) -> float:
        """
        Expected loss as a fraction of PIK-aware risk-free PV.

        Returns
        -------
        float
            ``1 - mean_mc_pv / risk_free_pv``, so ``0.03`` is a 3% credit
            haircut. The benchmark PV accretes notional under the configured
            PIK schedule, and the value turns negative when the simulated PV
            exceeds that benchmark.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def unexpected_loss(self) -> float:
        """
        Unexpected loss (std dev of path PVs / notional).

        Returns
        -------
        float
            Dispersion of the loss distribution as a fraction of par, not a
            percentage: ``0.05`` here is comparable to 5 points on
            :attr:`clean_price_pct`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def expected_shortfall_95(self) -> float:
        """
        Expected shortfall at the 95% confidence level.

        Returns
        -------
        float
            Mean of the worst 5% of path PVs, expressed as a percentage of par
            like :attr:`clean_price_pct`. It is a price level rather than a
            loss, so lower is worse and it never exceeds the clean price.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def average_pik_fraction(self) -> float:
        """
        Average PIK fraction across coupon dates and paths.

        Returns
        -------
        float
            PIK elections divided by simulated coupon periods, in ``[0, 1]``.
            Counts whole elections, so a 50/50 split coupon still registers as
            one election. Identical to
            ``path_statistics.pik_exercise_rate``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def effective_spread_bp(self) -> float:
        """
        Effective spread in basis points versus risk-free PV.

        Returns
        -------
        float
            Constant continuous spread ``s`` solving ``risk_free_pv`` with each
            discount factor scaled by ``exp(-s * t)`` equal to the mean
            simulated PV. Solved on whichever discount basis priced the bond,
            so curve shape stays out of the spread.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def path_statistics(self) -> PathStatistics:
        """
        Path-level simulation statistics.

        Returns
        -------
        PathStatistics
            Default frequency, timing, recovery, and PIK-election diagnostics
            for the same run that produced these prices.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def num_paths(self) -> int:
        """
        Number of independent Monte Carlo estimators used.

        Returns
        -------
        int
            The resolved ``model_config.mc_paths`` count. With antithetic
            sampling each estimator simulates a mirrored pair, so twice as many
            paths are simulated.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def standard_error(self) -> float:
        """
        Standard error of the clean price (percentage of par).

        Returns
        -------
        float
            Sampling error of :attr:`clean_price_pct` in the same
            percent-of-par units, so 1.96 of these brackets a 95% interval
            either side of the price. With antithetic variates the estimate
            comes from pair averages, which keeps the negatively correlated
            legs from understating it.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the headline results as a single-row pandas ``DataFrame``.

        Returns
        -------
        pd.DataFrame
            Columns ``clean_price_pct``, ``dirty_price_pct``, ``expected_loss``,
            ``unexpected_loss``, ``expected_shortfall_95``,
            ``average_pik_fraction``, ``effective_spread_bp``, ``num_paths``,
            ``standard_error``, ``default_rate``, ``avg_default_time``,
            ``avg_terminal_notional``, ``avg_recovery_rate`` and
            ``pik_exercise_rate``.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def _repr_html_(self) -> str:
        """
        Jupyter rich display: the :meth:`to_dataframe` table as HTML.

        Returns
        -------
        str
            HTML table rendered by pandas.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

class PathStatistics:
    """
    Path-level statistics from a Merton Monte Carlo simulation.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.dates import StubKind
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.core.types import Rate
    >>> from finstack_quant.valuations.instruments import (
    ...     BarrierCrossing,
    ...     Bond,
    ...     MertonMcConfig,
    ...     PikMode,
    ...     PikSchedule,
    ... )
    >>> from finstack_quant.models.credit import MertonModel
    >>> config = (
    ...     MertonMcConfig(MertonModel(100.0, 0.25, 60.0, 0.04), 0.40)
    ...     .pik_schedule(PikSchedule.uniform(PikMode.pik()))
    ...     .barrier_crossing(BarrierCrossing.discrete())
    ... )
    >>> bond = Bond.fixed(
    ...     "PIK-1",
    ...     Money(100.0, Currency("USD")),
    ...     Rate(0.08),
    ...     datetime.date(2024, 1, 15),
    ...     datetime.date(2029, 1, 15),
    ...     StubKind.NONE,
    ...     "USD-OIS",
    ... )
    >>> 0.0 <= bond.price_merton_mc(config, 0.04, datetime.date(2024, 1, 15)).path_statistics.default_rate <= 1.0
    True
    """

    @property
    def default_rate(self) -> float:
        """
        Fraction of paths that defaulted.

        Returns
        -------
        float
            Defaulted paths divided by simulated paths, in ``[0, 1]``. This is
            a cumulative default probability to maturity, not an annualised
            hazard rate.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def avg_default_time(self) -> float:
        """
        Average default time in years among defaulted paths.

        Returns
        -------
        float
            Mean time from the valuation date to the barrier crossing,
            averaged over defaulted paths only. Exactly ``0.0`` when no path
            defaulted, so check :attr:`default_rate` before reading it.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def avg_terminal_notional(self) -> float:
        """
        Average terminal notional reflecting PIK accretion.

        Returns
        -------
        float
            Currency-unit notional at maturity averaged over surviving paths,
            so it exceeds the issued notional whenever coupons were PIKed.
            Falls back to the issued notional when every path defaulted.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def avg_recovery_rate(self) -> float:
        """
        Average recovery rate among defaulted paths.

        Returns
        -------
        float
            Decimal fraction: ``0.40`` means 40%
            recovery on the notional accreted up to the default time,
            averaged over defaulted paths. Exactly ``0.0`` when no path
            defaulted.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def pik_exercise_rate(self) -> float:
        """
        Fraction of coupon dates where PIK was elected.

        Returns
        -------
        float
            Same quantity as ``MertonMcResult.average_pik_fraction``, in
            ``[0, 1]``: a uniform cash schedule pins it to ``0.0`` and a
            uniform PIK schedule to ``1.0``, so only toggle and split
            schedules put it in between.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class PikMode:
    """
    Per-coupon PIK behavior for the Merton Monte Carlo engine.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import PikMode
    >>> PikMode.cash().to_json()
    '"cash"'
    """

    @staticmethod
    def cash() -> PikMode:
        """
        Coupon paid entirely in cash.

        Returns
        -------
        PikMode
            Cash mode, the default a schedule falls back to before its first
            step and whenever a toggle model is missing.

        Notes
        -----
        This factory does not raise; it returns a new instance with the documented defaults.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import PikMode
        >>> PikMode.cash().to_json()
        '"cash"'
        """
        ...

    @staticmethod
    def pik() -> PikMode:
        """
        Coupon accreted to notional.

        Returns
        -------
        PikMode
            Payment-in-kind mode: the coupon pays no cash and instead raises
            the notional that later coupons and recovery are computed on.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import PikMode
        >>> PikMode.pik().to_json()
        '"pik"'
        """
        ...

    @staticmethod
    def split(cash_fraction: float, pik_fraction: float) -> PikMode:
        """
        Coupon split between cash and PIK.

        Parameters
        ----------
        cash_fraction : float
            Fraction paid in cash as a decimal.
        pik_fraction : float
            Fraction accreted to notional as a decimal.

        Returns
        -------
        PikMode
            Split PIK mode. The two fractions must be non-negative and sum to
            one; the engine rejects anything else when the bond is priced, not
            when this mode is built.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import PikMode
        >>> PikMode.split(0.5, 0.5).to_json()
        '{"split":{"cash_fraction":0.5,"pik_fraction":0.5}}'
        """
        ...

    @staticmethod
    def toggle() -> PikMode:
        """
        Defer to the toggle exercise model on the config.

        Returns
        -------
        PikMode
            Toggle mode, which decides cash versus PIK per path from
            ``MertonMcConfig.toggle_model``. Without that model set the coupon
            silently falls back to cash.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import PikMode
        >>> PikMode.toggle().to_json()
        '"toggle"'
        """
        ...

    @staticmethod
    def from_json(json: str) -> PikMode:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON-encoded PIK mode.

        Returns
        -------
        PikMode
            The decoded mode.

        Raises
        ------
        ValueError
            If ``json`` is not valid JSON for a PIK mode.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import PikMode
        >>> PikMode.from_json('"pik"').to_json()
        '"pik"'
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            JSON-encoded PIK mode.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def __repr__(self) -> str:
        """Return a Python-style constructor form of this value.

        Returns
        -------
        str
            ``PikMode.split(cash_fraction=0.5, pik_fraction=0.5)`` text.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            Reconstruction callable and canonical JSON arguments.

        Raises
        ------
        ValueError
            If serialization of the stored value fails.
        """
        ...

class PikSchedule:
    """
    Time-varying PIK schedule for the Merton Monte Carlo engine.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import PikMode, PikSchedule
    >>> PikSchedule.uniform(PikMode.pik()).mode_at(1.0).to_json()
    '"pik"'
    """

    @staticmethod
    def uniform(mode: PikMode) -> PikSchedule:
        """
        Apply the same PIK mode at every coupon date.

        Parameters
        ----------
        mode : PikMode
            PIK mode applied uniformly.

        Returns
        -------
        PikSchedule
            Uniform schedule; every coupon date resolves to ``mode`` for the
            whole life of the bond.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import PikMode, PikSchedule
        >>> PikSchedule.uniform(PikMode.pik()).mode_at(1.0).to_json()
        '"pik"'
        """
        ...

    @staticmethod
    def stepped(steps: list[tuple[float, PikMode]]) -> PikSchedule:
        """
        Step-function PIK schedule keyed by year fraction.

        Parameters
        ----------
        steps : list[tuple[float, PikMode]]
            ``(year_fraction, mode)`` pairs sorted by time ascending.

        Returns
        -------
        PikSchedule
            Stepped schedule in which each entry stays in force from its year
            fraction until the next one. Coupons before the first step fall
            back to cash, so start the list at ``0.0`` to control them.

        Raises
        ------
        ValueError
            If ``steps`` cannot be parsed or fails validation at pricing time.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import PikMode, PikSchedule
        >>> schedule = PikSchedule.stepped([(0.0, PikMode.pik()), (2.0, PikMode.cash())])
        >>> schedule.mode_at(2.5).to_json()
        '"cash"'
        """
        ...

    def mode_at(self, t: float) -> PikMode:
        """
        Look up the active PIK mode at time ``t``.

        Parameters
        ----------
        t : float
            Time in years from the valuation date.

        Returns
        -------
        PikMode
            Active mode at ``t``.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    @staticmethod
    def from_json(json: str) -> PikSchedule:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON-encoded PIK schedule.

        Returns
        -------
        PikSchedule
            The decoded schedule.

        Raises
        ------
        ValueError
            If ``json`` is not valid JSON for a PIK schedule.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import PikMode, PikSchedule
        >>> PikSchedule.from_json('{"stepped":[[0.0,"pik"],[2.0,"cash"]]}').mode_at(0.5).to_json()
        '"pik"'
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            JSON-encoded PIK schedule.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def __repr__(self) -> str:
        """Return a Python-style constructor form of this value.

        Returns
        -------
        str
            ``PikSchedule.uniform(PikMode.pik())`` text.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            Reconstruction callable and canonical JSON arguments.

        Raises
        ------
        ValueError
            If serialization of the stored value fails.
        """
        ...

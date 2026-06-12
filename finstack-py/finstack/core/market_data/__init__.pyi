"""Market data bindings from ``finstack-core``: curves, FX, and market context.

Provides term-structure curve types (discount, forward, hazard, price,
inflation, volatility surfaces, volatility index), FX rate matrix, and the unified :class:`MarketContext`
container.

Example::

    >>> import datetime
    >>> from finstack.core.market_data import DiscountCurve
    >>> curve = DiscountCurve(
    ...     id="USD-OIS",
    ...     base_date=datetime.date(2024, 1, 1),
    ...     knots=[(0.25, 0.99), (0.5, 0.98), (1.0, 0.96)],
    ... )
    >>> curve.df(0.5)
    0.98
"""

from __future__ import annotations

import datetime
from typing import Optional, Union

from finstack.core.currency import Currency
from finstack.core.market_data import arbitrage as arbitrage
from finstack.core.market_data import dtsm as dtsm

__all__ = [
    # submodules
    "curves",
    "fx",
    "context",
    "dtsm",
    "arbitrage",
    # curves
    "BaseCorrelationCurve",
    "CreditIndexData",
    "DiscountCurve",
    "ForwardCurve",
    "FxDeltaVolSurface",
    "HazardCurve",
    "InflationCurve",
    "PriceCurve",
    "VolSurface",
    "VolCube",
    "VolatilityIndexCurve",
    # fx
    "FxConversionPolicy",
    "FxRateResult",
    "FxMatrix",
    # context
    "MarketContext",
]

# ---------------------------------------------------------------------------
# Curves
# ---------------------------------------------------------------------------

class DiscountCurve:
    """Discount factor curve for present-value calculations.

    Constructed from ``(time, discount_factor)`` knot pairs with configurable
    interpolation and extrapolation.

    Parameters
    ----------
    id : str
        Unique curve identifier (e.g. ``"USD-OIS"``).
    base_date : datetime.date
        Valuation date.
    knots : list[tuple[float, float]]
        ``(time_years, discount_factor)`` pairs.
    interp : str
        Interpolation style (default ``"monotone_convex"``).
    extrapolation : str
        Extrapolation policy (default ``"flat_forward"``).
    day_count : str | None
        Day-count convention. When omitted, Rust infers a market default from the curve ID.

    Raises
    ------
    ValueError
        If the curve cannot be built from the given parameters.

    Examples
    --------
    >>> import datetime
    >>> from finstack.core.market_data import DiscountCurve
    >>> dc = DiscountCurve(
    ...     id="USD-OIS",
    ...     base_date=datetime.date(2024, 1, 1),
    ...     knots=[(0.0, 1.0), (1.0, 0.96), (5.0, 0.82)],
    ... )
    >>> dc.df(1.0)
    0.96
    >>> dc.zero(1.0)  # continuously-compounded zero rate
    0.040821994520255166
    """

    def __init__(
        self,
        id: str,
        base_date: datetime.date,
        knots: list[tuple[float, float]],
        interp: str = "monotone_convex",
        extrapolation: str = "flat_forward",
        day_count: Optional[str] = None,
    ) -> None:
        """Construct a discount curve from knot points.

        Parameters
        ----------
        id : str
            Unique curve identifier.
        base_date : datetime.date
            Valuation date.
        knots : list[tuple[float, float]]
            ``(time_years, discount_factor)`` pairs.
        interp : str
            Interpolation style (default ``"monotone_convex"``).
        extrapolation : str
            Extrapolation policy (default ``"flat_forward"``).
        day_count : str | None
            Day-count convention. When omitted, Rust infers a market default from the curve ID.

        Raises
        ------
        ValueError
            If the curve cannot be built.
        """
        ...

    def df(self, t: float) -> float:
        """Discount factor at year fraction *t*.

        Parameters
        ----------
        t : float
            Time in year fractions from the base date.

        Returns
        -------
        float
            Discount factor.
        """
        ...

    def zero(self, t: float) -> float:
        """Continuously-compounded zero rate at year fraction *t*.

        Parameters
        ----------
        t : float
            Time in year fractions.

        Returns
        -------
        float
            Zero rate.
        """
        ...

    def forward(self, t1: float, t2: float) -> float:
        """Continuously-compounded forward rate between *t1* and *t2*.

        Parameters
        ----------
        t1 : float
            Start time in year fractions.
        t2 : float
            End time in year fractions.

        Returns
        -------
        float
            Forward rate.

        Raises
        ------
        ValueError
            If *t1* >= *t2*.
        """
        ...

    @property
    def id(self) -> str:
        """Curve identifier string.

        Returns
        -------
        str
        """
        ...

    @property
    def base_date(self) -> datetime.date:
        """Valuation base date.

        Returns
        -------
        datetime.date
        """
        ...

    def __repr__(self) -> str: ...

class ForwardCurve:
    """Forward rate curve for a floating-rate index with a fixed tenor.

    Constructed from ``(time, forward_rate)`` knot pairs.

    Parameters
    ----------
    id : str
        Unique curve identifier (e.g. ``"USD-SOFR-3M"``).
    tenor : float
        Index tenor in years (e.g. ``0.25`` for 3 months).
    knots : list[tuple[float, float]]
        ``(time_years, forward_rate)`` pairs.
    base_date : datetime.date
        Valuation date.
    day_count : str, optional
        Day-count convention. When omitted, Rust infers a market default from the curve ID.
    interp : str
        Interpolation style (default ``"linear"``).
    extrapolation : str
        Extrapolation policy (default ``"flat_forward"``).

    Raises
    ------
    ValueError
        If the curve cannot be built from the given parameters.
    """

    def __init__(
        self,
        id: str,
        tenor: float,
        knots: list[tuple[float, float]],
        base_date: datetime.date,
        day_count: str | None = None,
        interp: str = "linear",
        extrapolation: str = "flat_forward",
    ) -> None:
        """Construct a forward rate curve from knot points.

        Parameters
        ----------
        id : str
            Unique curve identifier.
        tenor : float
            Index tenor in years.
        knots : list[tuple[float, float]]
            ``(time_years, forward_rate)`` pairs.
        base_date : datetime.date
            Valuation date.
        day_count : str, optional
            Day-count convention. When omitted, Rust infers a market default from the curve ID.
        interp : str
            Interpolation style (default ``"linear"``).
        extrapolation : str
            Extrapolation policy (default ``"flat_forward"``).

        Raises
        ------
        ValueError
            If the curve cannot be built.
        """
        ...

    def rate(self, t: float) -> float:
        """Forward rate at year fraction *t*.

        Parameters
        ----------
        t : float
            Time in year fractions.

        Returns
        -------
        float
            Forward rate.
        """
        ...

    @property
    def id(self) -> str:
        """Curve identifier string.

        Returns
        -------
        str
        """
        ...

    @property
    def base_date(self) -> datetime.date:
        """Valuation base date.

        Returns
        -------
        datetime.date
        """
        ...

    def __repr__(self) -> str: ...

class HazardCurve:
    """Credit hazard-rate curve for default probability modeling.

    Constructed from ``(time, hazard_rate)`` knot pairs.

    Parameters
    ----------
    id : str
        Unique curve identifier (e.g. ``"ACME-HZD"``).
    base_date : datetime.date
        Valuation date.
    knots : list[tuple[float, float]]
        ``(time_years, hazard_rate)`` pairs.
    recovery_rate : float
        Recovery rate. Defaults to the credit assumptions registry value.
    day_count : str
        Day-count convention (default ``"act_365f"``).
    par_spreads : list[tuple[float, float]] | None
        Market par-spread quotes in basis points used for rebootstrap risks.

    Raises
    ------
    ValueError
        If the curve cannot be built from the given parameters.
    """

    def __init__(
        self,
        id: str,
        base_date: datetime.date,
        knots: list[tuple[float, float]],
        recovery_rate: float | None = None,
        day_count: str = "act_365f",
        par_spreads: list[tuple[float, float]] | None = None,
    ) -> None:
        """Construct a hazard curve from knot points.

        Parameters
        ----------
        id : str
            Unique curve identifier.
        base_date : datetime.date
            Valuation date.
        knots : list[tuple[float, float]]
            ``(time_years, hazard_rate)`` pairs.
        recovery_rate : float
            Recovery rate (default ``0.4``).
        day_count : str
            Day-count convention (default ``"act_365f"``).
        par_spreads : list[tuple[float, float]] | None
            Market par-spread quotes in basis points used for rebootstrap risks.

        Raises
        ------
        ValueError
            If the curve cannot be built.
        """
        ...

    def sp(self, t: float) -> float:
        """Survival probability at year fraction *t*.

        Parameters
        ----------
        t : float
            Time in year fractions.

        Returns
        -------
        float
            Survival probability in ``[0, 1]``.
        """
        ...

    def hazard_rate(self, t: float) -> float:
        """Instantaneous hazard rate at year fraction *t*.

        Parameters
        ----------
        t : float
            Time in year fractions.

        Returns
        -------
        float
            Hazard rate.
        """
        ...

    @property
    def id(self) -> str:
        """Curve identifier string.

        Returns
        -------
        str
        """
        ...

    @property
    def base_date(self) -> datetime.date:
        """Valuation base date.

        Returns
        -------
        datetime.date
        """
        ...

    def __repr__(self) -> str: ...

class BaseCorrelationCurve:
    """Base-correlation curve for synthetic credit index tranche pricing."""

    def __init__(self, id: str, knots: list[tuple[float, float]]) -> None:
        """Construct a base-correlation curve from knot points."""
        ...

    @property
    def id(self) -> str:
        """Curve identifier string."""
        ...

    def correlation(self, detachment_pct: float) -> float:
        """Interpolated base correlation at a detachment point."""
        ...

    def __repr__(self) -> str: ...

class CreditIndexData:
    """Credit index data bundle for synthetic tranche pricing."""

    def __init__(
        self,
        num_constituents: int,
        recovery_rate: float,
        index_credit_curve: HazardCurve,
        base_correlation_curve: BaseCorrelationCurve,
    ) -> None:
        """Construct homogeneous credit index data."""
        ...

    @property
    def num_constituents(self) -> int:
        """Number of constituents in the index."""
        ...

    @property
    def recovery_rate(self) -> float:
        """Index recovery rate."""
        ...

    def __repr__(self) -> str: ...

class PriceCurve:
    """Forward price curve for commodities and other price-based assets.

    Constructed from ``(time, forward_price)`` knot pairs.

    Parameters
    ----------
    id : str
        Unique curve identifier (e.g. ``"WTI-FORWARD"``).
    base_date : datetime.date
        Valuation date.
    knots : list[tuple[float, float]]
        ``(time_years, forward_price)`` pairs.
    extrapolation : str
        Extrapolation policy (default ``"flat_zero"``).
    interp : str
        Interpolation style (default ``"linear"``).
    day_count : str
        Day-count convention (default ``"act_365f"``).

    Raises
    ------
    ValueError
        If the curve cannot be built from the given parameters.
    """

    def __init__(
        self,
        id: str,
        base_date: datetime.date,
        knots: list[tuple[float, float]],
        extrapolation: str = "flat_zero",
        interp: str = "linear",
        day_count: str = "act_365f",
    ) -> None:
        """Construct a price curve from knot points.

        Parameters
        ----------
        id : str
            Unique curve identifier.
        base_date : datetime.date
            Valuation date.
        knots : list[tuple[float, float]]
            ``(time_years, forward_price)`` pairs.
        extrapolation : str
            Extrapolation policy (default ``"flat_zero"``).
        interp : str
            Interpolation style (default ``"linear"``).
        day_count : str
            Day-count convention (default ``"act_365f"``).

        Raises
        ------
        ValueError
            If the curve cannot be built.
        """
        ...

    def price(self, t: float) -> float:
        """Forward price at year fraction *t*.

        Parameters
        ----------
        t : float
            Time in year fractions.

        Returns
        -------
        float
            Forward price.
        """
        ...

    @property
    def id(self) -> str:
        """Curve identifier string.

        Returns
        -------
        str
        """
        ...

    @property
    def base_date(self) -> datetime.date:
        """Valuation base date.

        Returns
        -------
        datetime.date
        """
        ...

    def __repr__(self) -> str: ...

class InflationCurve:
    """CPI inflation curve for inflation-linked pricing and breakeven analysis.

    Constructed from ``(time, cpi_level)`` knot pairs.

    Parameters
    ----------
    id : str
        Unique curve identifier (e.g. ``"US-CPI"``).
    base_date : datetime.date
        Valuation date.
    base_cpi : float
        CPI level at ``t = 0``.
    knots : list[tuple[float, float]]
        ``(time_years, cpi_level)`` pairs.
    day_count : str
        Day-count convention (default ``"act_365f"``).
    indexation_lag_months : int
        Observation lag in months (default ``3``).
    interp : str
        Interpolation style (default ``"log_linear"``).

    Raises
    ------
    ValueError
        If the curve cannot be built from the given parameters.
    """

    def __init__(
        self,
        id: str,
        base_date: datetime.date,
        base_cpi: float,
        knots: list[tuple[float, float]],
        day_count: str = "act_365f",
        indexation_lag_months: int = 3,
        interp: str = "log_linear",
    ) -> None: ...
    def cpi(self, t: float) -> float:
        """CPI level at year fraction *t*, without indexation lag."""
        ...

    def cpi_with_lag(self, t: float) -> float:
        """CPI level at year fraction *t*, with indexation lag applied."""
        ...

    def inflation_rate(self, t1: float, t2: float) -> float:
        """Annualized inflation rate between *t1* and *t2* using CAGR."""
        ...

    def inflation_rate_simple(self, t1: float, t2: float) -> float:
        """Simple non-compounded inflation rate between *t1* and *t2*."""
        ...

    @property
    def id(self) -> str: ...
    @property
    def base_date(self) -> datetime.date: ...
    @property
    def day_count(self) -> str: ...
    @property
    def indexation_lag_months(self) -> int: ...
    @property
    def base_cpi(self) -> float: ...
    def __repr__(self) -> str: ...

class VolSurface:
    """Two-dimensional implied volatility surface on an expiry x strike grid.

    Parameters
    ----------
    id : str
        Unique surface identifier.
    expiries : list[float]
        Expiry axis in years.
    strikes : list[float]
        Strike axis.
    vols_row_major : list[float]
        Flat row-major volatility values of length ``len(expiries) * len(strikes)``.
    secondary_axis : str
        Semantic meaning of the second axis: ``"strike"`` or ``"tenor"``.
    interpolation_mode : str
        Interpolation contract: ``"vol"`` or ``"total_variance"``.
    quote_type : str
        Quoting convention: ``"black_lognormal"`` or ``"normal"``.

    Raises
    ------
    ValueError
        If the surface cannot be built from the given parameters.
    """

    def __init__(
        self,
        id: str,
        expiries: list[float],
        strikes: list[float],
        vols_row_major: list[float],
        secondary_axis: str = "strike",
        interpolation_mode: str = "vol",
        quote_type: str = "black_lognormal",
    ) -> None: ...
    def value_checked(self, expiry: float, strike: float) -> float:
        """Interpolated surface value with explicit bounds checking."""
        ...

    def value_clamped(self, expiry: float, strike: float) -> float:
        """Interpolated surface value with flat extrapolation at the edges."""
        ...

    @property
    def id(self) -> str: ...
    @property
    def expiries(self) -> list[float]: ...
    @property
    def strikes(self) -> list[float]: ...
    @property
    def secondary_axis(self) -> str: ...
    @property
    def quote_type(self) -> str: ...
    @property
    def interpolation_mode(self) -> str: ...
    @property
    def grid_shape(self) -> tuple[int, int]: ...
    def __repr__(self) -> str: ...

class FxDeltaVolSurface:
    """FX vol surface in delta space (ATM, 25-d RR/BF, optional 10-d wings).

    Forward delta (premium-unadjusted). Strike conversion uses Garman-Kohlhagen.
    See ``docs/REFERENCES.md#clark-fx-options`` and ``#wystup-fx-options``.
    """

    def __init__(
        self,
        id: str,
        expiries: list[float],
        atm_vols: list[float],
        rr_25d: list[float],
        bf_25d: list[float],
        rr_10d: list[float] | None = None,
        bf_10d: list[float] | None = None,
    ) -> None:
        """Build a delta-quoted FX vol surface.

        Parameters
        ----------
        id
            Unique surface identifier.
        expiries
            Strictly increasing positive expiry times (years).
        atm_vols
            ATM delta-neutral straddle vols per expiry (positive).
        rr_25d
            25-delta risk reversal per expiry (call vol − put vol).
        bf_25d
            25-delta butterfly per expiry (wing average − ATM).
        rr_10d, bf_10d
            Optional 10-delta wings; both required when either is set.

        Raises
        ------
        ValueError
            Invalid inputs or mismatched ``rr_10d`` / ``bf_10d``.
        """
        ...
    @property
    def id(self) -> str: ...
    @property
    def expiries(self) -> list[float]: ...
    @property
    def num_expiries(self) -> int: ...
    def pillar_vols(self, expiry_idx: int) -> tuple[float, float, float]:
        """Pillar vols at ``expiry_idx`` as ``(atm, put_25d_vol, call_25d_vol)``.

        Raises
        ------
        IndexError
            If ``expiry_idx`` is out of range.
        """
        ...

    def implied_vol(
        self,
        expiry: float,
        strike: float,
        forward: float,
        r_d: float,
        r_f: float,
    ) -> float:
        """Interpolated implied vol at the given ``(expiry, strike)``."""
        ...

    def to_vol_surface(self, spot: float, r_d: float, r_f: float) -> VolSurface:
        """Materialize this delta-quoted surface as a strike-axis :class:`VolSurface`."""
        ...

    @staticmethod
    def delta_to_strike(delta: float, forward: float, vol: float, expiry: float, r_f: float) -> float:
        """Convert a forward delta to a strike (Garman-Kohlhagen, premium-unadjusted)."""
        ...

    @staticmethod
    def strike_to_delta(strike: float, forward: float, vol: float, expiry: float, r_f: float) -> float:
        """Convert a strike to forward call delta."""
        ...

    def __repr__(self) -> str: ...

class VolCube:
    """SABR volatility cube on an expiry x tenor grid.

    Stores calibrated SABR parameters at each (expiry, tenor) node and
    evaluates implied volatilities via bilinear parameter interpolation
    and the Hagan (2002) approximation.

    Parameters
    ----------
    id : str
        Unique cube identifier.
    expiries : list[float]
        Option expiry axis in years.
    tenors : list[float]
        Underlying swap tenor axis in years.
    params_row_major : list[dict[str, float]]
        SABR parameter dicts with keys ``"alpha"``, ``"beta"``, ``"rho"``,
        ``"nu"``, and optionally ``"shift"``.
    forwards_row_major : list[float]
        Forward rates in row-major order
        (length ``len(expiries) * len(tenors)``).
    interpolation_mode : str
        Interpolation contract: ``"vol"`` or ``"total_variance"``
        (default ``"vol"``).

    Raises
    ------
    ValueError
        If the cube cannot be built from the given parameters.
    """

    def __init__(
        self,
        id: str,
        expiries: list[float],
        tenors: list[float],
        params_row_major: list[dict[str, float]],
        forwards_row_major: list[float],
        interpolation_mode: str = "vol",
    ) -> None: ...
    def vol(self, expiry: float, tenor: float, strike: float) -> float:
        """Implied volatility with bounds checking.

        Parameters
        ----------
        expiry : float
            Option expiry in years.
        tenor : float
            Underlying swap tenor in years.
        strike : float
            Strike rate.

        Returns
        -------
        float
            Black-76 implied volatility.

        Raises
        ------
        ValueError
            If expiry or tenor falls outside the grid.
        """
        ...

    def vol_clamped(self, expiry: float, tenor: float, strike: float) -> float:
        """Implied volatility with clamped extrapolation at the grid edges."""
        ...

    def vol_normal(self, expiry: float, tenor: float, strike: float) -> float:
        """Normal (Bachelier) implied volatility with bounds checking.

        Parameters
        ----------
        expiry : float
            Option expiry in years.
        tenor : float
            Underlying swap tenor in years.
        strike : float
            Strike rate.

        Returns
        -------
        float
            Normal (Bachelier) implied volatility in absolute rate units
            (e.g. ``0.008`` = 80 bp/yr).

        Raises
        ------
        ValueError
            If expiry or tenor falls outside the grid, if the expansion
            yields a non-finite volatility, or for cross-zero quotes
            (``(F+s)(K+s) <= 0``) with ``beta > 0``, which require an
            explicit shift.
        """
        ...

    def vol_normal_clamped(self, expiry: float, tenor: float, strike: float) -> float:
        """Normal (Bachelier) implied volatility with clamped extrapolation.

        Never raises and never returns a non-finite or non-positive value;
        degenerate expansions are floored to a small positive normal vol.
        """
        ...

    def materialize_tenor_slice(self, tenor: float, strikes: list[float]) -> VolSurface:
        """Materialize a tenor slice as a :class:`VolSurface`.

        Parameters
        ----------
        tenor : float
            Tenor to slice at (years).
        strikes : list[float]
            Strike axis for the resulting surface.

        Returns
        -------
        VolSurface
        """
        ...

    def materialize_tenor_slice_normal(self, tenor: float, strikes: list[float]) -> VolSurface:
        """Materialize a tenor slice as a normal-vol (Bachelier) :class:`VolSurface`.

        Vols are in absolute rate units and the resulting surface is tagged
        with the normal quote type.

        Parameters
        ----------
        tenor : float
            Tenor to slice at (years).
        strikes : list[float]
            Strike axis for the resulting surface.

        Returns
        -------
        VolSurface
        """
        ...

    def materialize_expiry_slice(self, expiry: float, strikes: list[float]) -> VolSurface:
        """Materialize an expiry slice as a :class:`VolSurface`.

        Parameters
        ----------
        expiry : float
            Expiry to slice at (years).
        strikes : list[float]
            Strike axis for the resulting surface.

        Returns
        -------
        VolSurface
        """
        ...

    def materialize_expiry_slice_normal(self, expiry: float, strikes: list[float]) -> VolSurface:
        """Materialize an expiry slice as a normal-vol (Bachelier) :class:`VolSurface`.

        Vols are in absolute rate units and the resulting surface is tagged
        with the normal quote type.

        Parameters
        ----------
        expiry : float
            Expiry to slice at (years).
        strikes : list[float]
            Strike axis for the resulting surface.

        Returns
        -------
        VolSurface
        """
        ...

    @property
    def id(self) -> str: ...
    @property
    def expiries(self) -> list[float]: ...
    @property
    def tenors(self) -> list[float]: ...
    @property
    def grid_shape(self) -> tuple[int, int]: ...
    def __repr__(self) -> str: ...

class VolatilityIndexCurve:
    """Volatility index forward curve (e.g. VIX term structure).

    Constructed from ``(time, forward_level)`` knot pairs.

    Parameters
    ----------
    id : str
        Unique curve identifier (e.g. ``"VIX"``).
    base_date : datetime.date
        Valuation date.
    knots : list[tuple[float, float]]
        ``(time_years, forward_level)`` pairs.
    extrapolation : str
        Extrapolation policy (default ``"flat_zero"``).
    interp : str
        Interpolation style (default ``"linear"``).
    day_count : str
        Day-count convention (default ``"act_365f"``).

    Raises
    ------
    ValueError
        If the curve cannot be built from the given parameters.
    """

    def __init__(
        self,
        id: str,
        base_date: datetime.date,
        knots: list[tuple[float, float]],
        extrapolation: str = "flat_zero",
        interp: str = "linear",
        day_count: str = "act_365f",
    ) -> None:
        """Construct a volatility index curve from knot points.

        Parameters
        ----------
        id : str
            Unique curve identifier.
        base_date : datetime.date
            Valuation date.
        knots : list[tuple[float, float]]
            ``(time_years, forward_level)`` pairs.
        extrapolation : str
            Extrapolation policy (default ``"flat_zero"``).
        interp : str
            Interpolation style (default ``"linear"``).
        day_count : str
            Day-count convention (default ``"act_365f"``).

        Raises
        ------
        ValueError
            If the curve cannot be built.
        """
        ...

    def forward_level(self, t: float) -> float:
        """Forward volatility index level at year fraction *t*.

        Parameters
        ----------
        t : float
            Time in year fractions.

        Returns
        -------
        float
            Forward volatility index level.
        """
        ...

    @property
    def id(self) -> str:
        """Curve identifier string.

        Returns
        -------
        str
        """
        ...

    @property
    def base_date(self) -> datetime.date:
        """Valuation base date.

        Returns
        -------
        datetime.date
        """
        ...

    def __repr__(self) -> str: ...

# ---------------------------------------------------------------------------
# FX
# ---------------------------------------------------------------------------

class FxConversionPolicy:
    """FX conversion policy controlling when rates are sampled.

    Immutable enum-style type with class-level constants.
    """

    CASHFLOW_DATE: FxConversionPolicy
    """Use spot/forward on the cashflow date."""
    PERIOD_END: FxConversionPolicy
    """Use period end date."""
    PERIOD_AVERAGE: FxConversionPolicy
    """Use an average over the period."""
    CUSTOM: FxConversionPolicy
    """Custom strategy defined by the caller."""

    @classmethod
    def from_name(cls, name: str) -> FxConversionPolicy:
        """Parse from a string label.

        Parameters
        ----------
        name : str
            Policy label (e.g. ``"cashflow_date"``, ``"period_end"``).

        Returns
        -------
        FxConversionPolicy

        Raises
        ------
        ValueError
            If *name* is not recognised.
        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class FxRateResult:
    """Result of an FX rate query.

    Immutable value type returned by :meth:`FxMatrix.rate`.
    """

    @property
    def rate(self) -> float:
        """The FX conversion rate.

        Returns
        -------
        float
        """
        ...

    @property
    def triangulated(self) -> bool:
        """Whether the rate was obtained via triangulation.

        Returns
        -------
        bool
        """
        ...

    def __repr__(self) -> str: ...

class FxMatrix:
    """Foreign-exchange rate matrix for currency conversion.

    Manages explicit FX quotes and supports rate lookup with optional
    triangulation.
    """

    def __init__(self) -> None:
        """Create an empty FX matrix."""
        ...

    def set_quote(
        self,
        base: Union[Currency, str],
        quote: Union[Currency, str],
        rate: float,
    ) -> None:
        """Set an explicit FX quote.

        Parameters
        ----------
        base : Currency | str
            Base (from) currency.
        quote : Currency | str
            Quote (to) currency.
        rate : float
            The conversion rate (``1 base = rate quote``).

        Raises
        ------
        ValueError
            If a currency code is invalid or rate is non-finite.
        """
        ...

    def rate(
        self,
        base: Union[Currency, str],
        quote: Union[Currency, str],
        date: datetime.date,
        policy: Optional[Union[FxConversionPolicy, str]] = None,
    ) -> FxRateResult:
        """Look up an FX rate.

        Parameters
        ----------
        base : Currency | str
            Base (from) currency.
        quote : Currency | str
            Quote (to) currency.
        date : datetime.date
            Applicable date for the rate.
        policy : FxConversionPolicy | str | None
            Conversion policy (default ``"cashflow_date"``).

        Returns
        -------
        FxRateResult
            The looked-up rate with metadata.

        Raises
        ------
        KeyError
            If no rate is available for the requested pair.
        ValueError
            If the rate cannot be determined for another reason
            (e.g. invalid or non-finite quotes).
        """
        ...

    def __repr__(self) -> str: ...

# ---------------------------------------------------------------------------
# Market context
# ---------------------------------------------------------------------------

class MarketContext:
    """Unified market data container for curves, surfaces, and FX.

    Provides a single access point for all market data required by
    pricing and analytics functions. Curves are stored behind ``Arc``
    and the context is cheap to clone.
    """

    def __init__(self) -> None:
        """Create an empty market context."""
        ...

    def insert(
        self,
        curve: Union[
            DiscountCurve,
            ForwardCurve,
            HazardCurve,
            InflationCurve,
            PriceCurve,
            VolSurface,
            FxDeltaVolSurface,
            VolCube,
            VolatilityIndexCurve,
        ],
    ) -> MarketContext:
        """Insert a curve into the context (fluent, returns ``self``).

        Accepts any curve type: :class:`DiscountCurve`, :class:`ForwardCurve`,
        :class:`HazardCurve`, :class:`InflationCurve`, :class:`PriceCurve`,
        :class:`VolSurface`, :class:`FxDeltaVolSurface`, :class:`VolCube`,
        or :class:`VolatilityIndexCurve`.

        Parameters
        ----------
        curve : DiscountCurve | ForwardCurve | HazardCurve | InflationCurve | PriceCurve | VolSurface | FxDeltaVolSurface | VolCube | VolatilityIndexCurve
            The curve to insert.

        Returns
        -------
        MarketContext
            ``self`` for method chaining.

        Raises
        ------
        TypeError
            If *curve* is not a supported curve type.
        """
        ...

    def insert_fx(self, fx: FxMatrix) -> None:
        """Insert an FX matrix into the context.

        Parameters
        ----------
        fx : FxMatrix
            FX rate matrix.
        """
        ...

    def insert_price(
        self,
        id: str,
        value: float,
        currency: str | None = None,
    ) -> None:
        """Insert a scalar market price into the context.

        Parameters
        ----------
        id : str
            Market scalar identifier.
        value : float
            Unitless scalar value or monetary amount.
        currency : str | None, optional
            Currency for monetary prices. If omitted, stores a unitless scalar.
        """
        ...

    def insert_credit_index(self, id: str, data: CreditIndexData) -> None:
        """Insert credit index data into the context.

        Parameters
        ----------
        id : str
            Credit index identifier.
        data : CreditIndexData
            Credit index data bundle.
        """
        ...

    def get_discount(self, id: str) -> DiscountCurve:
        """Retrieve a discount curve by identifier.

        Parameters
        ----------
        id : str
            Curve identifier.

        Returns
        -------
        DiscountCurve

        Raises
        ------
        KeyError
            If no discount curve with this *id* exists.
        """
        ...

    def get_forward(self, id: str) -> ForwardCurve:
        """Retrieve a forward curve by identifier.

        Parameters
        ----------
        id : str
            Curve identifier.

        Returns
        -------
        ForwardCurve

        Raises
        ------
        KeyError
            If no forward curve with this *id* exists.
        """
        ...

    def get_hazard(self, id: str) -> HazardCurve:
        """Retrieve a hazard curve by identifier.

        Parameters
        ----------
        id : str
            Curve identifier.

        Returns
        -------
        HazardCurve

        Raises
        ------
        KeyError
            If no hazard curve with this *id* exists.
        """
        ...

    def get_inflation_curve(self, id: str) -> InflationCurve:
        """Retrieve an inflation curve by identifier.

        Parameters
        ----------
        id : str
            Curve identifier.

        Returns
        -------
        InflationCurve

        Raises
        ------
        KeyError
            If no inflation curve with this *id* exists.
        """
        ...

    def get_price_curve(self, id: str) -> PriceCurve:
        """Retrieve a price curve by identifier.

        Parameters
        ----------
        id : str
            Curve identifier.

        Returns
        -------
        PriceCurve

        Raises
        ------
        KeyError
            If no price curve with this *id* exists.
        """
        ...

    def get_surface(self, id: str) -> VolSurface:
        """Retrieve a vol surface by identifier.

        Parameters
        ----------
        id : str
            Surface identifier.

        Returns
        -------
        VolSurface

        Raises
        ------
        KeyError
            If no surface with this *id* exists.
        """
        ...

    def get_fx_delta_vol_surface(self, id: str) -> FxDeltaVolSurface:
        """Retrieve a delta-quoted FX vol surface by identifier.

        Parameters
        ----------
        id : str
            Surface identifier.

        Returns
        -------
        FxDeltaVolSurface

        Raises
        ------
        KeyError
            If no delta-quoted FX surface with this *id* exists.
        """
        ...

    def get_vol_cube(self, id: str) -> VolCube:
        """Retrieve a vol cube by identifier.

        Parameters
        ----------
        id : str
            Cube identifier.

        Returns
        -------
        VolCube

        Raises
        ------
        KeyError
            If no vol cube with this *id* exists.
        """
        ...

    def get_vol_index_curve(self, id: str) -> VolatilityIndexCurve:
        """Retrieve a volatility index curve by identifier.

        Parameters
        ----------
        id : str
            Curve identifier.

        Returns
        -------
        VolatilityIndexCurve

        Raises
        ------
        KeyError
            If no vol-index curve with this *id* exists.
        """
        ...

    @property
    def fx(self) -> Optional[FxMatrix]:
        """Access the FX matrix (returns ``None`` if not set).

        Returns
        -------
        FxMatrix | None
        """
        ...

    @staticmethod
    def from_json(json: str) -> MarketContext:
        """Deserialize a market context from a JSON string.

        Accepts the same JSON format produced by :meth:`to_json` and by the
        calibration and pricing pipelines.

        Parameters
        ----------
        json : str
            JSON-serialized ``MarketContext``.

        Returns
        -------
        MarketContext
            Deserialized market context.

        Raises
        ------
        ValueError
            If the JSON is not a valid market context.
        """
        ...

    def to_json(self) -> str:
        """Serialize this context to pretty-printed JSON (compatible with pricing APIs).

        Returns
        -------
        str
            JSON string accepted by ``price_instrument`` / ``price_instrument_with_metrics``.
        """
        ...

    def __repr__(self) -> str: ...

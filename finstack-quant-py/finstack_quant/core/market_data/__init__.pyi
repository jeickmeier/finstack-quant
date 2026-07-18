"""
Market data bindings from ``finstack-quant-core``: curves, FX, and market context.

Provides term-structure curve types (discount, forward, hazard, price,
inflation, volatility surfaces, volatility index), FX rate matrix, and the unified :class:`MarketContext`
container.

Example::

    >>> import datetime
    >>> from finstack_quant.core.market_data import DiscountCurve
    >>> curve = DiscountCurve(
    ...     id="USD-OIS",
    ...     base_date=datetime.date(2024, 1, 1),
    ...     knots=[(0.25, 0.99), (0.5, 0.98), (1.0, 0.96)],
    ... )
    >>> curve.df(0.5)
    0.98

Examples
--------
>>> import finstack_quant.core.market_data as market_data
>>> market_data.__name__
'finstack_quant.core.market_data'
"""

from __future__ import annotations

import datetime
from decimal import Decimal
from typing import Optional, Union

from finstack_quant.core.currency import Currency
from finstack_quant.core.money import Money
from finstack_quant.core.market_data import arbitrage as arbitrage
from finstack_quant.core.market_data import context as context
from finstack_quant.core.market_data import curves as curves
from finstack_quant.core.market_data import dtsm as dtsm
from finstack_quant.core.market_data import fx as fx
from finstack_quant.core.market_data import scalars as scalars

__all__ = [
    # submodules
    "curves",
    "fx",
    "context",
    "scalars",
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
    "ScalarTimeSeries",
    "InflationIndex",
    # context
    "MarketContext",
]

# ---------------------------------------------------------------------------
# Curves
# ---------------------------------------------------------------------------

class DiscountCurve:
    """
    Discount factor curve for present-value calculations.

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
    validation_mode : str
        Rust validation preset: ``"market_standard"`` (default) or
        ``"negative_rate_friendly"``.
    forward_floor : float | None
        Required minimum implied forward for ``"negative_rate_friendly"``.

    Raises
    ------
    ValueError
        If the curve cannot be built from the given parameters.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.market_data import DiscountCurve
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
        validation_mode: str = "market_standard",
        forward_floor: float | None = None,
    ) -> None:
        """
        Construct a discount curve from knot points.

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
        validation_mode : str
            Rust validation preset.
        forward_floor : float | None
            Required with ``validation_mode="negative_rate_friendly"``.

        Raises
        ------
        ValueError
            If the curve cannot be built.
        """
        ...

    @staticmethod
    def flat(
        id: str,
        base_date: datetime.date,
        continuous_rate: float,
    ) -> DiscountCurve:
        """
        Construct a flat continuously-compounded discount curve.

        Parameters
        ----------
        id : str
            Unique market-context identifier for the constructed curve.
        base_date : datetime.date
            Date at which the curve has a discount factor of one.
        continuous_rate : float
            Flat annual continuously compounded zero rate as a decimal, such
            as ``0.05`` for 5%.

        Returns
        -------
        DiscountCurve
            Result of flat for this `DiscountCurve` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.core.market_data import DiscountCurve
        >>> callable(DiscountCurve.flat)
        True
        """
        ...

    def df(self, t: float) -> float:
        """
        Discount factor at year fraction *t*.

        Parameters
        ----------
        t : float
            Time in year fractions from the base date.

        Returns
        -------
        float
            Discount factor.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def zero(self, t: float) -> float:
        """
        Continuously-compounded zero rate at year fraction *t*.

        Parameters
        ----------
        t : float
            Time in year fractions.

        Returns
        -------
        float
            Zero rate.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def forward(self, t1: float, t2: float) -> float:
        """
        Continuously-compounded forward rate between *t1* and *t2*.

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
        """
        Curve identifier string.

        Returns
        -------
        str
            The id exposed by this `DiscountCurve`.
        """
        ...

    @property
    def base_date(self) -> datetime.date:
        """
        Valuation base date.

        Returns
        -------
        datetime.date
            The base date exposed by this `DiscountCurve`.
        """
        ...

    def __repr__(self) -> str: ...

class ForwardCurve:
    """
    Forward rate curve for a floating-rate index with a fixed tenor.

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
    projection_grid : list[float] | None, optional
        Contractual reset/end-date projection boundaries. Omit for legacy
        fixed numeric-tenor DF stepping.
    reset_lag : int | None, optional
        Business days from fixing to spot. Omit for Rust curve-ID inference.

    Raises
    ------
    ValueError
        If the curve cannot be built from the given parameters.

    Examples
    --------
    >>> from finstack_quant.core.market_data import ForwardCurve
    >>> ForwardCurve.__name__
    'ForwardCurve'
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
        projection_grid: list[float] | None = None,
        reset_lag: int | None = None,
    ) -> None:
        """
        Construct a forward rate curve from knot points.

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
        projection_grid : list[float] | None, optional
            Contractual reset/end-date projection boundaries.
        reset_lag : int | None, optional
            Business days from fixing to spot.
        Raises
        ------
        ValueError
            If the curve cannot be built.
        """
        ...

    @classmethod
    def from_knots(
        cls,
        id: str,
        *,
        tenor: float,
        base_date: datetime.date,
        knots: list[tuple[float, float]],
        day_count: str | None = None,
        interp: str = "linear",
        extrapolation: str = "flat_forward",
        projection_grid: list[float] | None = None,
        reset_lag: int | None = None,
    ) -> ForwardCurve:
        """
        Construct from an unambiguous keyword-only specification.

        Parameters
        ----------
        id : str
            Unique market-context identifier for the index forward curve.
        tenor : float
            Contractual floating-index tenor in years, such as ``0.25`` for 3M.
        base_date : datetime.date
            Curve valuation date corresponding to time zero.
        knots : list[tuple[float, float]]
            ``(time_years, forward_rate)`` pillars in ascending time order.
        day_count : str or None, default None
            Day-count convention; ``None`` applies the curve-ID market default.
        interp : str, default "linear"
            Interpolation method used between supplied forward-rate pillars.
        extrapolation : str, default "flat_forward"
            Policy applied before the first or after the last curve pillar.
        projection_grid : list[float] or None, default None
            Optional contractual reset/end-date boundaries in year fractions.
        reset_lag : int or None, default None
            Business days from fixing to spot; ``None`` uses curve-ID inference.

        Returns
        -------
        ForwardCurve
            Result of from knots for this `ForwardCurve` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.core.market_data import ForwardCurve
        >>> callable(ForwardCurve.from_knots)
        True
        """
        ...

    def rate(self, t: float) -> float:
        """
        Forward rate at year fraction *t*.

        Parameters
        ----------
        t : float
            Time in year fractions.

        Returns
        -------
        float
            Forward rate.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def rate_between(self, t1: float, t2: float) -> float:
        """
        Discount-factor-implied simple forward rate over ``(t1, t2)``.

        Parameters
        ----------
        t1 : float
            Start of the accrual interval in year fractions from ``base_date``.
        t2 : float
            End of the accrual interval in year fractions; it must exceed ``t1``.

        Raises
        ------
        ValueError
            If either time is non-finite or ``t2 <= t1``.

        Returns
        -------
        float
            Result of rate between for this `ForwardCurve` in the annotated representation.
        """
        ...

    @property
    def id(self) -> str:
        """
        Curve identifier string.

        Returns
        -------
        str
            The id exposed by this `ForwardCurve`.
        """
        ...

    @property
    def base_date(self) -> datetime.date:
        """
        Valuation base date.

        Returns
        -------
        datetime.date
            The base date exposed by this `ForwardCurve`.
        """
        ...

    @property
    def projection_grid(self) -> list[float] | None:
        """
        Contractual projection boundaries, if explicitly configured.

        Returns
        -------
        list[float] | None
            The projection grid exposed by this `ForwardCurve`.
        """
        ...

    @property
    def reset_lag(self) -> int:
        """
        Business days from fixing to spot.

        Returns
        -------
        int
            The reset lag exposed by this `ForwardCurve`.
        """
        ...

    def __repr__(self) -> str: ...

class HazardCurve:
    """
    Credit hazard-rate curve for default probability modeling.

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

    Examples
    --------
    >>> from finstack_quant.core.market_data import HazardCurve
    >>> HazardCurve.__name__
    'HazardCurve'
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
        """
        Construct a hazard curve from knot points.

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
        """
        Survival probability at year fraction *t*.

        Parameters
        ----------
        t : float
            Time in year fractions.

        Returns
        -------
        float
            Survival probability in ``[0, 1]``.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def hazard_rate(self, t: float) -> float:
        """
        Instantaneous hazard rate at year fraction *t*.

        Parameters
        ----------
        t : float
            Time in year fractions.

        Returns
        -------
        float
            Hazard rate.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    @property
    def id(self) -> str:
        """
        Curve identifier string.

        Returns
        -------
        str
            The id exposed by this `HazardCurve`.
        """
        ...

    @property
    def base_date(self) -> datetime.date:
        """
        Valuation base date.

        Returns
        -------
        datetime.date
            The base date exposed by this `HazardCurve`.
        """
        ...

    def __repr__(self) -> str: ...

class BaseCorrelationCurve:
    """
    Base-correlation curve for synthetic credit index tranche pricing.

    Examples
    --------
    >>> from finstack_quant.core.market_data import BaseCorrelationCurve
    >>> BaseCorrelationCurve.__name__
    'BaseCorrelationCurve'
    """

    def __init__(self, id: str, knots: list[tuple[float, float]]) -> None:
        """
        Construct a base-correlation curve from knot points.

        Parameters
        ----------
        id : str
            Unique market-context identifier for the tranche correlation curve.
        knots : list[tuple[float, float]]
            ``(detachment_percent, base_correlation)`` pillars ordered by
            detachment, with correlations represented as decimal fractions.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    @property
    def id(self) -> str:
        """
        Curve identifier string.
        Returns
        -------
        str
            The id exposed by this `BaseCorrelationCurve`.
        """
        ...

    def correlation(self, detachment_pct: float) -> float:
        """
        Return interpolated base correlation at a tranche detachment point.

        Parameters
        ----------
        detachment_pct : float
            Tranche detachment expressed as a percentage of portfolio notional,
            for example ``30.0`` for a 0-30% base-correlation point.

        Returns
        -------
        float
            Result of correlation for this `BaseCorrelationCurve` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def __repr__(self) -> str: ...

class CreditIndexData:
    """
    Credit index data bundle for synthetic tranche pricing.

    Examples
    --------
    >>> from finstack_quant.core.market_data import CreditIndexData
    >>> CreditIndexData.__name__
    'CreditIndexData'
    """

    def __init__(
        self,
        num_constituents: int,
        recovery_rate: float,
        index_credit_curve: HazardCurve,
        base_correlation_curve: BaseCorrelationCurve,
    ) -> None:
        """
        Construct homogeneous credit index data for tranche pricing.

        Parameters
        ----------
        num_constituents : int
            Number of equal-name constituents in the synthetic credit index.
        recovery_rate : float
            Assumed recovery fraction as a decimal, such as ``0.4`` for 40%.
        index_credit_curve : HazardCurve
            Index-level default-intensity curve used to project portfolio loss.
        base_correlation_curve : BaseCorrelationCurve
            Detachment-dependent correlation curve used for tranche valuation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    @property
    def num_constituents(self) -> int:
        """
        Number of constituents in the index.
        Returns
        -------
        int
            The num constituents exposed by this `CreditIndexData`.
        """
        ...

    @property
    def recovery_rate(self) -> float:
        """
        Index recovery rate.
        Returns
        -------
        float
            The recovery rate exposed by this `CreditIndexData`.
        """
        ...

    def __repr__(self) -> str: ...

class PriceCurve:
    """
    Forward price curve for commodities and other price-based assets.

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

    Examples
    --------
    >>> from finstack_quant.core.market_data import PriceCurve
    >>> PriceCurve.__name__
    'PriceCurve'
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
        """
        Construct a price curve from knot points.

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
        """
        Forward price at year fraction *t*.

        Parameters
        ----------
        t : float
            Time in year fractions.

        Returns
        -------
        float
            Forward price.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    @property
    def id(self) -> str:
        """
        Curve identifier string.

        Returns
        -------
        str
            The id exposed by this `PriceCurve`.
        """
        ...

    @property
    def base_date(self) -> datetime.date:
        """
        Valuation base date.

        Returns
        -------
        datetime.date
            The base date exposed by this `PriceCurve`.
        """
        ...

    def __repr__(self) -> str: ...

class InflationCurve:
    """
    CPI inflation curve for inflation-linked pricing and breakeven analysis.

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

    Examples
    --------
    >>> from finstack_quant.core.market_data import InflationCurve
    >>> InflationCurve.__name__
    'InflationCurve'
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
    ) -> None:
        """
        Compute   init for `InflationCurve`.

        Parameters
        ----------
        id : object
            Stable identifier used to select the required object or result entry.
        base_date : object
            Date used in the documented calculation or scheduling role.
        base_cpi : object
            Value supplied for `base_cpi` to the documented binding operation.
        knots : object
            Value supplied for `knots` to the documented binding operation.
        day_count : object
            Value supplied for `day_count` to the documented binding operation.
        indexation_lag_months : object
            Value supplied for `indexation_lag_months` to the documented binding operation.
        interp : object
            Value supplied for `interp` to the documented binding operation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def cpi(self, t: float) -> float:
        """
        Return CPI level at a curve time without indexation lag.

        Parameters
        ----------
        t : float
            Year fraction from the curve base date at which CPI is requested.

        Returns
        -------
        float
            Result of cpi for this `InflationCurve` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def cpi_with_lag(self, t: float) -> float:
        """
        Return CPI level at a curve time after the contractual observation lag.

        Parameters
        ----------
        t : float
            Year fraction from the curve base date before applying indexation lag.

        Returns
        -------
        float
            Result of cpi with lag for this `InflationCurve` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def index_ratio(self, t: float) -> float:
        """
        Return the principal indexation ratio at a curve time.

        Computes ``cpi_with_lag(t) / base_cpi`` -- the factor by which the
        notional of an inflation-linked security is uplifted at `t`. This is
        the curve-level view of the ``inflation_index_ratio`` reported per
        cashflow by the valuations layer. No deflation floor is applied, so
        the ratio may fall below 1.0 in a deflationary curve.

        Parameters
        ----------
        t : float
            Year fraction from the curve base date identifying the settlement
            date whose indexed principal is wanted; the indexation lag is
            applied on top of it.

        Returns
        -------
        float
            Lagged CPI at `t` divided by the curve's base CPI, as a
            dimensionless multiplier on original face.

        Raises
        ------
        ValueError
            If the curve's base CPI is not finite and strictly positive.
        """
        ...

    def inflation_rate(self, t1: float, t2: float) -> float:
        """
        Return annualized compounded inflation between two curve times.

        Parameters
        ----------
        t1 : float
            Start time in year fractions from the inflation-curve base date.
        t2 : float
            End time in year fractions from the inflation-curve base date.

        Returns
        -------
        float
            Result of inflation rate for this `InflationCurve` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def inflation_rate_simple(self, t1: float, t2: float) -> float:
        """
        Return simple non-compounded inflation between two curve times.

        Parameters
        ----------
        t1 : float
            Start time in year fractions from the inflation-curve base date.
        t2 : float
            End time in year fractions from the inflation-curve base date.

        Returns
        -------
        float
            Result of inflation rate simple for this `InflationCurve` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    @property
    def id(self) -> str:
        """
        Return the id for `InflationCurve`.

        Returns
        -------
        str
            The id exposed by this `InflationCurve`.
        """
        ...

    @property
    def base_date(self) -> datetime.date:
        """
        Return the base date for `InflationCurve`.

        Returns
        -------
        datetime.date
            The base date exposed by this `InflationCurve`.
        """
        ...

    @property
    def day_count(self) -> str:
        """
        Return the day count for `InflationCurve`.

        Returns
        -------
        str
            The day count exposed by this `InflationCurve`.
        """
        ...

    @property
    def indexation_lag_months(self) -> int:
        """
        Return the indexation lag months for `InflationCurve`.

        Returns
        -------
        int
            The indexation lag months exposed by this `InflationCurve`.
        """
        ...

    @property
    def base_cpi(self) -> float:
        """
        Return the base cpi for `InflationCurve`.

        Returns
        -------
        float
            The base cpi exposed by this `InflationCurve`.
        """
        ...

    def __repr__(self) -> str: ...

class VolSurface:
    """
    Two-dimensional implied volatility surface on an expiry x strike grid.

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

    Examples
    --------
    >>> from finstack_quant.core.market_data import VolSurface
    >>> VolSurface.__name__
    'VolSurface'
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
    ) -> None:
        """
        Compute   init for `VolSurface`.

        Parameters
        ----------
        id : object
            Stable identifier used to select the required object or result entry.
        expiries : object
            Value supplied for `expiries` to the documented binding operation.
        strikes : object
            Value supplied for `strikes` to the documented binding operation.
        vols_row_major : object
            Value supplied for `vols_row_major` to the documented binding operation.
        secondary_axis : object
            Value supplied for `secondary_axis` to the documented binding operation.
        interpolation_mode : object
            Value supplied for `interpolation_mode` to the documented binding operation.
        quote_type : object
            Value supplied for `quote_type` to the documented binding operation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def value_checked(self, expiry: float, strike: float) -> float:
        """
        Return an interpolated volatility with explicit grid bounds checking.

        Parameters
        ----------
        expiry : float
            Option expiry in years, required to lie within the surface grid.
        strike : float
            Strike or configured secondary-axis coordinate to interpolate at.

        Returns
        -------
        float
            Result of value checked for this `VolSurface` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def value_clamped(self, expiry: float, strike: float) -> float:
        """
        Return an interpolated volatility with flat edge extrapolation.

        Parameters
        ----------
        expiry : float
            Option expiry in years; values beyond the grid are clamped to an edge.
        strike : float
            Strike or configured secondary-axis coordinate, clamped at grid edges.

        Returns
        -------
        float
            Result of value clamped for this `VolSurface` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    @property
    def id(self) -> str:
        """
        Return the id for `VolSurface`.

        Returns
        -------
        str
            The id exposed by this `VolSurface`.
        """
        ...

    @property
    def expiries(self) -> list[float]:
        """
        Return the expiries for `VolSurface`.

        Returns
        -------
        list[float]
            The expiries exposed by this `VolSurface`.
        """
        ...

    @property
    def strikes(self) -> list[float]:
        """
        Return the strikes for `VolSurface`.

        Returns
        -------
        list[float]
            The strikes exposed by this `VolSurface`.
        """
        ...

    @property
    def secondary_axis(self) -> str:
        """
        Return the secondary axis for `VolSurface`.

        Returns
        -------
        str
            The secondary axis exposed by this `VolSurface`.
        """
        ...

    @property
    def quote_type(self) -> str:
        """
        Return the quote type for `VolSurface`.

        Returns
        -------
        str
            The quote type exposed by this `VolSurface`.
        """
        ...

    @property
    def interpolation_mode(self) -> str:
        """
        Return the interpolation mode for `VolSurface`.

        Returns
        -------
        str
            The interpolation mode exposed by this `VolSurface`.
        """
        ...

    @property
    def grid_shape(self) -> tuple[int, int]:
        """
        Return the grid shape for `VolSurface`.

        Returns
        -------
        tuple[int, int]
            The grid shape exposed by this `VolSurface`.
        """
        ...

    def __repr__(self) -> str: ...

class FxDeltaVolSurface:
    """
    FX vol surface in delta space (ATM, 25-d RR/BF, optional 10-d wings).

    Forward delta (premium-unadjusted). Strike conversion uses Garman-Kohlhagen.
    See ``docs/REFERENCES.md#clark-fx-options`` and ``#wystup-fx-options``.

    Examples
    --------
    >>> from finstack_quant.core.market_data import FxDeltaVolSurface
    >>> FxDeltaVolSurface.__name__
    'FxDeltaVolSurface'
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
        """
        Build a delta-quoted FX vol surface.

        Parameters
        ----------
        id : str
            Unique surface identifier.
        expiries : list[float]
            Strictly increasing positive expiry times (years).
        atm_vols : list[float]
            ATM delta-neutral straddle vols per expiry (positive).
        rr_25d : list[float]
            25-delta risk reversal per expiry (call vol − put vol).
        bf_25d : list[float]
            25-delta butterfly per expiry (wing average − ATM).
        rr_10d : list[float] or None, default None
            Optional 10-delta risk reversals; require ``bf_10d`` when supplied.
        bf_10d : list[float] or None, default None
            Optional 10-delta butterflies; require ``rr_10d`` when supplied.

        Raises
        ------
        ValueError
            Invalid inputs or mismatched ``rr_10d`` / ``bf_10d``.
        """
        ...
    @property
    def id(self) -> str:
        """
        Return the id for `FxDeltaVolSurface`.

        Returns
        -------
        str
            The id exposed by this `FxDeltaVolSurface`.
        """
        ...

    @property
    def expiries(self) -> list[float]:
        """
        Return the expiries for `FxDeltaVolSurface`.

        Returns
        -------
        list[float]
            The expiries exposed by this `FxDeltaVolSurface`.
        """
        ...

    @property
    def num_expiries(self) -> int:
        """
        Return the num expiries for `FxDeltaVolSurface`.

        Returns
        -------
        int
            The num expiries exposed by this `FxDeltaVolSurface`.
        """
        ...

    def pillar_vols(self, expiry_idx: int) -> tuple[float, float, float]:
        """
        Pillar vols at ``expiry_idx`` as ``(atm, put_25d_vol, call_25d_vol)``.

        Parameters
        ----------
        expiry_idx : int
            Zero-based index into the surface's ordered expiry pillars.

        Raises
        ------
        IndexError
            If ``expiry_idx`` is out of range.

        Returns
        -------
        tuple[float, float, float]
            Result of pillar vols for this `FxDeltaVolSurface` in the annotated representation.
        """
        ...

    def implied_vol(
        self,
        expiry: float,
        strike: float,
        forward: float,
    ) -> float:
        """
        Return interpolated implied volatility at an FX strike.

        Parameters
        ----------
        expiry : float
            Option expiry in years, interpolated across the delta-surface pillars.
        strike : float
            FX strike quoted as units of quote currency per base currency.
        forward : float
            Positive FX forward for the same expiry and quotation direction.

        Returns
        -------
        float
            Result of implied vol for this `FxDeltaVolSurface` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def to_vol_surface(self, spot: float, r_d: float, r_f: float) -> VolSurface:
        """
        Materialize this delta surface as a strike-axis :class:`VolSurface`.

        Parameters
        ----------
        spot : float
            Positive FX spot in the surface's base/quote convention.
        r_d : float
            Domestic continuously compounded annual rate as a decimal.
        r_f : float
            Foreign continuously compounded annual rate as a decimal.

        Returns
        -------
        VolSurface
            Result of to vol surface for this `FxDeltaVolSurface` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    @staticmethod
    def delta_to_strike(delta: float, forward: float, vol: float, expiry: float) -> float:
        """
        Convert a forward delta to a premium-unadjusted Garman-Kohlhagen strike.

        Parameters
        ----------
        delta : float
            Forward call delta, with the sign selecting call or put convention.
        forward : float
            Positive FX forward in the chosen base/quote quotation direction.
        vol : float
            Annualized implied volatility as a positive decimal.
        expiry : float
            Positive option expiry in years.

        Returns
        -------
        float
            Result of delta to strike for this `FxDeltaVolSurface` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.core.market_data import FxDeltaVolSurface
        >>> callable(FxDeltaVolSurface.delta_to_strike)
        True
        """
        ...

    @staticmethod
    def strike_to_delta(strike: float, forward: float, vol: float, expiry: float) -> float:
        """
        Convert a strike to premium-unadjusted forward call delta.

        Parameters
        ----------
        strike : float
            Positive FX strike in the selected base/quote quotation direction.
        forward : float
            Positive FX forward for the option expiry.
        vol : float
            Annualized implied volatility as a positive decimal.
        expiry : float
            Positive option expiry in years.

        Returns
        -------
        float
            Result of strike to delta for this `FxDeltaVolSurface` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.core.market_data import FxDeltaVolSurface
        >>> callable(FxDeltaVolSurface.strike_to_delta)
        True
        """
        ...

    def __repr__(self) -> str: ...

class VolCube:
    """
    SABR volatility cube on an expiry x tenor grid.

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

    Examples
    --------
    >>> from finstack_quant.core.market_data import VolCube
    >>> VolCube.__name__
    'VolCube'
    """

    def __init__(
        self,
        id: str,
        expiries: list[float],
        tenors: list[float],
        params_row_major: list[dict[str, float]],
        forwards_row_major: list[float],
        interpolation_mode: str = "vol",
    ) -> None:
        """
        Compute   init for `VolCube`.

        Parameters
        ----------
        id : object
            Stable identifier used to select the required object or result entry.
        expiries : object
            Value supplied for `expiries` to the documented binding operation.
        tenors : object
            Value supplied for `tenors` to the documented binding operation.
        params_row_major : object
            Value supplied for `params_row_major` to the documented binding operation.
        forwards_row_major : object
            Value supplied for `forwards_row_major` to the documented binding operation.
        interpolation_mode : object
            Value supplied for `interpolation_mode` to the documented binding operation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def vol(self, expiry: float, tenor: float, strike: float) -> float:
        """
        Implied volatility with bounds checking.

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
        """
        Return Black implied volatility with clamped extrapolation.

        Parameters
        ----------
        expiry : float
            Option expiry in years, clamped to the nearest cube expiry when outside.
        tenor : float
            Underlying swap tenor in years, clamped to the nearest cube tenor.
        strike : float
            Strike rate in decimal rate units; non-finite inputs return ``NaN``.

        Returns
        -------
        float
            Result of vol clamped for this `VolCube` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def vol_normal(self, expiry: float, tenor: float, strike: float) -> float:
        """
        Normal (Bachelier) implied volatility with bounds checking.

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
        """
        Normal (Bachelier) implied volatility with clamped extrapolation.

        Degenerate finite expansions are floored to a small positive normal
        vol. Non-finite inputs return NaN.

        Parameters
        ----------
        expiry : float
            Option expiry in years, clamped to the nearest cube expiry when outside.
        tenor : float
            Underlying swap tenor in years, clamped to the nearest cube tenor.
        strike : float
            Strike rate in decimal rate units; non-finite inputs return ``NaN``.

        Returns
        -------
        float
            Result of vol normal clamped for this `VolCube` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def materialize_tenor_slice(self, tenor: float, strikes: list[float]) -> VolSurface:
        """
        Materialize a tenor slice as a :class:`VolSurface`.

        Parameters
        ----------
        tenor : float
            Tenor to slice at (years).
        strikes : list[float]
            Strike axis for the resulting surface.

        Returns
        -------
        VolSurface
            Result of materialize tenor slice for this `VolCube` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def materialize_tenor_slice_normal(self, tenor: float, strikes: list[float]) -> VolSurface:
        """
        Materialize a tenor slice as a normal-vol (Bachelier) :class:`VolSurface`.

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
            Result of materialize tenor slice normal for this `VolCube` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def materialize_expiry_slice(self, expiry: float, strikes: list[float]) -> VolSurface:
        """
        Materialize an expiry slice as a :class:`VolSurface`.

        Parameters
        ----------
        expiry : float
            Expiry to slice at (years).
        strikes : list[float]
            Strike axis for the resulting surface.

        Returns
        -------
        VolSurface
            Result of materialize expiry slice for this `VolCube` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def materialize_expiry_slice_normal(self, expiry: float, strikes: list[float]) -> VolSurface:
        """
        Materialize an expiry slice as a normal-vol (Bachelier) :class:`VolSurface`.

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
            Result of materialize expiry slice normal for this `VolCube` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    @property
    def id(self) -> str:
        """
        Return the id for `VolCube`.

        Returns
        -------
        str
            The id exposed by this `VolCube`.
        """
        ...

    @property
    def expiries(self) -> list[float]:
        """
        Return the expiries for `VolCube`.

        Returns
        -------
        list[float]
            The expiries exposed by this `VolCube`.
        """
        ...

    @property
    def tenors(self) -> list[float]:
        """
        Return the tenors for `VolCube`.

        Returns
        -------
        list[float]
            The tenors exposed by this `VolCube`.
        """
        ...

    @property
    def grid_shape(self) -> tuple[int, int]:
        """
        Return the grid shape for `VolCube`.

        Returns
        -------
        tuple[int, int]
            The grid shape exposed by this `VolCube`.
        """
        ...

    @property
    def interpolation_mode(self) -> str:
        """
        Return the interpolation mode for `VolCube`.

        Returns
        -------
        str
            The interpolation mode exposed by this `VolCube`.
        """
        ...

    def __repr__(self) -> str: ...

class VolatilityIndexCurve:
    """
    Volatility index forward curve (e.g. VIX term structure).

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

    Examples
    --------
    >>> from finstack_quant.core.market_data import VolatilityIndexCurve
    >>> VolatilityIndexCurve.__name__
    'VolatilityIndexCurve'
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
        """
        Construct a volatility index curve from knot points.

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
        """
        Forward volatility index level at year fraction *t*.

        Parameters
        ----------
        t : float
            Time in year fractions.

        Returns
        -------
        float
            Forward volatility index level.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    @property
    def id(self) -> str:
        """
        Curve identifier string.

        Returns
        -------
        str
            The id exposed by this `VolatilityIndexCurve`.
        """
        ...

    @property
    def base_date(self) -> datetime.date:
        """
        Valuation base date.

        Returns
        -------
        datetime.date
            The base date exposed by this `VolatilityIndexCurve`.
        """
        ...

    def __repr__(self) -> str: ...

# ---------------------------------------------------------------------------
# FX
# ---------------------------------------------------------------------------

class FxConversionPolicy:
    """
    FX conversion policy controlling when rates are sampled.

    Immutable enum-style type with class-level constants.

    Examples
    --------
    >>> from finstack_quant.core.market_data import FxConversionPolicy
    >>> FxConversionPolicy.__name__
    'FxConversionPolicy'
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
        """
        Parse from a string label.

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

        Examples
        --------
        >>> from finstack_quant.core.market_data import FxConversionPolicy
        >>> callable(FxConversionPolicy.from_name)
        True
        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class FxRateResult:
    """
    Result of an FX rate query.

    Immutable value type returned by :meth:`FxMatrix.rate`.

    Examples
    --------
    >>> from finstack_quant.core.market_data import FxRateResult
    >>> FxRateResult.__name__
    'FxRateResult'
    """

    @property
    def rate(self) -> float:
        """
        The FX conversion rate.

        Returns
        -------
        float
            The rate exposed by this `FxRateResult`.
        """
        ...

    @property
    def triangulated(self) -> bool:
        """
        Whether the rate was obtained via triangulation.

        Returns
        -------
        bool
            The triangulated exposed by this `FxRateResult`.
        """
        ...

    def __repr__(self) -> str: ...

class FxMatrix:
    """
    Foreign-exchange rate matrix for currency conversion.

    Manages explicit FX quotes and supports rate lookup with optional
    triangulation.

    Examples
    --------
    >>> from finstack_quant.core.market_data import FxMatrix
    >>> FxMatrix.__name__
    'FxMatrix'
    """

    def __init__(self) -> None:
        """
        Create an empty FX matrix.
        Returns
        -------
        None
        """
        ...

    def set_quote(
        self,
        base: Union[Currency, str],
        quote: Union[Currency, str],
        rate: float,
    ) -> None:
        """
        Set an explicit FX quote.

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

    def set_quote_on(
        self,
        base: Union[Currency, str],
        quote: Union[Currency, str],
        date: datetime.date,
        policy: Union[FxConversionPolicy, str],
        rate: float,
    ) -> None:
        """
        Set an authoritative FX quote scoped to one date and conversion policy.

        Parameters
        ----------
        base : Currency or str
            Source currency to convert from, as a ``Currency`` or ISO-4217 code.
        quote : Currency or str
            Destination currency to convert to, as a ``Currency`` or ISO code.
        date : datetime.date
            Valuation date for which this quote is authoritative.
        policy : FxConversionPolicy or str
            Conversion policy key that selects this dated quote during lookup.
        rate : float
            Positive conversion rate satisfying ``1 base = rate quote``.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def rate(
        self,
        base: Union[Currency, str],
        quote: Union[Currency, str],
        date: datetime.date,
        policy: Optional[Union[FxConversionPolicy, str]] = None,
    ) -> FxRateResult:
        """
        Look up an FX rate.

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
# Scalar time series
# ---------------------------------------------------------------------------

class ScalarTimeSeries:
    """
    Date-indexed scalar market observations with Rust-owned interpolation.

    Decimal observations are accepted only when exactly representable by the
    underlying float storage.

    Examples
    --------
    >>> from finstack_quant.core.market_data import ScalarTimeSeries
    >>> ScalarTimeSeries.__name__
    'ScalarTimeSeries'
    """

    def __init__(
        self,
        id: str,
        observations: list[tuple[datetime.date, float | int | Decimal]],
        currency: Currency | str | None = None,
        interpolation: str | None = None,
    ) -> None:
        """
        Create a date-indexed scalar market-data series.

        Parameters
        ----------
        id : str
            Stable market-context identifier for the observation series.
        observations : list[tuple[datetime.date, float | int | Decimal]]
            Dated scalar values, ordered or sortable by date; ``Decimal``
            values must be exactly representable by the Rust float storage.
        currency : Currency or str or None, default None
            Optional currency tag for monetary observations; ``None`` is unitless.
        interpolation : str or None, default None
            Optional interpolation mode; ``None`` selects the binding default.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...
    @property
    def id(self) -> str:
        """
        Return the id for `ScalarTimeSeries`.

        Returns
        -------
        str
            The id exposed by this `ScalarTimeSeries`.
        """
        ...

    @property
    def currency(self) -> Currency | None:
        """
        Return the currency for `ScalarTimeSeries`.

        Returns
        -------
        Currency | None
            The currency exposed by this `ScalarTimeSeries`.
        """
        ...

    @property
    def interpolation(self) -> str:
        """
        Return the interpolation for `ScalarTimeSeries`.

        Returns
        -------
        str
            The interpolation exposed by this `ScalarTimeSeries`.
        """
        ...

    @property
    def observations(self) -> list[tuple[datetime.date, float]]:
        """
        Return the observations for `ScalarTimeSeries`.

        Returns
        -------
        list[tuple[datetime.date, float]]
            The observations exposed by this `ScalarTimeSeries`.
        """
        ...

    def value_on(self, date: datetime.date) -> float:
        """
        Return the interpolated scalar value on a requested date.

        Parameters
        ----------
        date : datetime.date
            Observation or interpolation date evaluated under this series mode.

        Returns
        -------
        float
            Result of value on for this `ScalarTimeSeries` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...
    def to_json(self) -> str:
        """
        Serialize `ScalarTimeSeries` to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation of this `ScalarTimeSeries`, suitable for a matching `from_json` call.
        """
        ...

    @staticmethod
    def from_json(json: str) -> ScalarTimeSeries:
        """
        Parse a scalar series from its canonical JSON representation.

        Parameters
        ----------
        json : str
            Canonical serialized series JSON, including identifier, observations,
            optional currency, and interpolation configuration.

        Returns
        -------
        ScalarTimeSeries
            Validated `ScalarTimeSeries` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the JSON payload cannot be parsed or does not satisfy the `ValueError` schema and invariants.

        Examples
        --------
        >>> from finstack_quant.core.market_data import ScalarTimeSeries
        >>> callable(ScalarTimeSeries.from_json)
        True
        """
        ...
    def __len__(self) -> int: ...
    def __repr__(self) -> str: ...

class InflationIndex:
    """
    Inflation index observations with Rust-owned interpolation and validation.

    Examples
    --------
    >>> from finstack_quant.core.market_data import InflationIndex
    >>> InflationIndex.__name__
    'InflationIndex'
    """

    def __init__(
        self,
        id: str,
        observations: list[tuple[datetime.date, float | int]],
        currency: Currency | str,
        interpolation: str | None = None,
    ) -> None:
        """
        Create a date-indexed inflation-index observation series.

        Parameters
        ----------
        id : str
            Stable market-context identifier for the inflation index.
        observations : list[tuple[datetime.date, float | int]]
            Dated CPI or index levels, ordered or sortable by observation date.
        currency : Currency or str
            Currency or economic-area tag attached to the published index level.
        interpolation : str or None, default None
            Optional interpolation mode; ``None`` selects the binding default.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...
    @property
    def id(self) -> str:
        """
        Return the id for `InflationIndex`.

        Returns
        -------
        str
            The id exposed by this `InflationIndex`.
        """
        ...

    @property
    def currency(self) -> Currency:
        """
        Return the currency for `InflationIndex`.

        Returns
        -------
        Currency
            The currency exposed by this `InflationIndex`.
        """
        ...

    @property
    def interpolation(self) -> str:
        """
        Return the interpolation for `InflationIndex`.

        Returns
        -------
        str
            The interpolation exposed by this `InflationIndex`.
        """
        ...

    @property
    def observations(self) -> list[tuple[datetime.date, float]]:
        """
        Return the observations for `InflationIndex`.

        Returns
        -------
        list[tuple[datetime.date, float]]
            The observations exposed by this `InflationIndex`.
        """
        ...

    def value_on(self, date: datetime.date) -> float:
        """
        Return the interpolated index level on a requested date.

        Parameters
        ----------
        date : datetime.date
            Observation or interpolation date evaluated under this index mode.

        Returns
        -------
        float
            Result of value on for this `InflationIndex` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...
    def to_json(self) -> str:
        """
        Serialize `InflationIndex` to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation of this `InflationIndex`, suitable for a matching `from_json` call.
        """
        ...

    @staticmethod
    def from_json(json: str) -> InflationIndex:
        """
        Parse an inflation index from its canonical JSON representation.

        Parameters
        ----------
        json : str
            Canonical serialized index JSON, including identifier, levels,
            currency, and interpolation configuration.

        Returns
        -------
        InflationIndex
            Validated `InflationIndex` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the JSON payload cannot be parsed or does not satisfy the `ValueError` schema and invariants.

        Examples
        --------
        >>> from finstack_quant.core.market_data import InflationIndex
        >>> callable(InflationIndex.from_json)
        True
        """
        ...
    def __len__(self) -> int: ...
    def __repr__(self) -> str: ...

# ---------------------------------------------------------------------------
# Market context
# ---------------------------------------------------------------------------

class MarketContext:
    """
    Unified market data container for curves, surfaces, and FX.

    Provides a single access point for all market data required by
    pricing and analytics functions. Curves are stored behind ``Arc``
    and the context is cheap to clone.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> MarketContext.__name__
    'MarketContext'
    """

    def __init__(self) -> None:
        """
        Create an empty market context.
        Returns
        -------
        None
        """
        ...

    def insert(
        self,
        curve: Union[
            DiscountCurve,
            ForwardCurve,
            HazardCurve,
            InflationCurve,
            PriceCurve,
            BaseCorrelationCurve,
            VolSurface,
            FxDeltaVolSurface,
            VolCube,
            VolatilityIndexCurve,
        ],
    ) -> MarketContext:
        """
        Insert a curve into the context (fluent, returns ``self``).

        Accepts any curve type: :class:`DiscountCurve`, :class:`ForwardCurve`,
        :class:`HazardCurve`, :class:`InflationCurve`, :class:`PriceCurve`,
        :class:`BaseCorrelationCurve`, :class:`VolSurface`,
        :class:`FxDeltaVolSurface`, :class:`VolCube`, or
        :class:`VolatilityIndexCurve`.

        Parameters
        ----------
        curve : DiscountCurve | ForwardCurve | HazardCurve | InflationCurve | PriceCurve | BaseCorrelationCurve | VolSurface | FxDeltaVolSurface | VolCube | VolatilityIndexCurve
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
        """
        Insert an FX matrix into the context.

        Parameters
        ----------
        fx : FxMatrix
            FX rate matrix.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def insert_price(
        self,
        id: str,
        value: float | int | Decimal,
        currency: Currency | str | None = None,
    ) -> None:
        """
        Insert a scalar market price into the context.

        Parameters
        ----------
        id : str
            Market scalar identifier.
        value : float | int | Decimal
            Unitless scalar value or monetary amount. Decimal monetary amounts
            preserve full precision; unitless Decimal values must be exactly
            representable as float.
        currency : Currency | str | None, optional
            Currency for monetary prices. If omitted, stores a unitless scalar.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def insert_credit_index(self, id: str, data: CreditIndexData) -> None:
        """
        Insert credit index data into the context.

        Parameters
        ----------
        id : str
            Credit index identifier.
        data : CreditIndexData
            Credit index data bundle.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def insert_series(self, series: ScalarTimeSeries) -> None:
        """
        Insert or replace a scalar time series using its own identifier.

        Parameters
        ----------
        series : ScalarTimeSeries
            Fully validated date-indexed series whose ``id`` becomes the lookup key.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def insert_inflation_index(self, index: InflationIndex) -> None:
        """
        Insert or replace an inflation index using its own identifier.

        Parameters
        ----------
        index : InflationIndex
            Fully validated index observation series whose ``id`` becomes the lookup key.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def get_discount(self, id: str) -> DiscountCurve:
        """
        Retrieve a discount curve by identifier.

        Parameters
        ----------
        id : str
            Curve identifier.

        Returns
        -------
        DiscountCurve

            Requested discount resolved from this `MarketContext` in the annotated representation.
        Raises
        ------
        KeyError
            If no discount curve with this *id* exists.
        """
        ...

    def get_forward(self, id: str) -> ForwardCurve:
        """
        Retrieve a forward curve by identifier.

        Parameters
        ----------
        id : str
            Curve identifier.

        Returns
        -------
        ForwardCurve

            Requested forward resolved from this `MarketContext` in the annotated representation.
        Raises
        ------
        KeyError
            If no forward curve with this *id* exists.
        """
        ...

    def get_hazard(self, id: str) -> HazardCurve:
        """
        Retrieve a hazard curve by identifier.

        Parameters
        ----------
        id : str
            Curve identifier.

        Returns
        -------
        HazardCurve

            Requested hazard resolved from this `MarketContext` in the annotated representation.
        Raises
        ------
        KeyError
            If no hazard curve with this *id* exists.
        """
        ...

    def get_base_correlation(self, id: str) -> BaseCorrelationCurve:
        """
        Retrieve a base-correlation curve by identifier.

        Parameters
        ----------
        id : str
            Market-context key of the base-correlation curve to retrieve.

        Returns
        -------
        BaseCorrelationCurve
            Requested base correlation resolved from this `MarketContext` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def get_inflation_curve(self, id: str) -> InflationCurve:
        """
        Retrieve an inflation curve by identifier.

        Parameters
        ----------
        id : str
            Curve identifier.

        Returns
        -------
        InflationCurve

            Requested inflation curve resolved from this `MarketContext` in the annotated representation.
        Raises
        ------
        KeyError
            If no inflation curve with this *id* exists.
        """
        ...

    def get_price_curve(self, id: str) -> PriceCurve:
        """
        Retrieve a price curve by identifier.

        Parameters
        ----------
        id : str
            Curve identifier.

        Returns
        -------
        PriceCurve

            Requested price curve resolved from this `MarketContext` in the annotated representation.
        Raises
        ------
        KeyError
            If no price curve with this *id* exists.
        """
        ...

    def get_price(self, id: str) -> tuple[float | Decimal, str | None]:
        """
        Retrieve ``(value, currency)`` for a scalar market price.

        Currency-tagged values use :class:`Decimal` to preserve their exact
        stored amount. Unitless values use ``float`` and return ``None`` for
        the currency.

        Parameters
        ----------
        id : str
            Market-context key of the scalar or currency-tagged price to retrieve.

        Returns
        -------
        tuple[float | Decimal, str | None]
            Requested price resolved from this `MarketContext` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def get_series(self, id: str) -> ScalarTimeSeries:
        """
        Retrieve a scalar time series by identifier.

        Parameters
        ----------
        id : str
            Market-context key assigned when the series was inserted.

        Returns
        -------
        ScalarTimeSeries
            Requested series resolved from this `MarketContext` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def get_inflation_index(self, id: str) -> InflationIndex:
        """
        Retrieve an inflation index by identifier.

        Parameters
        ----------
        id : str
            Market-context key assigned when the inflation index was inserted.

        Returns
        -------
        InflationIndex
            Requested inflation index resolved from this `MarketContext` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def get_surface(self, id: str) -> VolSurface:
        """
        Retrieve a vol surface by identifier.

        Parameters
        ----------
        id : str
            Surface identifier.

        Returns
        -------
        VolSurface

            Requested surface resolved from this `MarketContext` in the annotated representation.
        Raises
        ------
        KeyError
            If no surface with this *id* exists.
        """
        ...

    def get_fx_delta_vol_surface(self, id: str) -> FxDeltaVolSurface:
        """
        Retrieve a delta-quoted FX vol surface by identifier.

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
        """
        Retrieve a vol cube by identifier.

        Parameters
        ----------
        id : str
            Cube identifier.

        Returns
        -------
        VolCube

            Requested vol cube resolved from this `MarketContext` in the annotated representation.
        Raises
        ------
        KeyError
            If no vol cube with this *id* exists.
        """
        ...

    def get_vol_index_curve(self, id: str) -> VolatilityIndexCurve:
        """
        Retrieve a volatility index curve by identifier.

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

    def get_credit_index(self, id: str) -> CreditIndexData:
        """
        Retrieve credit-index data by identifier.

        Parameters
        ----------
        id : str
            Market-context key of the synthetic credit-index data bundle.

        Returns
        -------
        CreditIndexData
            Requested credit index resolved from this `MarketContext` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    @property
    def fx(self) -> Optional[FxMatrix]:
        """
        Access the FX matrix (returns ``None`` if not set).

        Returns
        -------
        FxMatrix | None
            The fx exposed by this `MarketContext`.
        """
        ...

    @staticmethod
    def from_json(json: str) -> MarketContext:
        """
        Deserialize a market context from a JSON string.

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

        Examples
        --------
        >>> from finstack_quant.core.market_data import MarketContext
        >>> callable(MarketContext.from_json)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this context to pretty-printed JSON (compatible with pricing APIs).

        Returns
        -------
        str
            JSON string accepted by ``price_instrument`` / ``price_instrument_with_metrics``.
        """
        ...

    def __repr__(self) -> str: ...

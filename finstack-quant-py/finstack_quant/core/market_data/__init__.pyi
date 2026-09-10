"""
Market data bindings from ``finstack-quant-core``: curves, FX, and market context.

Provides term-structure curve types (discount, forward, hazard, price incl.
volatility-index, inflation), volatility surfaces and cubes, the FX rate matrix,
scalar series and the unified :class:`MarketContext` container.

Every curve query (``df``, ``zero``, ``rate``, ``sp``, ``hazard_rate``,
``price``, ``cpi``) accepts either a year fraction (``float``) or a date
(``datetime.date`` or ISO ``"YYYY-MM-DD"`` string); dates are converted with
the curve's own day count by Rust. Constructor options after ``knots`` are
keyword-only.

Examples
--------
>>> import datetime
>>> from finstack_quant.core.market_data import DiscountCurve
>>> round(DiscountCurve.flat("USD-OIS", datetime.date(2025, 1, 1), 0.05).df(1.0), 6)
0.951229

"""

from __future__ import annotations

import datetime
from decimal import Decimal
from typing import Any, Optional, Sequence, Union

import pandas as pd

from finstack_quant.core.currency import Currency
from finstack_quant.core.money import Money
from finstack_quant.core.market_data import context as context
from finstack_quant.core.market_data import curves as curves
from finstack_quant.core.market_data import fx as fx
from finstack_quant.core.market_data import scalars as scalars

__all__ = [
    "BaseCorrelationCurve",
    "CreditIndexData",
    "DiscountCurve",
    "ForwardCurve",
    "FxConversionPolicy",
    "FxDeltaVolSurface",
    "FxMatrix",
    "FxPairConvention",
    "FxQuoteConvention",
    "FxRateResult",
    "HazardCurve",
    "InflationCurve",
    "InflationIndex",
    "MarketContext",
    "PriceCurve",
    "SabrParameterData",
    "ScalarTimeSeries",
    "VolCube",
    "VolSurface",
    "context",
    "curves",
    "fx",
    "fx_market_pair",
    "fx_pair_convention",
    "fx_pip_size",
    "invert_fx_rate",
    "scalars",
]

DateLike = Union[datetime.date, str]
"""A ``datetime.date`` (or ``datetime``/``pandas.Timestamp``) or an ISO ``"YYYY-MM-DD"`` string."""

TimeOrDate = Union[float, datetime.date, str]
"""A year fraction from the curve base date, or a :data:`DateLike` converted with the curve day count."""

# Curves

class DiscountCurve:
    """
    Discount factor curve for present-value calculations.

    Stores ``(t, DF)`` knots in years from ``base_date`` and interpolates
    between them. Instances are immutable; equality compares the canonical
    JSON wire form and ``pickle`` round-trips through ``to_json``.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.market_data import DiscountCurve
    >>> curve = DiscountCurve("USD-OIS", datetime.date(2025, 1, 1), [(0.0, 1.0), (1.0, 0.95), (5.0, 0.80)])
    >>> round(curve.df(1.0), 4)
    0.95
    >>> round(curve.df("2026-01-01"), 4)
    0.95
    >>> curve.knots, curve.day_count
    ([0.0, 1.0, 5.0], 'act_365f')

    """

    def __init__(
        self,
        id: str,
        base_date: DateLike,
        knots: Sequence[tuple[float, float]],
        *,
        interp: Optional[str] = None,
        extrapolation: Optional[str] = None,
        day_count: Optional[str] = None,
        validation_mode: str = "market_standard",
        forward_floor: Optional[float] = None,
    ) -> None:
        """
        Construct a discount curve from ``(time_years, discount_factor)`` knots.

        Parameters
        ----------
        id : str
            Unique curve identifier (e.g. ``"USD-OIS"``).
        base_date : datetime.date or str
            Valuation date anchoring ``t = 0``.
        knots : Sequence[tuple[float, float]]
            ``(time_years, discount_factor)`` pairs; discount factors are
            unitless and positive. A ``(0.0, 1.0)`` anchor is conventional.
        interp : str, optional
            Interpolation style (``"monotone_convex"``, ``"linear"``,
            ``"log_linear"``, ``"cubic"``, ...). Default ``"monotone_convex"``.
        extrapolation : str, optional
            Extrapolation policy (``"flat_forward"``, ``"flat_zero"``,
            ``"linear"``, ``"error"``). Default ``"flat_forward"``.
        day_count : str, optional
            Day-count label used to convert query dates to curve time. The
            default is fixed at ``"act_365f"`` (it is not inferred from ``id``).
        validation_mode : str, optional
            ``"market_standard"`` (default: monotonic DFs and a -50bp implied
            forward floor) or ``"negative_rate_friendly"``.
        forward_floor : float, optional
            Minimum implied forward (decimal) required by
            ``"negative_rate_friendly"``.

        Raises
        ------
        ValueError
            If a knot is non-finite or duplicated, discount factors violate
            the validation mode, or a label is unknown.
        TypeError
            If ``base_date`` is neither a date nor a string.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.core.market_data import DiscountCurve
        >>> curve = DiscountCurve("USD-OIS", datetime.date(2025, 1, 1), [(0.0, 1.0), (1.0, 0.95)], day_count="act_360")
        >>> curve.day_count
        'act_360'

        """
        ...

    @staticmethod
    def flat(id: str, base_date: DateLike, continuous_rate: float) -> DiscountCurve:
        """
        Construct a flat continuously-compounded discount curve.

        Parameters
        ----------
        id : str
            Unique curve identifier.
        base_date : datetime.date or str
            Valuation date anchoring ``t = 0``.
        continuous_rate : float
            Continuously-compounded zero rate as a decimal (``0.05`` is 5%).

        Returns
        -------
        DiscountCurve
            Curve with ``df(t) == exp(-continuous_rate * t)``.

        Raises
        ------
        ValueError
            If the rate is non-finite or has magnitude greater than ``1.0``
            (a percentage passed where a decimal was expected).

        Examples
        --------
        >>> from finstack_quant.core.market_data import DiscountCurve
        >>> round(DiscountCurve.flat("USD-OIS", "2025-01-01", 0.05).df(2.0), 6)
        0.904837

        """
        ...

    @staticmethod
    def from_zero_rates(
        id: str,
        base_date: DateLike,
        points: Sequence[tuple[float, float]],
        compounding: str = "continuous",
    ) -> DiscountCurve:
        """
        Construct a discount curve from zero-rate pillars.

        Parameters
        ----------
        id : str
            Unique curve identifier.
        base_date : datetime.date or str
            Valuation date anchoring ``t = 0``.
        points : Sequence[tuple[float, float]]
            ``(time_years, zero_rate)`` pillars with rates as decimals. A
            ``(0, 1.0)`` discount-factor anchor is added when no ``t = 0``
            pillar is given.
        compounding : str, optional
            ``"continuous"`` (default), ``"simple"``, ``"annual"``,
            ``"semi_annual"``, ``"quarterly"`` or ``"monthly"``.

        Returns
        -------
        DiscountCurve
            Curve with builder defaults (``act_365f``, monotone-convex,
            flat-forward extrapolation, market-standard validation).

        Raises
        ------
        ValueError
            If ``points`` is empty, a pillar is non-finite, the label is
            unknown, or the implied discount factors fail validation.

        Examples
        --------
        >>> from finstack_quant.core.market_data import DiscountCurve
        >>> curve = DiscountCurve.from_zero_rates(
        ...     "USD-OIS", "2025-01-01", [(1.0, 0.05), (2.0, 0.05)], compounding="annual"
        ... )
        >>> round(curve.df(2.0), 6)
        0.907029

        """
        ...

    @staticmethod
    def from_dates(
        id: str,
        base_date: DateLike,
        points: Sequence[tuple[DateLike, float]],
        day_count: Optional[str] = None,
    ) -> DiscountCurve:
        """
        Construct a discount curve from dated discount-factor pillars.

        Parameters
        ----------
        id : str
            Unique curve identifier.
        base_date : datetime.date or str
            Valuation date anchoring ``t = 0``.
        points : Sequence[tuple[datetime.date or str, float]]
            ``(date, discount_factor)`` pillars on or after ``base_date``.
        day_count : str, optional
            Day count used to convert pillar dates to years; default ``"act_365f"``.

        Returns
        -------
        DiscountCurve
            The constructed discount curve, ready for ``df`` and zero-rate queries.

        Raises
        ------
        ValueError
            If ``points`` is empty, a pillar precedes ``base_date``, or the
            discount factors fail validation.

        Examples
        --------
        >>> from finstack_quant.core.market_data import DiscountCurve
        >>> curve = DiscountCurve.from_dates("USD-OIS", "2025-01-01", [("2026-01-01", 0.95)])
        >>> round(curve.df("2026-01-01"), 4)
        0.95

        """
        ...

    @staticmethod
    def from_json(json: str) -> DiscountCurve:
        """
        Deserialize a curve from its canonical JSON wire form.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json` (or the Rust serde impl).

        Returns
        -------
        DiscountCurve
            The constructed discount curve, ready for ``df`` and zero-rate queries.

        Raises
        ------
        ValueError
            If the JSON is malformed, has unknown fields, or fails validation.

        Examples
        --------
        >>> from finstack_quant.core.market_data import DiscountCurve
        >>> curve = DiscountCurve.flat("USD-OIS", "2025-01-01", 0.05)
        >>> DiscountCurve.from_json(curve.to_json()) == curve
        True

        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form.

        Returns
        -------
        str
            Compact JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If the curve cannot be serialized.
        """
        ...

    def df(self, t: TimeOrDate) -> float:
        """
        Discount factor at a year fraction or date.

        Parameters
        ----------
        t : float, datetime.date or str
            Year fraction from ``base_date``, or a date converted with the
            curve day count.

        Returns
        -------
        float
            Unitless discount factor.

        Raises
        ------
        ValueError
            If a date precedes ``base_date`` or is not a valid ISO date.
        TypeError
            If ``t`` is neither a number nor date-like.
        """
        ...

    def zero(self, t: TimeOrDate) -> float:
        """
        Continuously-compounded zero rate (decimal) at a year fraction or date.

        Parameters
        ----------
        t : float, datetime.date or str
            Year fraction from ``base_date``, or a date converted with the
            curve day count.

        Returns
        -------
        float
            Zero rate as a decimal (``0.05`` is 5%); ``0.0`` at ``t = 0``.

        Raises
        ------
        ValueError
            If a date precedes ``base_date``.
        """
        ...

    def zero_annual(self, t: float) -> float:
        """
        Annually-compounded zero rate (decimal) at year fraction ``t``.

        Parameters
        ----------
        t : float
            Year fraction from ``base_date``.

        Returns
        -------
        float
            Zero rate as a decimal under annual compounding.
        Notes
        -----
        This method does not raise.

        """
        ...

    def zero_rate(self, t: float, compounding: str = "continuous") -> float:
        """
        Zero rate at year fraction ``t`` under an explicit compounding convention.

        Parameters
        ----------
        t : float
            Year fraction from ``base_date``.
        compounding : str, optional
            ``"continuous"`` (default), ``"simple"``, ``"annual"``,
            ``"semi_annual"``, ``"quarterly"`` or ``"monthly"``.

        Returns
        -------
        float
            Zero rate as a decimal.

        Raises
        ------
        ValueError
            If ``compounding`` is not a recognised label.
        """
        ...

    def zero_rate_on_date(self, date: DateLike, compounding: str = "continuous") -> float:
        """
        Zero rate on a date under an explicit compounding convention.

        Parameters
        ----------
        date : datetime.date or str
            Target date, converted with the curve day count.
        compounding : str, optional
            Compounding label; see :meth:`zero_rate`. Default ``"continuous"``.

        Returns
        -------
        float
            Zero rate as a decimal.

        Raises
        ------
        ValueError
            If ``date`` precedes ``base_date`` or the label is unknown.
        """
        ...

    def df_on_date_curve(self, date: DateLike) -> float:
        """
        Discount factor on a date using the curve day count.

        Parameters
        ----------
        date : datetime.date or str
            Target date on or after ``base_date``.

        Returns
        -------
        float
            Unitless discount factor.

        Raises
        ------
        ValueError
            If the year fraction cannot be computed.
        """
        ...

    def df_between_dates(self, from_date: DateLike, to_date: DateLike) -> float:
        """
        Forward discount factor between two dates: ``DF(0, to) / DF(0, from)``.

        Parameters
        ----------
        from_date : datetime.date or str
            Start date.
        to_date : datetime.date or str
            End date; may precede ``from_date`` (the ratio inverts). ``1.0``
            when both dates coincide.

        Returns
        -------
        float
            Unitless forward discount factor.

        Raises
        ------
        ValueError
            If either year fraction cannot be computed or a discount factor is
            non-positive.
        """
        ...

    def forward(self, t1: float, t2: float) -> float:
        """
        Continuously-compounded forward rate (decimal) between ``t1`` and ``t2``.

        Parameters
        ----------
        t1 : float
            Start year fraction.
        t2 : float
            End year fraction; must exceed ``t1`` by at least the curve's
            minimum forward tenor.

        Returns
        -------
        float
            Forward rate as a decimal.

        Raises
        ------
        ValueError
            If the interval is empty, reversed or non-finite.
        """
        ...

    def to_forward_curve(self, forward_id: str, tenor: float, interp: Optional[str] = None) -> ForwardCurve:
        """
        Derive a simple forward-rate curve for a fixed tenor from this curve.

        Parameters
        ----------
        forward_id : str
            Identifier for the resulting forward curve.
        tenor : float
            Forward tenor in years (``0.25`` for 3M); must be positive.
        interp : str, optional
            Interpolation style of the forward curve; default ``"linear"``.

        Returns
        -------
        ForwardCurve
            Curve with one simple forward per discount knot.

        Raises
        ------
        ValueError
            If ``tenor`` is non-positive, the curve has fewer than two knots,
            or a derived forward is non-finite.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export knots as a pandas ``DataFrame``.

        Returns
        -------
        pd.DataFrame
            Columns ``t`` (years) and ``df``; one row per knot, ascending.

        Raises
        ------
        ValueError
            If the frame cannot be built.
        """
        ...

    @property
    def id(self) -> str:
        """
        Identifier this curve is registered and looked up under.

        Returns
        -------
        str
            The curve id supplied at construction (for example ``"USD-OIS"``); market-data containers key on it exactly.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def base_date(self) -> datetime.date:
        """
        Valuation date the curve's time axis is measured from.

        Returns
        -------
        datetime.date
            The base date; a year fraction ``t`` on this curve means ``t`` years after this date under :attr:`day_count`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def knots(self) -> list[float]:
        """
        Pillar times the curve is defined on.

        Returns
        -------
        list[float]
            Strictly ascending year fractions measured from ``base_date``; interpolation happens between neighbouring entries.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def dfs(self) -> list[float]:
        """
        Discount factors at the curve pillars.

        Returns
        -------
        list[float]
            One unitless discount factor per entry of :attr:`knots`, in the same order; ``0.95`` means 95 cents today per unit at that pillar.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def day_count(self) -> str:
        """
        Day-count convention converting calendar dates to curve time.

        Returns
        -------
        str
            Lower-case ISDA day-count label such as ``"act_365f"``, ``"act_360"`` or ``"30_360"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def interp_style(self) -> str:
        """
        Interpolation scheme used between pillars.

        Returns
        -------
        str
            Style label such as ``"linear"``, ``"log_linear"`` or ``"monotone_convex"``; it fixes how values between knots are produced.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def extrapolation(self) -> str:
        """
        Policy applied beyond the first and last pillar.

        Returns
        -------
        str
            Policy label such as ``"flat_forward"``, ``"flat_zero"`` or ``"none"``; it governs queries outside the knot range.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...
    def _repr_html_(self) -> Optional[str]: ...

class ForwardCurve:
    """
    Forward rate curve for a floating-rate index with a fixed tenor.

    Stores ``(t, forward_rate)`` knots in years from ``base_date`` with rates
    as decimals (``0.04`` is 4%). ``rate`` and ``df`` accept a year fraction or
    a date; dates are converted with the curve day count by Rust.

    Examples
    --------
    >>> from finstack_quant.core.market_data import ForwardCurve
    >>> curve = ForwardCurve("USD-SOFR-3M", 0.25, "2025-01-01", [(0.0, 0.04), (1.0, 0.045)])
    >>> round(curve.rate(0.5), 4)
    0.0425
    >>> curve.forwards
    [0.04, 0.045]

    """

    def __init__(
        self,
        id: str,
        tenor: float,
        base_date: DateLike,
        knots: Sequence[tuple[float, float]],
        *,
        day_count: Optional[str] = None,
        interp: Optional[str] = None,
        extrapolation: Optional[str] = None,
        projection_grid: Optional[Sequence[float]] = None,
        reset_lag: Optional[int] = None,
    ) -> None:
        """
        Construct a forward rate curve from ``(time_years, forward_rate)`` knots.

        Parameters
        ----------
        id : str
            Unique curve identifier (e.g. ``"USD-SOFR-3M"``). Day count and
            reset lag are inferred from the ID unless given explicitly.
        tenor : float
            Index tenor in years (``0.25`` for 3 months).
        base_date : datetime.date or str
            Valuation date anchoring ``t = 0``.
        knots : Sequence[tuple[float, float]]
            ``(time_years, forward_rate)`` pairs with rates as decimals.
        day_count : str, optional
            Day-count label (``"act_360"``, ``"act_365f"``, ...). When
            omitted, Rust infers a market default from ``id``.
        interp : str, optional
            Interpolation style; default ``"linear"``.
        extrapolation : str, optional
            Extrapolation policy; default ``"flat_forward"``.
        projection_grid : Sequence[float], optional
            Contractual reset/end-date boundaries in years. Omit for fixed
            numeric-tenor stepping.
        reset_lag : int, optional
            Business days from fixing to spot. Omit for curve-ID inference.

        Raises
        ------
        ValueError
            If a knot is non-finite or duplicated, ``tenor`` is non-positive,
            or a label is unknown.

        Examples
        --------
        >>> from finstack_quant.core.market_data import ForwardCurve
        >>> curve = ForwardCurve("USD-SOFR-3M", 0.25, "2025-01-01", [(0.0, 0.04), (1.0, 0.045)], day_count="act_360")
        >>> curve.tenor
        0.25

        """
        ...

    @staticmethod
    def flat(id: str, tenor: float, base_date: DateLike, rate: float) -> ForwardCurve:
        """
        Construct a flat forward curve quoting ``rate`` at every maturity.

        Parameters
        ----------
        id : str
            Unique curve identifier.
        tenor : float
            Index tenor in years; must be positive.
        base_date : datetime.date or str
            Valuation date anchoring ``t = 0``.
        rate : float
            Simple forward rate as a decimal (``0.04`` is 4%).

        Returns
        -------
        ForwardCurve
            The constructed forward curve, ready for index projection.

        Raises
        ------
        ValueError
            If ``rate`` is non-finite or ``tenor`` is non-positive.

        Examples
        --------
        >>> from finstack_quant.core.market_data import ForwardCurve
        >>> round(ForwardCurve.flat("USD-SOFR-3M", 0.25, "2025-01-01", 0.04).rate(7.0), 6)
        0.04

        """
        ...

    @staticmethod
    def from_json(json: str) -> ForwardCurve:
        """
        Deserialize a curve from its canonical JSON wire form.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        ForwardCurve
            The constructed forward curve, ready for index projection.

        Raises
        ------
        ValueError
            If the JSON is malformed, has unknown fields, or fails validation.

        Examples
        --------
        >>> from finstack_quant.core.market_data import ForwardCurve
        >>> curve = ForwardCurve.flat("USD-SOFR-3M", 0.25, "2025-01-01", 0.04)
        >>> ForwardCurve.from_json(curve.to_json()) == curve
        True

        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form.

        Returns
        -------
        str
            Compact JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If the curve cannot be serialized.
        """
        ...

    def rate(self, t: TimeOrDate) -> float:
        """
        Forward rate (decimal) at a year fraction or date.

        Parameters
        ----------
        t : float, datetime.date or str
            Year fraction from ``base_date``, or a date converted with the
            curve day count.

        Returns
        -------
        float
            Forward rate as a decimal.

        Raises
        ------
        ValueError
            If a date precedes ``base_date``.
        """
        ...

    def rate_between(self, t1: float, t2: float) -> float:
        """
        Discount-factor-implied simple forward rate (decimal) over ``(t1, t2)``.

        Parameters
        ----------
        t1 : float
            Start year fraction.
        t2 : float
            End year fraction; must be finite and greater than ``t1``.

        Returns
        -------
        float
            Simple forward rate as a decimal.

        Raises
        ------
        ValueError
            If the interval is empty, reversed or non-finite.
        """
        ...

    def rate_period(self, t1: float, t2: float) -> float:
        """
        Average forward rate (decimal) over ``[t1, t2]`` from the stored knots.

        Parameters
        ----------
        t1 : float
            Start year fraction.
        t2 : float
            End year fraction.

        Returns
        -------
        float
            Period-average forward rate as a decimal.
        Notes
        -----
        This method does not raise.

        """
        ...

    def df(self, t: TimeOrDate) -> float:
        """
        Discount factor implied by compounding the forwards to a year fraction or date.

        Parameters
        ----------
        t : float, datetime.date or str
            Year fraction from ``base_date``, or a date converted with the
            curve day count.

        Returns
        -------
        float
            Unitless discount factor.

        Raises
        ------
        ValueError
            If ``t`` is negative or non-finite, or a date precedes ``base_date``.
        """
        ...

    def df_on_date_curve(self, date: DateLike) -> float:
        """
        Discount factor on a date using the curve day count.

        Parameters
        ----------
        date : datetime.date or str
            Target date on or after ``base_date``.

        Returns
        -------
        float
            Unitless discount factor.

        Raises
        ------
        ValueError
            If the year fraction cannot be computed.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export knots as a pandas ``DataFrame``.

        Returns
        -------
        pd.DataFrame
            Columns ``t`` (years) and ``forward`` (decimal); one row per knot.

        Raises
        ------
        ValueError
            If the frame cannot be built.
        """
        ...

    @property
    def id(self) -> str:
        """
        Identifier this curve is registered and looked up under.

        Returns
        -------
        str
            The curve id supplied at construction (for example ``"USD-OIS"``); market-data containers key on it exactly.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def base_date(self) -> datetime.date:
        """
        Valuation date the curve's time axis is measured from.

        Returns
        -------
        datetime.date
            The base date; a year fraction ``t`` on this curve means ``t`` years after this date under :attr:`day_count`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def tenor(self) -> float:
        """
        Accrual length of the forward rates stored on this curve.

        Returns
        -------
        float
            Tenor as a year fraction (``0.25`` for a 3M index); each forward covers a period of this length.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def knots(self) -> list[float]:
        """
        Pillar times the curve is defined on.

        Returns
        -------
        list[float]
            Strictly ascending year fractions measured from ``base_date``; interpolation happens between neighbouring entries.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def forwards(self) -> list[float]:
        """
        Projected index fixings at the curve pillars.

        Returns
        -------
        list[float]
            One simply-compounded forward rate per knot as a decimal (``0.045`` is 4.5%), covering :attr:`tenor` years from that knot.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def day_count(self) -> str:
        """
        Day-count convention converting calendar dates to curve time.

        Returns
        -------
        str
            Lower-case ISDA day-count label such as ``"act_360"`` or ``"act_365f"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def interp_style(self) -> str:
        """
        Interpolation scheme used between pillars.

        Returns
        -------
        str
            Style label such as ``"linear"`` or ``"log_linear"``, fixing how values between knots are produced.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def extrapolation(self) -> str:
        """
        Policy applied beyond the first and last pillar.

        Returns
        -------
        str
            Policy label such as ``"flat_forward"``, ``"flat_zero"`` or ``"none"``; it governs queries outside the knot range.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def projection_grid(self) -> Optional[list[float]]:
        """
        Explicit period boundaries the index projects on.

        Returns
        -------
        Optional[list[float]]
            Ascending year fractions defining contractual projection periods, or ``None`` to step forward by :attr:`tenor` instead.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def reset_lag(self) -> int:
        """
        Settlement lag between a fixing and the start of accrual.

        Returns
        -------
        int
            Number of business days from the fixing date to the period start (``2`` for most IBOR indices, ``0`` for overnight).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...
    def _repr_html_(self) -> Optional[str]: ...

class HazardCurve:
    """
    Credit hazard-rate curve for default-probability modelling.

    Stores piecewise-constant hazard rates ``(t, lambda)`` in years from
    ``base_date`` with ``lambda`` as an annual default intensity (decimal).
    Each ``lambda`` applies to the segment *ending* at its knot, so
    ``sp(t) = exp(-integral of lambda)``. Query methods accept a year fraction
    or a date (converted with the curve day count by Rust).

    Examples
    --------
    >>> from finstack_quant.core.market_data import HazardCurve
    >>> curve = HazardCurve("ACME-HZD", "2025-01-01", [(1.0, 0.02), (5.0, 0.03)], recovery_rate=0.4)
    >>> round(curve.sp(1.0), 6)
    0.980199
    >>> curve.knot_points
    [(1.0, 0.02), (5.0, 0.03)]

    """

    def __init__(
        self,
        id: str,
        base_date: DateLike,
        knots: Sequence[tuple[float, float]],
        *,
        recovery_rate: float,
        day_count: Optional[str] = None,
        par_spreads: Optional[Sequence[tuple[float, float]]] = None,
        interp: Optional[str] = None,
        par_interp: Optional[str] = None,
        issuer: Optional[str] = None,
        seniority: Optional[str] = None,
        currency: Optional[Union[Currency, str]] = None,
        max_hazard_rate: Optional[float] = None,
    ) -> None:
        """
        Construct a hazard curve from ``(time_years, hazard_rate)`` knots.

        Parameters
        ----------
        id : str
            Unique curve identifier (e.g. ``"ACME-HZD"``).
        base_date : datetime.date or str
            Valuation date anchoring ``t = 0``.
        knots : Sequence[tuple[float, float]]
            ``(time_years, hazard_rate)`` pairs; hazard rates are annual
            default intensities as decimals (``0.02`` is 2% per year).
        recovery_rate : float
            Recovery on default as a decimal fraction in ``[0, 1]`` (keyword-only).
        day_count : str, optional
            Day-count label; default ``"act_365f"``.
        par_spreads : Sequence[tuple[float, float]], optional
            ``(time_years, par_spread_bp)`` market quotes in **basis points**
            kept for reporting and re-bootstrap risk.
        interp : str, optional
            Survival-probability interpolation; only ``"log_linear"`` is supported
            to preserve the piecewise-constant hazard representation.
        par_interp : str, optional
            Par-spread readout interpolation: ``"linear"`` (default) or ``"log_linear"``.
        issuer : str, optional
            Issuer name metadata.
        seniority : str, optional
            ``"senior_secured"``, ``"senior"``, ``"subordinated"`` or ``"junior"``.
        currency : Currency or str, optional
            Currency of the protection leg (metadata).
        max_hazard_rate : float, optional
            Sanity ceiling on any hazard rate; default ``10.0``.

        Raises
        ------
        ValueError
            If a knot is non-finite, negative, duplicated or above
            ``max_hazard_rate``, ``recovery_rate`` is outside ``[0, 1]``,
            survival interpolation is not log-linear, or a label is unknown.
        TypeError
            If ``recovery_rate`` is omitted.

        Examples
        --------
        >>> from finstack_quant.core.market_data import HazardCurve
        >>> curve = HazardCurve("ACME-HZD", "2025-01-01", [(1.0, 0.02)], recovery_rate=0.4, seniority="senior")
        >>> curve.seniority
        'senior'

        """
        ...

    @staticmethod
    def flat(id: str, base_date: DateLike, hazard_rate: float, recovery_rate: float) -> HazardCurve:
        """
        Construct a flat (constant-intensity) hazard curve.

        Parameters
        ----------
        id : str
            Unique curve identifier.
        base_date : datetime.date or str
            Valuation date anchoring ``t = 0``.
        hazard_rate : float
            Constant annual default intensity as a decimal (``0.02`` is 2%).
        recovery_rate : float
            Recovery on default as a decimal fraction in ``[0, 1]``.

        Returns
        -------
        HazardCurve
            Curve with ``sp(t) == exp(-hazard_rate * t)``.

        Raises
        ------
        ValueError
            If ``hazard_rate`` is non-finite or negative, or ``recovery_rate``
            is outside ``[0, 1]``.

        Examples
        --------
        >>> from finstack_quant.core.market_data import HazardCurve
        >>> round(HazardCurve.flat("ACME", "2025-01-01", 0.02, 0.4).sp(5.0), 6)
        0.904837

        """
        ...

    @staticmethod
    def from_survival_probs(
        id: str,
        base_date: DateLike,
        points: Sequence[tuple[float, float]],
        recovery_rate: float,
    ) -> HazardCurve:
        """
        Construct a hazard curve from survival-probability pillars.

        Parameters
        ----------
        id : str
            Unique curve identifier.
        base_date : datetime.date or str
            Valuation date anchoring ``t = 0``.
        points : Sequence[tuple[float, float]]
            ``(time_years, survival_probability)`` pillars with probabilities
            in ``(0, 1]`` and non-increasing in time. A ``t = 0`` pillar must
            be ``1.0``.
        recovery_rate : float
            Recovery on default as a decimal fraction in ``[0, 1]``.

        Returns
        -------
        HazardCurve
            Piecewise-constant hazard curve reproducing every pillar exactly.

        Raises
        ------
        ValueError
            If ``points`` is empty, a probability is outside ``(0, 1]`` or
            increases with time, or ``recovery_rate`` is outside ``[0, 1]``.

        Examples
        --------
        >>> from finstack_quant.core.market_data import HazardCurve
        >>> curve = HazardCurve.from_survival_probs("ACME", "2025-01-01", [(1.0, 0.98), (5.0, 0.90)], 0.4)
        >>> round(curve.sp(5.0), 6)
        0.9

        """
        ...

    @staticmethod
    def from_json(json: str) -> HazardCurve:
        """
        Deserialize a curve from its canonical JSON wire form.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        HazardCurve
            The constructed hazard curve, ready for survival-probability queries.

        Raises
        ------
        ValueError
            If the JSON is malformed, has unknown fields, or fails validation.

        Examples
        --------
        >>> from finstack_quant.core.market_data import HazardCurve
        >>> curve = HazardCurve.flat("ACME", "2025-01-01", 0.02, 0.4)
        >>> HazardCurve.from_json(curve.to_json()) == curve
        True

        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form.

        Returns
        -------
        str
            Compact JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If the curve cannot be serialized.
        """
        ...

    def sp(self, t: TimeOrDate) -> float:
        """
        Survival probability at a year fraction or date.

        Parameters
        ----------
        t : float, datetime.date or str
            Year fraction from ``base_date``, or a date converted with the
            curve day count.

        Returns
        -------
        float
            Probability in ``(0, 1]``; ``1.0`` at or before ``t = 0``.

        Raises
        ------
        ValueError
            If a date precedes ``base_date``.
        """
        ...

    def hazard_rate(self, t: TimeOrDate) -> float:
        """
        Instantaneous hazard rate (decimal per year) at a year fraction or date.

        Parameters
        ----------
        t : float, datetime.date or str
            Year fraction from ``base_date``, or a date converted with the
            curve day count.

        Returns
        -------
        float
            Hazard rate of the segment containing ``t``.

        Raises
        ------
        ValueError
            If a date precedes ``base_date``.
        """
        ...

    def sp_on_date(self, date: DateLike) -> float:
        """
        Survival probability on a date using the curve day count.

        Parameters
        ----------
        date : datetime.date or str
            Target date on or after ``base_date``.

        Returns
        -------
        float
            Survival probability.

        Raises
        ------
        ValueError
            If the year fraction cannot be computed.
        """
        ...

    def hazard_rate_on_date(self, date: DateLike) -> float:
        """
        Hazard rate (decimal per year) on a date using the curve day count.

        Parameters
        ----------
        date : datetime.date or str
            Target date on or after ``base_date``.

        Returns
        -------
        float
            Hazard rate.

        Raises
        ------
        ValueError
            If the year fraction cannot be computed.
        """
        ...

    def survival_at_dates(self, dates: Sequence[DateLike]) -> list[float]:
        """
        Survival probabilities on several dates.

        Parameters
        ----------
        dates : Sequence[datetime.date or str]
            Target dates on or after ``base_date``.

        Returns
        -------
        list[float]
            One survival probability per input date, in order.

        Raises
        ------
        ValueError
            If any year fraction cannot be computed.
        """
        ...

    def default_prob(self, t1: float, t2: float) -> float:
        """
        Probability of default in ``[t1, t2]``: ``sp(t1) - sp(t2)``.

        Parameters
        ----------
        t1 : float
            Start year fraction.
        t2 : float
            End year fraction; must not precede ``t1``.

        Returns
        -------
        float
            Default probability.

        Raises
        ------
        ValueError
            If ``t2 < t1``.
        """
        ...

    def cds_quote_bp(self, t: float, method: Optional[str] = None) -> float:
        """
        Interpolated par CDS spread in **basis points** at year fraction ``t``.

        Uses the stored ``par_spreads`` quotes; with fewer than two quotes it
        falls back to a hazard-based approximation.

        Parameters
        ----------
        t : float
            Year fraction from ``base_date``.
        method : str, optional
            ``"linear"`` or ``"log_linear"``; defaults to :attr:`par_interp`.

        Returns
        -------
        float
            Spread in basis points.

        Raises
        ------
        ValueError
            If ``method`` is not a recognised label.
        """
        ...

    def with_recovery_rate(self, recovery_rate: float) -> HazardCurve:
        """
        Copy of this curve with a different recovery-rate metadata value.

        Survival probabilities are unchanged.

        Parameters
        ----------
        recovery_rate : float
            New recovery as a decimal fraction in ``[0, 1]``.

        Returns
        -------
        HazardCurve
            The constructed hazard curve, ready for survival-probability queries.

        Raises
        ------
        ValueError
            If ``recovery_rate`` is outside ``[0, 1]``.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export knots as a pandas ``DataFrame``.

        Returns
        -------
        pd.DataFrame
            Columns ``t`` (years) and ``hazard_rate`` (decimal); one row per knot.

        Raises
        ------
        ValueError
            If the frame cannot be built.
        """
        ...

    @property
    def id(self) -> str:
        """
        Identifier this curve is registered and looked up under.

        Returns
        -------
        str
            The curve id supplied at construction (for example ``"USD-OIS"``); market-data containers key on it exactly.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def base_date(self) -> datetime.date:
        """
        Valuation date the curve's time axis is measured from.

        Returns
        -------
        datetime.date
            The base date; a year fraction ``t`` on this curve means ``t`` years after this date under :attr:`day_count`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def recovery_rate(self) -> float:
        """
        Assumed recovery on the reference obligation.

        Returns
        -------
        float
            Recovery as a decimal fraction of notional (``0.4`` is 40%); loss given default is ``1 - recovery_rate``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def knot_points(self) -> list[tuple[float, float]]:
        """
        Piecewise hazard-rate term structure of the credit.

        Returns
        -------
        list[tuple[float, float]]
            ``(time_years, hazard_rate)`` pairs in ascending time; hazard rates are continuous decimal intensities per year (``0.02`` is 2%/y).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def par_spread_points(self) -> list[tuple[float, float]]:
        """
        Market CDS quotes the curve was bootstrapped from.

        Returns
        -------
        list[tuple[float, float]]
            ``(time_years, par_spread_bp)`` pairs with spreads in basis points; empty when the curve was built directly from hazard rates.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def day_count(self) -> str:
        """
        Day-count convention converting calendar dates to curve time.

        Returns
        -------
        str
            Lower-case ISDA day-count label such as ``"act_365f"`` or ``"act_360"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def currency(self) -> Optional[Currency]:
        """
        Settlement currency recorded for the credit.

        Returns
        -------
        Optional[Currency]
            ISO-4217 currency code of the protection leg, or ``None`` when the curve carries no currency metadata.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def issuer(self) -> Optional[str]:
        """
        Reference entity the hazard curve describes.

        Returns
        -------
        Optional[str]
            Free-form issuer label used for reporting and curve selection, or ``None`` when unset.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def seniority(self) -> Optional[str]:
        """
        Capital-structure seniority of the reference obligation.

        Returns
        -------
        Optional[str]
            One of ``"senior_secured"``, ``"senior"``, ``"subordinated"`` or ``"junior"``, or ``None`` when unset; it typically drives the recovery assumption.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def par_interp(self) -> str:
        """
        Interpolation applied when reading par spreads off the quotes.

        Returns
        -------
        str
            Either ``"linear"`` or ``"log_linear"``, controlling interpolation between the stored par-spread pillars.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...
    def _repr_html_(self) -> Optional[str]: ...

class BaseCorrelationCurve:
    """
    Base-correlation curve for synthetic credit index tranche pricing.

    Stores ``(detachment_pct, correlation)`` knots where detachment points are
    in **percent** of index notional (``3.0`` for a 0-3% tranche) and
    correlations are decimals in ``[0, 1]``.

    Examples
    --------
    >>> from finstack_quant.core.market_data import BaseCorrelationCurve
    >>> curve = BaseCorrelationCurve("CDX-IG", [(3.0, 0.25), (7.0, 0.40), (10.0, 0.55)])
    >>> curve.correlation(3.0)
    0.25
    >>> curve.detachment_points
    [3.0, 7.0, 10.0]

    """

    def __init__(self, id: str, knots: Sequence[tuple[float, float]]) -> None:
        """
        Construct a base-correlation curve from ``(detachment_pct, correlation)`` knots.

        Parameters
        ----------
        id : str
            Unique curve identifier (typically index name plus maturity).
        knots : Sequence[tuple[float, float]]
            ``(detachment_pct, correlation)`` pairs; detachment in percent of
            notional, correlation as a decimal in ``[0, 1]``.

        Raises
        ------
        ValueError
            If ``knots`` is empty, a correlation is outside ``[0, 1]``, or
            detachment points are not strictly increasing.

        Examples
        --------
        >>> from finstack_quant.core.market_data import BaseCorrelationCurve
        >>> BaseCorrelationCurve("CDX-IG", [(3.0, 0.25), (10.0, 0.55)]).correlations
        [0.25, 0.55]

        """
        ...

    @staticmethod
    def from_json(json: str) -> BaseCorrelationCurve:
        """
        Deserialize a curve from its canonical JSON wire form.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        BaseCorrelationCurve

        Raises
        ------
        ValueError
            If the JSON is malformed or fails validation.

        Examples
        --------
        >>> from finstack_quant.core.market_data import BaseCorrelationCurve
        >>> curve = BaseCorrelationCurve("CDX-IG", [(3.0, 0.25), (10.0, 0.55)])
        >>> BaseCorrelationCurve.from_json(curve.to_json()) == curve
        True

        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form.

        Returns
        -------
        str
            Compact JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If the curve cannot be serialized.
        """
        ...

    def correlation(self, detachment_pct: float) -> float:
        """
        Interpolated base correlation (decimal) at a detachment point.

        Parameters
        ----------
        detachment_pct : float
            Detachment point in percent of index notional.

        Returns
        -------
        float
            Base correlation as a decimal in ``[0, 1]``.
        Notes
        -----
        This method does not raise.

        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export knots as a pandas ``DataFrame``.

        Returns
        -------
        pd.DataFrame
            Columns ``detachment_pct`` and ``correlation``; one row per knot.

        Raises
        ------
        ValueError
            If the frame cannot be built.
        """
        ...

    @property
    def id(self) -> str:
        """
        Identifier this curve is registered and looked up under.

        Returns
        -------
        str
            The curve id supplied at construction (for example ``"USD-OIS"``); market-data containers key on it exactly.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def detachment_points(self) -> list[float]:
        """
        Tranche detachment grid the correlations are quoted on.

        Returns
        -------
        list[float]
            Ascending detachment points in percent of index notional (``7.0`` is the 7% point), not decimals.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def correlations(self) -> list[float]:
        """
        Base correlations quoted at each detachment point.

        Returns
        -------
        list[float]
            One decimal correlation in ``[0, 1]`` per detachment point, in the same order as :attr:`detachment_points`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def interp_style(self) -> str:
        """
        Interpolation scheme used between detachment points.

        Returns
        -------
        str
            Style label such as ``"linear"``, fixing how correlations between quoted detachment points are produced.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def extrapolation(self) -> str:
        """
        Policy applied outside the quoted detachment range.

        Returns
        -------
        str
            Policy label such as ``"flat"``, governing queries below the first or above the last detachment point.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...
    def _repr_html_(self) -> Optional[str]: ...

class CreditIndexData:
    """
    Credit index data bundle for synthetic tranche pricing.

    Groups the index hazard curve, the base-correlation curve, the number of
    constituents and the index recovery assumption. The bundle holds shared
    curve handles and has no JSON form of its own; serialize the
    :class:`MarketContext` it is inserted into instead.

    Examples
    --------
    >>> from finstack_quant.core.market_data import BaseCorrelationCurve, CreditIndexData, HazardCurve
    >>> hazard = HazardCurve.flat("CDX-IG", "2025-01-01", 0.01, 0.4)
    >>> base_corr = BaseCorrelationCurve("CDX-IG-BC", [(3.0, 0.25), (10.0, 0.55)])
    >>> data = CreditIndexData(125, 0.4, hazard, base_corr)
    >>> (data.num_constituents, data.index_credit_curve.id)
    (125, 'CDX-IG')

    """

    def __init__(
        self,
        num_constituents: int,
        recovery_rate: float,
        index_credit_curve: HazardCurve,
        base_correlation_curve: BaseCorrelationCurve,
    ) -> None:
        """
        Construct homogeneous credit index data.

        Parameters
        ----------
        num_constituents : int
            Number of names in the index (e.g. ``125`` for CDX IG).
        recovery_rate : float
            Index recovery assumption as a decimal fraction in ``[0, 1]``.
        index_credit_curve : HazardCurve
            Hazard curve for the index as a whole.
        base_correlation_curve : BaseCorrelationCurve
            Base correlations by detachment point.

        Raises
        ------
        ValueError
            If ``num_constituents`` is zero or ``recovery_rate`` is outside ``[0, 1]``.

        Examples
        --------
        >>> from finstack_quant.core.market_data import BaseCorrelationCurve, CreditIndexData, HazardCurve
        >>> hazard = HazardCurve.flat("CDX-IG", "2025-01-01", 0.01, 0.4)
        >>> base_corr = BaseCorrelationCurve("CDX-IG-BC", [(3.0, 0.25), (10.0, 0.55)])
        >>> CreditIndexData(125, 0.4, hazard, base_corr).recovery_rate
        0.4

        """
        ...

    @property
    def num_constituents(self) -> int:
        """
        Size of the index the bundle describes.

        Returns
        -------
        int
            Count of reference names in the index (``125`` for a standard CDX or iTraxx series); each carries equal weight.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def recovery_rate(self) -> float:
        """
        Recovery assumption applied uniformly across the index.

        Returns
        -------
        float
            Recovery as a decimal fraction of notional (``0.4`` is 40%) used for every constituent.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def index_credit_curve(self) -> HazardCurve:
        """
        Credit curve describing the index level of risk.

        Returns
        -------
        HazardCurve
            The :class:`HazardCurve` bootstrapped from index quotes, used to price the index and to anchor tranche pricing.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def base_correlation_curve(self) -> BaseCorrelationCurve:
        """
        Correlation skew used to price index tranches.

        Returns
        -------
        BaseCorrelationCurve
            The :class:`BaseCorrelationCurve` supplying a base correlation per detachment point.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str: ...

class PriceCurve:
    """
    Forward price curve for commodities, other price-based assets and
    volatility indices.

    Stores ``(t, forward_price)`` knots in years from ``base_date`` in
    absolute price units, or index points (vol points) when
    ``kind="vol_index"``. ``price`` accepts a year fraction or a date.

    Examples
    --------
    >>> from finstack_quant.core.market_data import PriceCurve
    >>> curve = PriceCurve("WTI", "2025-01-01", [(0.0, 70.0), (1.0, 72.0)])
    >>> curve.price(0.5)
    71.0
    >>> vix = PriceCurve("VIX", "2025-01-01", [(0.0, 18.0), (1.0, 21.0)], kind="vol_index")
    >>> (vix.kind, vix.spot_price)
    ('vol_index', 18.0)

    """

    def __init__(
        self,
        id: str,
        base_date: DateLike,
        knots: Sequence[tuple[float, float]],
        *,
        kind: Optional[str] = None,
        spot_price: Optional[float] = None,
        extrapolation: Optional[str] = None,
        interp: Optional[str] = None,
        day_count: Optional[str] = None,
    ) -> None:
        """
        Construct a price curve from ``(time_years, forward_price)`` knots.

        Parameters
        ----------
        id : str
            Unique curve identifier (e.g. ``"WTI-FORWARD"`` or ``"VIX"``).
        base_date : datetime.date or str
            Valuation date anchoring ``t = 0``.
        knots : Sequence[tuple[float, float]]
            ``(time_years, forward_price)`` pairs in absolute price units. At
            least two knots; the first must be at ``t = 0`` unless
            ``spot_price`` is given.
        kind : str, optional
            ``"price"`` (default; signed prices allowed) or ``"vol_index"``
            (non-negative volatility-index levels in vol points, e.g. ``18.0``).
            A ``"vol_index"`` curve is stored and retrieved through
            :meth:`MarketContext.get_vol_index_curve`.
        spot_price : float, optional
            Spot level at ``t = 0``; inferred from a ``t = 0`` knot when omitted.
        extrapolation : str, optional
            Extrapolation policy; default ``"flat_zero"``.
        interp : str, optional
            Interpolation style; default ``"linear"``.
        day_count : str, optional
            Day-count label; default ``"act_365f"``.

        Raises
        ------
        ValueError
            If fewer than two knots are given, a knot is non-finite or
            duplicated, spot cannot be inferred, a vol-index level is
            negative, or a label is unknown.

        Examples
        --------
        >>> from finstack_quant.core.market_data import PriceCurve
        >>> PriceCurve("WTI", "2025-01-01", [(0.0, 70.0), (1.0, 72.0)], spot_price=69.5).spot_price
        69.5

        """
        ...

    @staticmethod
    def from_json(json: str) -> PriceCurve:
        """
        Deserialize a curve from its canonical JSON wire form.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        PriceCurve
            The constructed price curve, ready for forward-price queries.

        Raises
        ------
        ValueError
            If the JSON is malformed, has unknown fields, or fails validation.

        Examples
        --------
        >>> from finstack_quant.core.market_data import PriceCurve
        >>> curve = PriceCurve("WTI", "2025-01-01", [(0.0, 70.0), (1.0, 72.0)])
        >>> PriceCurve.from_json(curve.to_json()) == curve
        True

        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form.

        Returns
        -------
        str
            Compact JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If the curve cannot be serialized.
        """
        ...

    def price(self, t: TimeOrDate) -> float:
        """
        Forward price (or vol-index level) at a year fraction or date.

        Parameters
        ----------
        t : float, datetime.date or str
            Year fraction from ``base_date``, or a date converted with the
            curve day count.

        Returns
        -------
        float
            Price in the curve's units.

        Raises
        ------
        ValueError
            If a date precedes ``base_date``.
        """
        ...

    def price_on_date(self, date: DateLike) -> float:
        """
        Forward price on a date using the curve day count.

        Parameters
        ----------
        date : datetime.date or str
            Target date on or after ``base_date``.

        Returns
        -------
        float
            Price in the curve's units.

        Raises
        ------
        ValueError
            If the year fraction cannot be computed.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export knots as a pandas ``DataFrame``.

        Returns
        -------
        pd.DataFrame
            Columns ``t`` (years) and ``price``; one row per knot.

        Raises
        ------
        ValueError
            If the frame cannot be built.
        """
        ...

    @property
    def id(self) -> str:
        """
        Identifier this curve is registered and looked up under.

        Returns
        -------
        str
            The curve id supplied at construction (for example ``"USD-OIS"``); market-data containers key on it exactly.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def base_date(self) -> datetime.date:
        """
        Valuation date the curve's time axis is measured from.

        Returns
        -------
        datetime.date
            The base date; a year fraction ``t`` on this curve means ``t`` years after this date under :attr:`day_count`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def kind(self) -> str:
        """
        What the stored forward values represent.

        Returns
        -------
        str
            Either ``"price"`` (forward prices in currency units) or ``"vol_index"`` (index levels in volatility points).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def spot_price(self) -> float:
        """
        Level of the underlying on the base date.

        Returns
        -------
        float
            Spot price in the curve's currency, or the index level in volatility points when :attr:`kind` is ``"vol_index"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def knots(self) -> list[float]:
        """
        Pillar times the curve is defined on.

        Returns
        -------
        list[float]
            Strictly ascending year fractions measured from ``base_date``; interpolation happens between neighbouring entries.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def prices(self) -> list[float]:
        """
        Forward levels at the curve pillars.

        Returns
        -------
        list[float]
            One forward per knot, in absolute price units or volatility points depending on :attr:`kind`, in :attr:`knots` order.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def day_count(self) -> str:
        """
        Day-count convention converting calendar dates to curve time.

        Returns
        -------
        str
            Lower-case ISDA day-count label such as ``"act_365f"`` or ``"act_360"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def interp_style(self) -> str:
        """
        Interpolation scheme used between pillars.

        Returns
        -------
        str
            Style label such as ``"linear"`` or ``"log_linear"``, fixing how values between knots are produced.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def extrapolation(self) -> str:
        """
        Policy applied beyond the first and last pillar.

        Returns
        -------
        str
            Policy label such as ``"flat_zero"`` or ``"flat_forward"``, governing queries outside the knot range.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...
    def _repr_html_(self) -> Optional[str]: ...

class InflationCurve:
    """
    CPI inflation curve for inflation-linked pricing and breakeven analysis.

    Stores ``(t, cpi_level)`` knots in years from ``base_date`` as absolute
    index levels (e.g. ``300.0``). ``cpi`` accepts a year fraction or a date.

    Examples
    --------
    >>> from finstack_quant.core.market_data import InflationCurve
    >>> curve = InflationCurve("US-CPI", "2025-01-01", 300.0, [(0.0, 300.0), (1.0, 306.0), (2.0, 312.0)])
    >>> round(curve.inflation_rate(0.0, 1.0), 4)
    0.02
    >>> curve.cpi_levels
    [300.0, 306.0, 312.0]

    """

    def __init__(
        self,
        id: str,
        base_date: DateLike,
        base_cpi: float,
        knots: Sequence[tuple[float, float]],
        *,
        day_count: Optional[str] = None,
        indexation_lag_months: Optional[int] = None,
        interp: Optional[str] = None,
        extrapolation: Optional[str] = None,
    ) -> None:
        """
        Construct an inflation curve from CPI knot points.

        Parameters
        ----------
        id : str
            Unique curve identifier (e.g. ``"US-CPI"``).
        base_date : datetime.date or str
            Valuation date anchoring ``t = 0``.
        base_cpi : float
            Finite, strictly positive reference CPI level at ``t = 0`` used by
            :meth:`index_ratio`; must equal any zero-time CPI knot.
        knots : Sequence[tuple[float, float]]
            ``(time_years, cpi_level)`` pairs; levels must be positive.
        day_count : str, optional
            Day-count label; default ``"act_365f"``.
        indexation_lag_months : int, optional
            Indexation lag in months applied by :meth:`cpi_with_lag`; default ``3``.
        interp : str, optional
            Interpolation style; default ``"log_linear"``.
        extrapolation : str, optional
            Extrapolation policy; default ``"flat_forward"``.

        Raises
        ------
        ValueError
            If no knots are given, a knot is non-finite, duplicated or
            non-positive, base CPI is non-finite/non-positive or differs from
            a zero-time knot, or a label is unknown.

        Examples
        --------
        >>> from finstack_quant.core.market_data import InflationCurve
        >>> curve = InflationCurve("US-CPI", "2025-01-01", 300.0, [(0.0, 300.0), (1.0, 306.0)], indexation_lag_months=2)
        >>> curve.indexation_lag_months
        2

        """
        ...

    @staticmethod
    def from_json(json: str) -> InflationCurve:
        """
        Deserialize a curve from its canonical JSON wire form.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        InflationCurve
            The constructed inflation curve, ready for CPI and uplift queries.

        Raises
        ------
        ValueError
            If the JSON is malformed, has unknown fields, or fails validation.

        Examples
        --------
        >>> from finstack_quant.core.market_data import InflationCurve
        >>> curve = InflationCurve("US-CPI", "2025-01-01", 300.0, [(0.0, 300.0), (1.0, 306.0)])
        >>> InflationCurve.from_json(curve.to_json()) == curve
        True

        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form.

        Returns
        -------
        str
            Compact JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If the curve cannot be serialized.
        """
        ...

    def cpi(self, t: TimeOrDate) -> float:
        """
        CPI level at a year fraction or date, without indexation lag.

        Parameters
        ----------
        t : float, datetime.date or str
            Year fraction from ``base_date``, or a date converted with the
            curve day count.

        Returns
        -------
        float
            Absolute CPI index level.

        Raises
        ------
        ValueError
            If a date precedes ``base_date``.
        """
        ...

    def cpi_on_date(self, date: DateLike) -> float:
        """
        CPI level on a date using the curve day count (no indexation lag).

        Parameters
        ----------
        date : datetime.date or str
            Target date on or after ``base_date``.

        Returns
        -------
        float
            Absolute CPI index level.

        Raises
        ------
        ValueError
            If the year fraction cannot be computed.
        """
        ...

    def cpi_with_lag(self, t: float) -> float:
        """
        CPI level at year fraction ``t`` with the configured indexation lag applied.

        Parameters
        ----------
        t : float
            Year fraction from ``base_date``.

        Returns
        -------
        float
            Lagged CPI index level.
        Notes
        -----
        This method does not raise.

        """
        ...

    def index_ratio(self, t: float) -> float:
        """
        Principal indexation ratio ``cpi_with_lag(t) / base_cpi`` at year fraction ``t``.

        No deflation floor is applied.

        Parameters
        ----------
        t : float
            Year fraction from ``base_date``.

        Returns
        -------
        float
            Indexation ratio.

        Raises
        ------
        ValueError
            If ``base_cpi`` is not strictly positive.
        """
        ...

    def inflation_rate(self, t1: float, t2: float) -> float:
        """
        Annualized inflation rate (decimal, CAGR) between ``t1`` and ``t2``.

        Parameters
        ----------
        t1 : float
            Finite start time in years from the curve base date.
        t2 : float
            Finite end time in years, strictly greater than ``t1``.

        Returns
        -------
        float
            Annualized inflation rate as a decimal.
        Raises
        ------
        ValueError
            If times are non-finite or not increasing, CPI levels are
            non-positive/non-finite, or the annualized rate is non-finite.

        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export knots as a pandas ``DataFrame``.

        Returns
        -------
        pd.DataFrame
            Columns ``t`` (years) and ``cpi``; one row per knot.

        Raises
        ------
        ValueError
            If the frame cannot be built.
        """
        ...

    @property
    def id(self) -> str:
        """
        Identifier this curve is registered and looked up under.

        Returns
        -------
        str
            The curve id supplied at construction (for example ``"USD-OIS"``); market-data containers key on it exactly.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def base_date(self) -> datetime.date:
        """
        Valuation date the curve's time axis is measured from.

        Returns
        -------
        datetime.date
            The base date; a year fraction ``t`` on this curve means ``t`` years after this date under :attr:`day_count`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def day_count(self) -> str:
        """
        Day-count convention converting calendar dates to curve time.

        Returns
        -------
        str
            Lower-case ISDA day-count label such as ``"act_365f"`` or ``"act_360"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def indexation_lag_months(self) -> int:
        """
        Publication lag applied when reading the index.

        Returns
        -------
        int
            Whole months between the reference month and the published CPI print (``3`` for most inflation-linked bonds).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def base_cpi(self) -> float:
        """
        Index level the curve is normalised against.

        Returns
        -------
        float
            CPI level on the base date in index points; ratios against it give inflation uplift factors.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def knots(self) -> list[float]:
        """
        Pillar times the curve is defined on.

        Returns
        -------
        list[float]
            Strictly ascending year fractions measured from ``base_date``; interpolation happens between neighbouring entries.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def cpi_levels(self) -> list[float]:
        """
        Projected price-index path at the curve pillars.

        Returns
        -------
        list[float]
            One CPI level in index points per knot, in :attr:`knots` order; ratios of these give forward inflation.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def interp_style(self) -> str:
        """
        Interpolation scheme used between pillars.

        Returns
        -------
        str
            Style label such as ``"log_linear"`` or ``"linear"``, fixing how index levels between knots are produced.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def extrapolation(self) -> str:
        """
        Policy applied beyond the first and last pillar.

        Returns
        -------
        str
            Policy label such as ``"flat_forward"``, ``"flat_zero"`` or ``"none"``; it governs queries outside the knot range.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...
    def _repr_html_(self) -> Optional[str]: ...

class VolSurface:
    """
    Two-dimensional implied volatility surface on an expiry x strike grid.

    Volatilities are decimal annualised standard deviations (``0.20`` is 20%)
    stored row-major by expiry. The secondary axis is a strike, tenor or
    moneyness coordinate depending on ``secondary_axis``.

    Examples
    --------
    >>> from finstack_quant.core.market_data import VolSurface
    >>> surface = VolSurface("EQ-VOL", [1.0, 2.0], [90.0, 100.0, 110.0], [[0.22, 0.20, 0.21], [0.23, 0.21, 0.22]])
    >>> surface.grid_shape
    (2, 3)
    >>> round(surface.vol(1.5, 100.0), 4)
    0.205
    >>> surface.vols[0]
    [0.22, 0.2, 0.21]

    """

    def __init__(
        self,
        id: str,
        expiries: Sequence[float],
        strikes: Sequence[float],
        vols: Any,
        *,
        secondary_axis: str = "strike",
        interpolation_mode: str = "vol",
        quote_type: str = "black_lognormal",
    ) -> None:
        """
        Construct a vol surface from an expiry x strike grid.

        Parameters
        ----------
        id : str
            Unique surface identifier.
        expiries : Sequence[float]
            Strictly increasing expiry times in years.
        strikes : Sequence[float]
            Strictly increasing secondary-axis coordinates.
        vols : Sequence[float], Sequence[Sequence[float]] or numpy.ndarray
            Volatilities as decimals, either flat row-major
            (``len(expiries) * len(strikes)``) or as ``len(expiries)`` rows of
            ``len(strikes)`` values (a 2-D array is accepted).
        secondary_axis : str, optional
            ``"strike"`` (default) or ``"tenor"``.
        interpolation_mode : str, optional
            ``"vol"`` (default, bilinear in vol) or ``"total_variance"``.
        quote_type : str, optional
            ``"black_lognormal"`` (default) or ``"normal"``.

        Raises
        ------
        ValueError
            If the grid size does not match the axes, an axis is not strictly
            increasing, a vol is non-finite or negative, or a label is unknown.
        TypeError
            If ``vols`` is not a list, nested list or array-like.

        Examples
        --------
        >>> from finstack_quant.core.market_data import VolSurface
        >>> VolSurface("EQ-VOL", [1.0], [90.0, 100.0], [0.22, 0.20]).strikes
        [90.0, 100.0]

        """
        ...

    @staticmethod
    def from_json(json: str) -> VolSurface:
        """
        Deserialize a surface from its canonical JSON wire form.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        VolSurface
            The constructed volatility surface, ready for interpolated vol queries.

        Raises
        ------
        ValueError
            If the JSON is malformed, has unknown fields, or fails validation.

        Examples
        --------
        >>> from finstack_quant.core.market_data import VolSurface
        >>> surface = VolSurface("EQ-VOL", [1.0], [90.0, 100.0], [0.22, 0.20])
        >>> VolSurface.from_json(surface.to_json()) == surface
        True

        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form.

        Returns
        -------
        str
            Compact JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If the surface cannot be serialized.
        """
        ...

    def vol(self, expiry: float, strike: float) -> float:
        """
        Interpolated volatility (decimal) at an expiry / secondary-axis point.

        Bilinear in vol or in total variance according to ``interpolation_mode``.

        Parameters
        ----------
        expiry : float
            Expiry in years; must lie within the ``expiries`` range.
        strike : float
            Secondary-axis coordinate; must lie within the ``strikes`` range.

        Returns
        -------
        float
            Volatility as a decimal.

        Raises
        ------
        ValueError
            If a coordinate is non-finite or outside the grid.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the grid in long form.

        Returns
        -------
        pd.DataFrame
            Columns ``expiry``, ``strike`` and ``vol``; one row per grid node,
            expiries outer and strikes inner.

        Raises
        ------
        ValueError
            If the frame cannot be built.
        """
        ...

    @property
    def id(self) -> str:
        """
        Identifier this surface is registered and looked up under.

        Returns
        -------
        str
            The surface id supplied at construction; market-data containers key on it exactly.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def expiries(self) -> list[float]:
        """
        Option expiries the surface is quoted on.

        Returns
        -------
        list[float]
            Ascending expiry year fractions; they index the rows of the stored volatility grid.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def strikes(self) -> list[float]:
        """
        Second grid axis of the surface.

        Returns
        -------
        list[float]
            Ascending axis values whose meaning is given by :attr:`secondary_axis`: absolute strikes, swap tenors in years, or moneyness.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def vols(self) -> list[list[float]]:
        """
        Quoted volatilities on the expiry/strike grid.

        Returns
        -------
        list[list[float]]
            Row-major grid of decimal volatilities (``0.20`` is 20%) in the convention named by :attr:`quote_type`; one row per expiry.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def secondary_axis(self) -> str:
        """
        What the second axis of the grid measures.

        Returns
        -------
        str
            Either ``"strike"`` (absolute strike levels) or ``"tenor"`` (underlying swap tenor in years).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def quote_type(self) -> str:
        """
        Volatility convention the grid values are quoted in.

        Returns
        -------
        str
            Either ``"black_lognormal"`` (decimal Black vol) or ``"normal"`` (absolute Bachelier vol in the underlying rate units).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def interpolation_mode(self) -> str:
        """
        Quantity interpolated between grid nodes.

        Returns
        -------
        str
            Either ``"vol"`` (interpolate volatilities directly) or ``"total_variance"`` (interpolate ``vol**2 * T``, which is arbitrage-safer in expiry).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def grid_shape(self) -> tuple[int, int]:
        """
        Dimensions of the stored volatility grid.

        Returns
        -------
        tuple[int, int]
            ``(n_expiries, n_strikes)``: the row and column counts of the grid, useful for reshaping the flat value list.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...
    def _repr_html_(self) -> Optional[str]: ...

class FxDeltaVolSurface:
    """
    Delta-quoted FX volatility surface (ATM, 25-delta RR/BF, optional 10-delta wings).

    Uses forward delta (premium-unadjusted). Quotes are decimal vols
    (``0.08`` is 8%); risk reversals are call vol minus put vol and
    butterflies are average wing vol minus ATM.

    This type stores quotes only. Recover pillar vols with
    :func:`finstack_quant.models.volatility.get_fx_delta_pillar_vols`,
    evaluate a strike with
    :func:`finstack_quant.models.volatility.get_fx_delta_vol`, or
    materialize a strike-axis :class:`VolSurface` with
    :func:`finstack_quant.models.volatility.materialize_fx_delta_surface`.
    There is no Python quote-builder class; those helpers are the conversion
    surface.

    Examples
    --------
    >>> from finstack_quant.core.market_data import FxDeltaVolSurface
    >>> surface = FxDeltaVolSurface("EURUSD", [0.25, 1.0], [0.08, 0.09], [0.01, 0.015], [0.005, 0.007])
    >>> (surface.num_expiries, surface.rr_10d)
    (2, None)

    """

    def __init__(
        self,
        id: str,
        expiries: Sequence[float],
        atm_vols: Sequence[float],
        rr_25d: Sequence[float],
        bf_25d: Sequence[float],
        rr_10d: Optional[Sequence[float]] = None,
        bf_10d: Optional[Sequence[float]] = None,
    ) -> None:
        """
        Construct an FX delta-quoted vol surface with 25-delta wings.

        Parameters
        ----------
        id : str
            Unique surface identifier.
        expiries : Sequence[float]
            Strictly increasing positive expiry times in years.
        atm_vols : Sequence[float]
            ATM delta-neutral straddle vols per expiry (decimal, positive).
        rr_25d : Sequence[float]
            25-delta risk reversal per expiry (decimal, call vol - put vol).
        bf_25d : Sequence[float]
            25-delta butterfly per expiry (decimal, wing average - ATM).
        rr_10d : Sequence[float], optional
            10-delta risk reversal per expiry; requires ``bf_10d``.
        bf_10d : Sequence[float], optional
            10-delta butterfly per expiry; requires ``rr_10d``.

        Raises
        ------
        ValueError
            If only one of ``rr_10d`` / ``bf_10d`` is given, any vector is
            empty or mismatched in length, expiries are not strictly
            increasing and positive, or a quote is non-finite.

        Examples
        --------
        >>> from finstack_quant.core.market_data import FxDeltaVolSurface
        >>> FxDeltaVolSurface("EURUSD", [1.0], [0.09], [0.015], [0.007]).atm_vols
        [0.09]

        """
        ...

    @staticmethod
    def from_json(json: str) -> FxDeltaVolSurface:
        """
        Deserialize a surface from its canonical JSON wire form.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        FxDeltaVolSurface

        Raises
        ------
        ValueError
            If the JSON is malformed, has unknown fields, or fails validation.

        Examples
        --------
        >>> from finstack_quant.core.market_data import FxDeltaVolSurface
        >>> surface = FxDeltaVolSurface("EURUSD", [1.0], [0.09], [0.015], [0.007])
        >>> FxDeltaVolSurface.from_json(surface.to_json()) == surface
        True

        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form.

        Returns
        -------
        str
            Compact JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If the surface cannot be serialized.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export pillars as a pandas ``DataFrame``.

        Returns
        -------
        pd.DataFrame
            Columns ``expiry``, ``atm_vol``, ``rr_25d``, ``bf_25d`` and, when
            10-delta wings are stored, ``rr_10d`` and ``bf_10d``.

        Raises
        ------
        ValueError
            If the frame cannot be built.
        """
        ...

    @property
    def id(self) -> str:
        """
        Identifier this surface is registered and looked up under.

        Returns
        -------
        str
            The surface id supplied at construction; market-data containers key on it exactly.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def expiries(self) -> list[float]:
        """
        Option expiries the surface is quoted on.

        Returns
        -------
        list[float]
            Ascending expiry year fractions; they index the rows of the stored volatility grid.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def num_expiries(self) -> int:
        """
        How many expiries the surface carries.

        Returns
        -------
        int
            Count of expiry pillars; every quote list on the surface has this length.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def atm_vols(self) -> list[float]:
        """
        At-the-money volatility term structure.

        Returns
        -------
        list[float]
            One decimal delta-neutral-straddle volatility per expiry (``0.12`` is 12%), aligned with :attr:`expiries`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def rr_25d(self) -> list[float]:
        """
        25-delta skew quotes by expiry.

        Returns
        -------
        list[float]
            Call-minus-put 25-delta volatility differences as decimals, one per expiry; positive values mean calls trade above puts.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def bf_25d(self) -> list[float]:
        """
        25-delta smile curvature quotes by expiry.

        Returns
        -------
        list[float]
            Market-strangle butterfly quotes as decimals, one per expiry; they lift the 25-delta wings above the ATM level.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def rr_10d(self) -> Optional[list[float]]:
        """
        10-delta skew quotes by expiry, when supplied.

        Returns
        -------
        Optional[list[float]]
            Call-minus-put 10-delta volatility differences as decimals, one per expiry, or ``None`` when the wings were not quoted.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def bf_10d(self) -> Optional[list[float]]:
        """
        10-delta smile curvature quotes by expiry, when supplied.

        Returns
        -------
        Optional[list[float]]
            10-delta butterfly quotes as decimals, one per expiry, or ``None`` when the wings were not quoted.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...
    def _repr_html_(self) -> Optional[str]: ...

class SabrParameterData:
    """
    Calibrated SABR parameters for one vol-cube node.

    Examples
    --------
    >>> from finstack_quant.core.market_data import SabrParameterData
    >>> p = SabrParameterData(0.02, 0.5, -0.2, 0.3)
    >>> (p.alpha, p.beta, p.rho, p.nu, p.shift)
    (0.02, 0.5, -0.2, 0.3, None)

    """

    def __init__(self, alpha: float, beta: float, rho: float, nu: float, shift: Optional[float] = None) -> None:
        """
        Construct a validated SABR parameter node.

        Parameters
        ----------
        alpha : float
            Initial volatility level; strictly positive.
        beta : float
            CEV exponent in ``[0, 1]``.
        rho : float
            Forward/volatility correlation in ``(-1, 1)``.
        nu : float
            Volatility of volatility; strictly positive.
        shift : float, optional
            Displacement added to forward and strike (decimal rate units).

        Raises
        ------
        ValueError
            If any value is non-finite or outside its range.

        Examples
        --------
        >>> from finstack_quant.core.market_data import SabrParameterData
        >>> SabrParameterData(0.02, 0.5, -0.2, 0.3, shift=0.03).shift
        0.03

        """
        ...

    @staticmethod
    def from_json(json: str) -> SabrParameterData:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        SabrParameterData

        Raises
        ------
        ValueError
            If the JSON is malformed or the parameters fail validation.

        Examples
        --------
        >>> from finstack_quant.core.market_data import SabrParameterData
        >>> p = SabrParameterData(0.02, 0.5, -0.2, 0.3)
        >>> SabrParameterData.from_json(p.to_json()) == p
        True

        """
        ...

    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            ``{"alpha": ..., "beta": ..., "rho": ..., "nu": ...}`` plus ``shift`` when set.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    @property
    def alpha(self) -> float:
        """
        SABR ``alpha``: the level of instantaneous volatility.

        Returns
        -------
        float
            Strictly positive volatility scale at the forward; its units follow the ``beta`` convention (decimal lognormal at ``beta = 1``).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def beta(self) -> float:
        """
        SABR ``beta``: the backbone exponent.

        Returns
        -------
        float
            Dimensionless exponent in ``[0, 1]``; ``1.0`` gives a lognormal backbone, ``0.0`` a normal one, ``0.5`` the CIR-style middle case.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def rho(self) -> float:
        """
        SABR ``rho``: correlation between forward and volatility.

        Returns
        -------
        float
            Dimensionless correlation in ``(-1, 1)``; negative values produce a downward-sloping skew.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def nu(self) -> float:
        """
        SABR ``nu``: the volatility of the volatility process.

        Returns
        -------
        float
            Strictly positive vol-of-vol per square root of a year; it controls smile curvature.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def shift(self) -> Optional[float]:
        """
        Shift applied before the SABR expansion.

        Returns
        -------
        Optional[float]
            Displacement added to both forward and strike in the forward's rate units, allowing negative rates, or ``None`` for the unshifted model.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class VolCube:
    """
    SABR volatility cube on an expiry x tenor grid.

    Each node stores calibrated SABR parameters and the forward swap rate
    (decimal) for that expiry/tenor pair, row-major by expiry.

    Examples
    --------
    >>> from finstack_quant.core.market_data import SabrParameterData, VolCube
    >>> p = SabrParameterData(0.02, 0.5, -0.2, 0.3)
    >>> cube = VolCube("USD-SWPT", [1.0, 2.0], [5.0], [p, p], [0.03, 0.032])
    >>> (cube.grid_shape, cube.forward_at(1, 0), cube.params_at(0, 0).alpha)
    ((2, 1), 0.032, 0.02)

    """

    def __init__(
        self,
        id: str,
        expiries: Sequence[float],
        tenors: Sequence[float],
        params_row_major: Sequence[Union[SabrParameterData, dict[str, float]]],
        forwards_row_major: Sequence[float],
        interpolation_mode: str = "vol",
    ) -> None:
        """
        Construct a vol cube from row-major grid data.

        Parameters
        ----------
        id : str
            Unique cube identifier.
        expiries : Sequence[float]
            Option expiry axis in years, strictly increasing.
        tenors : Sequence[float]
            Underlying swap tenor axis in years, strictly increasing.
        params_row_major : Sequence[SabrParameterData or dict]
            ``len(expiries) * len(tenors)`` SABR nodes, row-major by expiry.
            Dicts use keys ``"alpha"``, ``"beta"``, ``"rho"``, ``"nu"`` and
            optionally ``"shift"``.
        forwards_row_major : Sequence[float]
            Forward swap rates (decimal) in the same row-major order.
        interpolation_mode : str, optional
            ``"vol"`` (default) or ``"total_variance"``.

        Raises
        ------
        ValueError
            If grid sizes do not match the axes, a node fails SABR
            validation, or a label is unknown.
        TypeError
            If a node is neither ``SabrParameterData`` nor a dict.

        Examples
        --------
        >>> from finstack_quant.core.market_data import VolCube
        >>> node = {"alpha": 0.02, "beta": 0.5, "rho": -0.2, "nu": 0.3}
        >>> VolCube("USD-SWPT", [1.0], [5.0, 10.0], [node, node], [0.03, 0.035]).grid_shape
        (1, 2)

        """
        ...

    @staticmethod
    def from_json(json: str) -> VolCube:
        """
        Deserialize a cube from its canonical JSON wire form.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        VolCube
            The constructed volatility cube, ready for node and SABR queries.

        Raises
        ------
        ValueError
            If the JSON is malformed, has unknown fields, or fails validation.

        Examples
        --------
        >>> from finstack_quant.core.market_data import VolCube
        >>> node = {"alpha": 0.02, "beta": 0.5, "rho": -0.2, "nu": 0.3}
        >>> cube = VolCube("USD-SWPT", [1.0], [5.0], [node], [0.03])
        >>> VolCube.from_json(cube.to_json()) == cube
        True

        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form.

        Returns
        -------
        str
            Compact JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If the cube cannot be serialized.
        """
        ...

    def params_at(self, exp_idx: int, tenor_idx: int) -> SabrParameterData:
        """
        SABR parameters at grid indices.

        Parameters
        ----------
        exp_idx : int
            Zero-based expiry index.
        tenor_idx : int
            Zero-based tenor index.

        Returns
        -------
        SabrParameterData
            Node parameters.

        Raises
        ------
        IndexError
            If an index is outside the grid.
        """
        ...

    def forward_at(self, exp_idx: int, tenor_idx: int) -> float:
        """
        Forward swap rate (decimal) at grid indices.

        Parameters
        ----------
        exp_idx : int
            Zero-based expiry index.
        tenor_idx : int
            Zero-based tenor index.

        Returns
        -------
        float
            Forward rate as a decimal.

        Raises
        ------
        IndexError
            If an index is outside the grid.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export nodes in long form.

        Returns
        -------
        pd.DataFrame
            Columns ``expiry``, ``tenor``, ``alpha``, ``beta``, ``rho``, ``nu``,
            ``shift`` (``NaN`` when absent) and ``forward``.

        Raises
        ------
        ValueError
            If the frame cannot be built.
        """
        ...

    @property
    def id(self) -> str:
        """
        Identifier this cube is registered and looked up under.

        Returns
        -------
        str
            The cube id supplied at construction; market-data containers key on it exactly.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def expiries(self) -> list[float]:
        """
        Swaption expiries the cube is quoted on.

        Returns
        -------
        list[float]
            Ascending expiry year fractions; they form the outer axis of the row-major node grid.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def tenors(self) -> list[float]:
        """
        Underlying swap tenors the cube is quoted on.

        Returns
        -------
        list[float]
            Ascending swap tenors in years; they form the inner axis of the row-major node grid.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def params(self) -> list[SabrParameterData]:
        """
        Calibrated SABR parameters at every cube node.

        Returns
        -------
        list[SabrParameterData]
            One :class:`SabrParameterData` per (expiry, tenor) node, row-major with expiry as the outer axis.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def forwards(self) -> list[float]:
        """
        Forward swap rate anchoring each cube node.

        Returns
        -------
        list[float]
            One decimal forward swap rate per node (``0.035`` is 3.5%), row-major in the same order as :attr:`params`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def grid_shape(self) -> tuple[int, int]:
        """
        Dimensions of the cube's node grid.

        Returns
        -------
        tuple[int, int]
            ``(n_expiries, n_tenors)``: the outer and inner axis lengths of the row-major node lists.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def interpolation_mode(self) -> str:
        """
        Quantity interpolated between grid nodes.

        Returns
        -------
        str
            Either ``"vol"`` (interpolate volatilities directly) or ``"total_variance"`` (interpolate ``vol**2 * T``, which is arbitrage-safer in expiry).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...
    def _repr_html_(self) -> Optional[str]: ...

# FX

class FxConversionPolicy:
    """
    FX conversion policy controlling when rates are sampled.

    Immutable enum-style type with class-level constants.

    Examples
    --------
    >>> from finstack_quant.core.market_data import FxConversionPolicy
    >>> str(FxConversionPolicy.from_name("cashflow_date"))
    'cashflow_date'

    """

    CASHFLOW_DATE: FxConversionPolicy
    """Use spot/forward on the cashflow date."""
    PERIOD_END: FxConversionPolicy
    """Use period end date."""
    PERIOD_AVERAGE: FxConversionPolicy
    """Use an average over the period."""

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
        >>> str(FxConversionPolicy.from_name("cashflow_date"))
        'cashflow_date'

        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class FxRateResult:
    """
    Result of an FX rate query.

    Immutable value type returned by :meth:`FxMatrix.rate`; ``float(result)``
    yields the rate.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.market_data import FxMatrix
    >>> matrix = FxMatrix()
    >>> matrix.set_quote("EUR", "USD", 1.1)
    >>> result = matrix.rate("EUR", "USD", datetime.date(2025, 1, 1))
    >>> (result.rate, result.triangulated, float(result))
    (1.1, False, 1.1)

    """

    @staticmethod
    def from_json(json: str) -> FxRateResult:
        """
        Deserialize an FX lookup result from canonical JSON.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        FxRateResult
            The reconstructed FX conversion result, including its triangulation flag.

        Raises
        ------
        ValueError
            If the JSON is malformed or has unknown fields.

        Examples
        --------
        >>> from finstack_quant.core.market_data import FxRateResult
        >>> FxRateResult.from_json('{"rate": 1.1, "triangulated": false}').rate
        1.1

        """
        ...

    def to_json(self) -> str:
        """
        Serialize this result to compact canonical JSON.

        Returns
        -------
        str
            ``{"rate": ..., "triangulated": ...}``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    @property
    def rate(self) -> float:
        """
        Rate actually used to convert between the two currencies.

        Returns
        -------
        float
            Units of quote currency per one unit of base currency (``1.10`` for EURUSD means 1 EUR buys 1.10 USD).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def triangulated(self) -> bool:
        """
        Provenance flag for the returned rate.

        Returns
        -------
        bool
            ``True`` when the rate was derived by chaining two quotes through the pivot currency, ``False`` when a direct quote was found.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export as a single-row pandas DataFrame.

        Returns
        -------
        pd.DataFrame
            Columns ``rate`` and ``triangulated``.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def __float__(self) -> float: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...
    def _repr_html_(self) -> Optional[str]: ...

class FxQuoteConvention:
    """
    USD quotation style for a market FX pair.

    **Direct** means USD is the quote currency (EURUSD, GBPUSD). **Indirect**
    means USD is the base (USDJPY, USDCAD). Non-USD crosses inherit the USD
    quotation of market CCY1 versus USD.

    Examples
    --------
    >>> from finstack_quant.core.market_data import FxQuoteConvention
    >>> str(FxQuoteConvention.from_name("direct"))
    'direct'

    """

    DIRECT: FxQuoteConvention
    """USD is the quote currency (units of USD per one unit of CCY1)."""
    INDIRECT: FxQuoteConvention
    """USD is the base currency (units of CCY2 per one USD)."""

    @classmethod
    def from_name(cls, name: str) -> FxQuoteConvention:
        """
        Parse from a string label.

        Parameters
        ----------
        name : str
            Convention label (``"direct"`` or ``"indirect"``).

        Returns
        -------
        FxQuoteConvention
            Parsed USD quotation style.

        Raises
        ------
        ValueError
            If *name* is not ``"direct"`` or ``"indirect"``.

        Examples
        --------
        >>> from finstack_quant.core.market_data import FxQuoteConvention
        >>> str(FxQuoteConvention.from_name("indirect"))
        'indirect'

        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class FxPairConvention:
    """
    Market convention for one FX pair after Bloomberg/Reuters CCY1 ordering.

    Instances come from :func:`fx_pair_convention`. ``base`` / ``quote`` are
    always market CCY1/CCY2, even when the lookup arguments were inverted.

    Examples
    --------
    >>> from finstack_quant.core.market_data import fx_pair_convention
    >>> conv = fx_pair_convention("USD", "EUR")
    >>> (conv.base.code, conv.quote.code, str(conv.usd_quotation), conv.pip_size, conv.spot_lag_days)
    ('EUR', 'USD', 'direct', 0.0001, 2)

    """

    @property
    def base(self) -> Currency:
        """
        Base currency of the market-convention pair.

        Returns
        -------
        Currency
            ISO-4217 code of CCY1: the currency whose single unit the quoted rate prices.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def quote(self) -> Currency:
        """
        Quote currency of the market-convention pair.

        Returns
        -------
        Currency
            ISO-4217 code of CCY2: the currency the rate is expressed in, per one unit of CCY1.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def usd_quotation(self) -> FxQuoteConvention:
        """
        How USD sits in the market convention for this pair.

        Returns
        -------
        FxQuoteConvention
            Quotation label: direct when USD is the quote currency (as in EURUSD), indirect when USD is the base (as in USDJPY).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def pip_size(self) -> float:
        """
        Smallest conventional price increment for the pair.

        Returns
        -------
        float
            Pip size in outright-rate units: ``0.01`` for JPY, KRW and HUF pairs and ``0.0001`` for the rest.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def spot_lag_days(self) -> int:
        """
        Conventional settlement lag from trade date to spot.

        Returns
        -------
        int
            Business days to spot: ``1`` for USDCAD and similar pairs, ``2`` for the market standard.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str: ...

def fx_market_pair(
    a: Union[Currency, str],
    b: Union[Currency, str],
) -> tuple[Currency, Currency]:
    """
    Order two currencies into the market CCY1/CCY2 pair.

    Priority is EUR > GBP > AUD > NZD > USD > other, with a stable ISO-4217
    alphabetic tie-break when both sides share the same rank.

    Parameters
    ----------
    a : Currency or str
        First currency of the unordered pair. Need not be market CCY1.
    b : Currency or str
        Second currency of the unordered pair. Need not be market CCY2.

    Returns
    -------
    tuple[Currency, Currency]
        ``(CCY1, CCY2)`` in market order. ``fx_market_pair("USD", "EUR")``
        is ``(EUR, USD)``.

    Raises
    ------
    TypeError
        If *a* or *b* is not a :class:`Currency` or ISO code string.
    ValueError
        If either value is an unrecognized currency code.

    Examples
    --------
    >>> from finstack_quant.core.market_data import fx_market_pair
    >>> [c.code for c in fx_market_pair("USD", "EUR")]
    ['EUR', 'USD']

    """
    ...

def fx_pair_convention(
    base: Union[Currency, str],
    quote: Union[Currency, str],
) -> FxPairConvention:
    """
    Market convention for an unordered currency pair.

    Returned ``base`` / ``quote`` are always the market CCY1/CCY2, even when
    the arguments are inverted.

    Parameters
    ----------
    base : Currency or str
        One currency of the pair. Orientation is ignored.
    quote : Currency or str
        The other currency of the pair. Orientation is ignored.

    Returns
    -------
    FxPairConvention
        Market CCY1/CCY2, USD quotation, pip size, and standard spot lag.

    Raises
    ------
    TypeError
        If *base* or *quote* is not a :class:`Currency` or ISO code string.
    ValueError
        If either value is an unrecognized currency code.

    Examples
    --------
    >>> from finstack_quant.core.market_data import fx_pair_convention
    >>> conv = fx_pair_convention("USD", "JPY")
    >>> (conv.base.code, conv.quote.code, str(conv.usd_quotation), conv.pip_size, conv.spot_lag_days)
    ('USD', 'JPY', 'indirect', 0.01, 2)

    """
    ...

def fx_pip_size(
    base: Union[Currency, str],
    quote: Union[Currency, str],
) -> float:
    """
    Pip size in outright-rate units for a currency pair.

    Returns ``0.01`` when either side is JPY, KRW, or HUF; otherwise
    ``0.0001``. Argument order does not matter.

    Parameters
    ----------
    base : Currency or str
        One currency of the pair. Order is not significant.
    quote : Currency or str
        The other currency of the pair. Order is not significant.

    Returns
    -------
    float
        Pip size as a decimal increment of the outright FX rate.

    Raises
    ------
    TypeError
        If *base* or *quote* is not a :class:`Currency` or ISO code string.
    ValueError
        If either value is an unrecognized currency code.

    Examples
    --------
    >>> from finstack_quant.core.market_data import fx_pip_size
    >>> fx_pip_size("USD", "JPY")
    0.01

    """
    ...

def invert_fx_rate(rate: float) -> float:
    """
    Reciprocal of a strictly positive finite FX rate.

    Parameters
    ----------
    rate : float
        Outright FX rate to invert, in quote-per-base units. Must be finite
        and strictly positive; the reciprocal must also be a valid FX rate.

    Returns
    -------
    float
        ``1 / rate`` when that reciprocal is a valid FX rate.

    Raises
    ------
    ValueError
        If *rate* is non-finite or the reciprocal is not a usable FX rate
        (overflow, zero, or negative).
    KeyError
        If *rate* is exactly zero (the reciprocal helper reports a missing
        quote rather than an invalid numeric rate).

    Examples
    --------
    >>> from finstack_quant.core.market_data import invert_fx_rate
    >>> round(invert_fx_rate(1.10), 5)
    0.90909

    """
    ...

class FxMatrix:
    """
    Foreign-exchange rate matrix for currency conversion.

    Explicit pair-global quotes take priority in either orientation, followed
    by date/policy-scoped fixings pinned with :meth:`set_quote_on`, then provider
    observations. Market-context JSON retains those distinct source roles.
    Missing pairs are triangulated through the pivot
    currency (USD by default). Matrices obtained from a
    :class:`MarketContext` share state with it.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.market_data import FxMatrix
    >>> matrix = FxMatrix.from_dict({"EURUSD": 1.1, "GBP/USD": 1.25})
    >>> round(matrix.rate("EUR", "GBP", datetime.date(2025, 1, 1)).rate, 4)
    0.88
    >>> list(matrix.quotes()["base"])
    ['EUR', 'GBP']

    """

    def __init__(self) -> None:
        """
        Create an empty FX matrix backed by an in-memory quote provider.

        Notes
        -----
        This constructor does not raise; add quotes with the mutating
        methods after construction.

        Examples
        --------
        >>> from finstack_quant.core.market_data import FxMatrix
        >>> FxMatrix()
        FxMatrix(quotes=0, pinned_quotes=0)

        """
        ...

    @staticmethod
    def from_dict(quotes: dict[str, float]) -> FxMatrix:
        """
        Build a matrix from a ``{"EURUSD": 1.1, "GBP/USD": 1.25}`` mapping.

        Parameters
        ----------
        quotes : dict[str, float]
            Keys are six-letter ISO pairs (``"EURUSD"``) or slash-separated
            pairs (``"EUR/USD"``); values are ``1 base = rate quote``.

        Returns
        -------
        FxMatrix
            The constructed FX matrix holding the supplied quotes.

        Raises
        ------
        ValueError
            If a key is not a currency pair, a code is unknown, or a rate is
            non-finite or non-positive.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.core.market_data import FxMatrix
        >>> FxMatrix.from_dict({"EURUSD": 1.1}).rate("EUR", "USD", datetime.date(2025, 1, 1)).rate
        1.1

        """
        ...

    def set_quote(
        self,
        base: Union[Currency, str],
        quote: Union[Currency, str],
        rate: float,
    ) -> None:
        """
        Set an explicit pair-global FX quote.

        Parameters
        ----------
        base : Currency or str
            Base (from) currency.
        quote : Currency or str
            Quote (to) currency.
        rate : float
            Conversion rate ``1 base = rate quote``; finite and positive.

        Raises
        ------
        ValueError
            If ``rate`` is non-finite or non-positive, or a code is unknown.
        """
        ...

    def set_quotes(
        self,
        quotes: Sequence[tuple[Union[Currency, str], Union[Currency, str], float]],
    ) -> None:
        """
        Set several explicit FX quotes atomically.

        Parameters
        ----------
        quotes : Sequence[tuple[Currency or str, Currency or str, float]]
            ``(base, quote, rate)`` triples with ``1 base = rate quote``.

        Raises
        ------
        ValueError
            If any rate is non-finite or non-positive; no quote is applied
            in that case.
        """
        ...

    def set_quote_on(
        self,
        base: Union[Currency, str],
        quote: Union[Currency, str],
        date: DateLike,
        policy: Union[FxConversionPolicy, str],
        rate: float,
    ) -> None:
        """
        Set an authoritative FX quote scoped to one date and policy.

        A pair-global quote in either orientation takes priority over this
        fixing. The fixing takes priority over provider observations.

        Parameters
        ----------
        base : Currency or str
            Base (from) currency.
        quote : Currency or str
            Quote (to) currency.
        date : datetime.date or str
            Date the pinned quote applies to.
        policy : FxConversionPolicy or str
            Conversion policy the quote is pinned under.
        rate : float
            Conversion rate ``1 base = rate quote``; finite and positive.

        Raises
        ------
        ValueError
            If ``rate`` is invalid or ``policy`` is not a recognised label.
        """
        ...

    def rate(
        self,
        base: Union[Currency, str],
        quote: Union[Currency, str],
        date: DateLike,
        policy: Optional[Union[FxConversionPolicy, str]] = None,
    ) -> FxRateResult:
        """
        Look up an FX rate, triangulating through the pivot when needed.

        Source priority precedes orientation: global quotes, pinned fixings,
        then provider observations, taking a reciprocal within each source.

        Parameters
        ----------
        base : Currency or str
            Base (from) currency.
        quote : Currency or str
            Quote (to) currency.
        date : datetime.date or str
            Applicable date for the rate.
        policy : FxConversionPolicy or str, optional
            Conversion policy; default ``"cashflow_date"``.

        Returns
        -------
        FxRateResult
            Rate and whether it was triangulated.

        Raises
        ------
        KeyError
            If no direct, inverse or triangulated quote is available.
        ValueError
            If ``policy`` is not a recognised label.
        """
        ...

    def quotes(self) -> pd.DataFrame:
        """
        Explicit pair-global quotes as a pandas ``DataFrame``.

        Date-scoped quotes set with :meth:`set_quote_on` are not included.

        Returns
        -------
        pd.DataFrame
            Columns ``base``, ``quote`` (ISO codes) and ``rate``, sorted by pair.

        Raises
        ------
        ValueError
            If the frame cannot be built.
        """
        ...

    def __repr__(self) -> str: ...

# Scalars

class ScalarTimeSeries:
    """
    Date-indexed scalar market observations with Rust-owned interpolation.

    Examples
    --------
    >>> from finstack_quant.core.market_data import ScalarTimeSeries
    >>> series = ScalarTimeSeries("SOFR", [("2025-01-01", 0.04), ("2025-01-03", 0.05)], interpolation="linear")
    >>> round(series.value_on("2025-01-02"), 4)
    0.045
    >>> (series.first_date, len(series))
    (datetime.date(2025, 1, 1), 2)

    """

    def __init__(
        self,
        id: str,
        observations: Sequence[tuple[DateLike, Union[float, int, Decimal]]],
        currency: Optional[Union[Currency, str]] = None,
        interpolation: Optional[str] = None,
    ) -> None:
        """
        Construct a scalar time series from dated observations.

        Parameters
        ----------
        id : str
            Series identifier.
        observations : Sequence[tuple[datetime.date or str, float, int or Decimal]]
            Dated values; ``Decimal`` values must round-trip through ``float``
            exactly. Dates must be unique; any order is accepted.
        currency : Currency or str, optional
            Currency tag for monetary series; ``None`` for unitless values.
        interpolation : str, optional
            ``"step"`` (default, last observation carried forward) or ``"linear"``.

        Raises
        ------
        ValueError
            If ``observations`` is empty or has duplicate dates, a value is
            non-finite, or ``interpolation`` is not a recognised label.
        TypeError
            If a value is not a float, int or ``Decimal``.

        Examples
        --------
        >>> from finstack_quant.core.market_data import ScalarTimeSeries
        >>> ScalarTimeSeries("PX", [("2025-01-01", 100.0)], currency="USD").currency.code
        'USD'

        """
        ...

    @staticmethod
    def from_json(json: str) -> ScalarTimeSeries:
        """
        Deserialize canonical Rust series state from JSON.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        ScalarTimeSeries

        Raises
        ------
        ValueError
            If the JSON is malformed or fails validation.

        Examples
        --------
        >>> from finstack_quant.core.market_data import ScalarTimeSeries
        >>> series = ScalarTimeSeries("SOFR", [("2025-01-01", 0.03)])
        >>> ScalarTimeSeries.from_json(series.to_json()).value_on("2025-01-01")
        0.03

        """
        ...

    def to_json(self) -> str:
        """
        Serialize the canonical Rust series state to JSON.

        Returns
        -------
        str
            Compact JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def value_on(self, date: DateLike) -> float:
        """
        Value on a date under the series interpolation policy.

        Parameters
        ----------
        date : datetime.date or str
            Lookup date; must lie within the observation range.

        Returns
        -------
        float
            Interpolated (or stepped) value.

        Raises
        ------
        ValueError
            If ``date`` is outside the observed range.
        """
        ...

    def value_on_exact(self, date: DateLike) -> float:
        """
        Value on an exact observation date (no interpolation).

        Parameters
        ----------
        date : datetime.date or str
            Must match a stored observation date exactly.

        Returns
        -------
        float
            Stored value.

        Raises
        ------
        KeyError
            If no observation exists on ``date``.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export as a pandas ``DataFrame`` indexed by observation date.

        Returns
        -------
        pd.DataFrame
            Column ``value`` on a ``DatetimeIndex``; only stored observations
            appear (nothing is interpolated).

        Raises
        ------
        ValueError
            If the frame cannot be built.
        """
        ...

    @property
    def id(self) -> str:
        """
        Identifier this series is registered and looked up under.

        Returns
        -------
        str
            The series id supplied at construction; market-data containers key on it exactly.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def currency(self) -> Optional[Currency]:
        """
        Currency the observations are denominated in.

        Returns
        -------
        Optional[Currency]
            ISO-4217 currency code, or ``None`` for a unitless series such as an index level or a ratio.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def interpolation(self) -> str:
        """
        How values between observation dates are produced.

        Returns
        -------
        str
            Either ``"step"`` (hold the last observation, the default) or ``"linear"`` (interpolate between neighbouring dates).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def observations(self) -> list[tuple[datetime.date, float]]:
        """
        The stored observation history.

        Returns
        -------
        list[tuple[datetime.date, float]]
            ``(date, value)`` pairs in ascending date order, with one entry per observation date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def first_date(self) -> Optional[datetime.date]:
        """
        Start of the covered history.

        Returns
        -------
        Optional[datetime.date]
            The first observation date in the series; queries before it fall under the extrapolation policy.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def last_date(self) -> Optional[datetime.date]:
        """
        End of the covered history.

        Returns
        -------
        Optional[datetime.date]
            The last observation date in the series; queries after it fall under the extrapolation policy.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __len__(self) -> int: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...
    def _repr_html_(self) -> Optional[str]: ...

class InflationIndex:
    """
    Inflation index observations with Rust-owned interpolation, lag and seasonality.

    Examples
    --------
    >>> from finstack_quant.core.market_data import InflationIndex
    >>> index = InflationIndex("US-CPI", [("2025-01-01", 300.0), ("2025-02-01", 301.5)], "USD")
    >>> (index.value_on("2025-01-15"), index.lag, index.seasonality)
    (300.0, 'none', None)

    """

    def __init__(
        self,
        id: str,
        observations: Sequence[tuple[DateLike, Union[float, int, Decimal]]],
        currency: Union[Currency, str],
        interpolation: Optional[str] = None,
        lag: Optional[Union[str, int]] = None,
        seasonality: Optional[Sequence[float]] = None,
    ) -> None:
        """
        Construct an inflation index from dated observations.

        Parameters
        ----------
        id : str
            Index identifier (e.g. ``"US-CPI-U"``).
        observations : Sequence[tuple[datetime.date or str, float, int or Decimal]]
            Dated index levels; ``Decimal`` values must round-trip through
            ``float`` exactly.
        currency : Currency or str
            Currency of the index.
        interpolation : str, optional
            ``"step"`` (default) or ``"linear"``.
        lag : str or int, optional
            Publication lag applied before lookups: ``"none"`` (default),
            market strings such as ``"3M"`` or ``"90D"``, or an integer number
            of months.
        seasonality : Sequence[float], optional
            Twelve multiplicative factors, January through December.

        Raises
        ------
        ValueError
            If ``observations`` is empty or has duplicate dates, a label is
            unknown, or ``seasonality`` does not have exactly 12 entries.

        Examples
        --------
        >>> from finstack_quant.core.market_data import InflationIndex
        >>> InflationIndex("US-CPI", [("2025-01-01", 300.0)], "USD", lag="3M").lag
        '3M'

        """
        ...

    @staticmethod
    def from_json(json: str) -> InflationIndex:
        """
        Deserialize canonical Rust inflation-index state from JSON.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        InflationIndex
            The reconstructed inflation index, with its lag, interpolation and
            seasonality settings restored.

        Raises
        ------
        ValueError
            If the JSON is malformed or fails validation.

        Examples
        --------
        >>> from finstack_quant.core.market_data import InflationIndex
        >>> index = InflationIndex("US-CPI", [("2025-01-01", 300.0)], "USD")
        >>> InflationIndex.from_json(index.to_json()).id
        'US-CPI'

        """
        ...

    def to_json(self) -> str:
        """
        Serialize the canonical Rust inflation-index state to JSON.

        Returns
        -------
        str
            Compact JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def value_on(self, date: DateLike) -> float:
        """
        Index level on a date with lag, interpolation and seasonality applied.

        Parameters
        ----------
        date : datetime.date or str
            Contract date (the lag is applied by Rust).

        Returns
        -------
        float
            Index level.

        Raises
        ------
        ValueError
            If the lag-adjusted date is outside the observation range.
        """
        ...

    def ratio(self, base_date: DateLike, settle_date: DateLike) -> float:
        """
        Indexation ratio ``value_on(settle_date) / value_on(base_date)``.

        Parameters
        ----------
        base_date : datetime.date or str
            Reference date of the base index level.
        settle_date : datetime.date or str
            Date of the uplifted level.

        Returns
        -------
        float
            Indexation ratio.

        Raises
        ------
        ValueError
            If either lag-adjusted date is outside the observation range.
        """
        ...

    def ref_cpi_months_lag(self, date: DateLike, lag_months: int) -> float:
        """
        Reference CPI for a date under an explicit month lag (bond-style lookup).

        Parameters
        ----------
        date : datetime.date or str
            Contract date.
        lag_months : int
            Calendar months to look back (``3`` for TIPS/gilts). Observations
            must be unique within each reference month; missing months are not
            interpolated. The first of the contract month needs only the first
            lagged observation.

        Returns
        -------
        float
            Reference index level.

        Raises
        ------
        KeyError
            If a required monthly CPI observation is missing.
        ValueError
            If the contract date cannot be parsed or a required month has multiple observations.
        """
        ...

    def date_range(self) -> tuple[datetime.date, datetime.date]:
        """
        ``(first_date, last_date)`` of the stored observations.

        Returns
        -------
        tuple[datetime.date, datetime.date]

        Raises
        ------
        ValueError
            If the index has no observations.
        """
        ...

    @property
    def id(self) -> str:
        """
        Identifier this inflation index is looked up under.

        Returns
        -------
        str
            The index id supplied at construction (for example a CPI series name); containers key on it exactly.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def currency(self) -> Currency:
        """
        Currency the index is published for.

        Returns
        -------
        Currency
            ISO-4217 currency code of the economy the price index measures.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def interpolation(self) -> str:
        """
        How index values between publication dates are produced.

        Returns
        -------
        str
            Either ``"step"`` (hold the last print, the usual convention for monthly CPI) or ``"linear"`` (daily interpolation between prints).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def lag(self) -> str:
        """
        Delay between the reference period and its published print.

        Returns
        -------
        str
            Lag label: ``"none"``, ``"<n>M"`` for whole months (``"3M"`` is the standard linker lag) or ``"<n>D"`` for calendar days.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def seasonality(self) -> Optional[list[float]]:
        """
        Multiplicative seasonal adjustment applied to the index.

        Returns
        -------
        Optional[list[float]]
            Twelve factors starting with January, applied multiplicatively to the interpolated index, or ``None`` when no seasonality is modelled.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def observations(self) -> list[tuple[datetime.date, float]]:
        """
        The stored observation history.

        Returns
        -------
        list[tuple[datetime.date, float]]
            ``(date, value)`` pairs in ascending date order, with one entry per observation date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __len__(self) -> int: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

# Context

class MarketContext:
    """
    Unified market data container for curves, surfaces, scalars and FX.

    Curves are stored behind shared handles, so getters are cheap. Every
    ``insert_*`` method returns ``self`` for fluent chaining; ``id in ctx``
    and ``len(ctx)`` are supported; ``pickle`` and ``to_json`` round-trip the
    full state.

    Examples
    --------
    >>> from finstack_quant.core.market_data import DiscountCurve, HazardCurve, MarketContext
    >>> ctx = (
    ...     MarketContext()
    ...     .insert(DiscountCurve.flat("USD-OIS", "2025-01-01", 0.05))
    ...     .insert(HazardCurve.flat("ACME", "2025-01-01", 0.02, 0.4))
    ... )
    >>> ("USD-OIS" in ctx, len(ctx), ctx.curve_ids())
    (True, 2, ['ACME', 'USD-OIS'])
    >>> ctx.stats()["curve_counts"]
    {'Discount': 1, 'Hazard': 1}

    """

    def __init__(self) -> None:
        """
        Create an empty market context.

        Notes
        -----
        This constructor does not raise; populate the context with the fluent
        ``insert_*`` methods after construction.

        Examples
        --------
        >>> from finstack_quant.core.market_data import MarketContext
        >>> MarketContext().is_empty()
        True

        """
        ...

    @staticmethod
    def from_json(json: str) -> MarketContext:
        """
        Deserialize a market context from a JSON string.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json` or by the calibration pipeline.

        Returns
        -------
        MarketContext
            The reconstructed context, holding every curve, surface, scalar and
            mapping present in the payload.

        Raises
        ------
        ValueError
            If the JSON is malformed or fails validation.

        Examples
        --------
        >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
        >>> ctx = MarketContext().insert(DiscountCurve.flat("USD-OIS", "2025-01-01", 0.05))
        >>> MarketContext.from_json(ctx.to_json()).get_discount("USD-OIS").id
        'USD-OIS'

        """
        ...

    def to_json(self) -> str:
        """
        Serialize this market context to compact JSON (round-trips with pricers).

        Returns
        -------
        str
            Canonical ``MarketContextState`` JSON.

        Raises
        ------
        ValueError
            If serialization fails.
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
        ],
    ) -> MarketContext:
        """
        Insert a curve or surface under its own id (fluent).

        Parameters
        ----------
        curve : DiscountCurve, ForwardCurve, HazardCurve, InflationCurve, PriceCurve, BaseCorrelationCurve, VolSurface, FxDeltaVolSurface or VolCube
            Object to store. A ``PriceCurve`` with ``kind="vol_index"`` is
            stored as a vol-index curve.

        Returns
        -------
        MarketContext
            ``self``.

        Raises
        ------
        TypeError
            If ``curve`` is not one of the supported types.
        """
        ...

    def insert_fx(self, fx: FxMatrix) -> MarketContext:
        """
        Attach an FX matrix (fluent).

        Parameters
        ----------
        fx : FxMatrix
            Matrix shared by reference; later quote updates are visible here.

        Returns
        -------
        MarketContext
            ``self``, so inserts can be chained fluently.

        Notes
        -----
        This method does not raise; any previously attached matrix is replaced.
        """
        ...

    def insert_price(
        self,
        id: str,
        value: Union[float, int, Decimal],
        currency: Optional[Union[Currency, str]] = None,
    ) -> MarketContext:
        """
        Insert a scalar market price (fluent).

        Parameters
        ----------
        id : str
            Identifier for the scalar.
        value : float, int or Decimal
            Price or unitless value. Monetary ``Decimal`` values keep full
            precision; unitless ``Decimal`` values must round-trip through ``float``.
        currency : Currency or str, optional
            When given, the scalar is a monetary price in this currency;
            otherwise it is unitless.

        Returns
        -------
        MarketContext
            ``self``.

        Raises
        ------
        ValueError
            If ``value`` is non-finite or a unitless ``Decimal`` is not
            exactly representable.
        TypeError
            If ``value`` is not numeric.
        """
        ...

    def insert_credit_index(self, id: str, data: CreditIndexData) -> MarketContext:
        """
        Insert credit index data under ``id`` (fluent).

        Parameters
        ----------
        id : str
            Identifier for the bundle (e.g. ``"CDX-IG"``); the bundle carries
            no id of its own.
        data : CreditIndexData
            Bundle to store.

        Returns
        -------
        MarketContext
            ``self``, so inserts can be chained fluently.

        Notes
        -----
        This method does not raise; an existing bundle under ``id`` is replaced.
        """
        ...

    def insert_series(self, series: ScalarTimeSeries) -> MarketContext:
        """
        Insert a scalar time series under its own id (fluent).

        Parameters
        ----------
        series : ScalarTimeSeries
            Series to store.

        Returns
        -------
        MarketContext
            ``self``, so inserts can be chained fluently.

        Notes
        -----
        This method does not raise; an existing series with the same id is
        replaced.
        """
        ...

    def insert_inflation_index(self, index: InflationIndex) -> MarketContext:
        """
        Insert an inflation index under its own id (fluent).

        Parameters
        ----------
        index : InflationIndex
            Index to store.

        Returns
        -------
        MarketContext
            ``self``, so inserts can be chained fluently.

        Notes
        -----
        This method does not raise; an existing index with the same id is
        replaced.
        """
        ...

    def map_collateral(self, csa_code: str, discount_id: str) -> MarketContext:
        """
        Map a CSA code to a discount curve id for collateral discounting (fluent).

        Parameters
        ----------
        csa_code : str
            Collateral agreement identifier (e.g. ``"USD-CSA"``).
        discount_id : str
            Id of a discount curve in this context.

        Returns
        -------
        MarketContext
            ``self``, so mappings can be chained fluently.

        Notes
        -----
        This method does not raise; the curve id is resolved lazily when the
        mapping is used, and an existing mapping for ``csa_code`` is replaced.
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
            The constructed discount curve, ready for ``df`` and zero-rate queries.

        Raises
        ------
        KeyError
            If no curve is stored under ``id``.
        ValueError
            If the stored curve is not a discount curve.
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
            The constructed forward curve, ready for index projection.

        Raises
        ------
        KeyError
            If no curve is stored under ``id``.
        ValueError
            If the stored curve is not a forward curve.
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
            The constructed hazard curve, ready for survival-probability queries.

        Raises
        ------
        KeyError
            If no curve is stored under ``id``.
        ValueError
            If the stored curve is not a hazard curve.
        """
        ...

    def get_base_correlation(self, id: str) -> BaseCorrelationCurve:
        """
        Retrieve a base-correlation curve by identifier.

        Parameters
        ----------
        id : str
            Curve identifier.

        Returns
        -------
        BaseCorrelationCurve

        Raises
        ------
        KeyError
            If no curve is stored under ``id``.
        ValueError
            If the stored curve is not a base-correlation curve.
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
            The constructed inflation curve, ready for CPI and uplift queries.

        Raises
        ------
        KeyError
            If no curve is stored under ``id``.
        ValueError
            If the stored curve is not an inflation curve.
        """
        ...

    def get_price_curve(self, id: str) -> PriceCurve:
        """
        Retrieve a price curve (``kind="price"``) by identifier.

        Parameters
        ----------
        id : str
            Curve identifier.

        Returns
        -------
        PriceCurve
            The constructed price curve, ready for forward-price queries.

        Raises
        ------
        KeyError
            If no curve is stored under ``id``.
        ValueError
            If the stored curve is not a price curve.
        """
        ...

    def get_vol_index_curve(self, id: str) -> PriceCurve:
        """
        Retrieve a volatility-index curve (``PriceCurve`` with ``kind="vol_index"``).

        Parameters
        ----------
        id : str
            Curve identifier.

        Returns
        -------
        PriceCurve
            Curve whose ``kind`` is ``"vol_index"``.

        Raises
        ------
        KeyError
            If no curve is stored under ``id``.
        ValueError
            If the stored curve is not a vol-index curve.
        """
        ...

    def get_price(self, id: str) -> tuple[Union[float, Decimal], Optional[str]]:
        """
        Retrieve a scalar market price as ``(value, currency)``.

        Parameters
        ----------
        id : str
            Scalar identifier.

        Returns
        -------
        tuple[float or Decimal, str or None]
            Currency-tagged values return a lossless ``Decimal`` and ISO code;
            unitless values return a ``float`` and ``None``.

        Raises
        ------
        KeyError
            If no scalar is stored under ``id``.
        """
        ...

    def get_series(self, id: str) -> ScalarTimeSeries:
        """
        Retrieve a scalar time series by identifier.

        Parameters
        ----------
        id : str
            Series identifier.

        Returns
        -------
        ScalarTimeSeries

        Raises
        ------
        KeyError
            If no series is stored under ``id``.
        """
        ...

    def get_inflation_index(self, id: str) -> InflationIndex:
        """
        Retrieve an inflation index by identifier.

        Parameters
        ----------
        id : str
            Index identifier.

        Returns
        -------
        InflationIndex
            The stored inflation index registered under that identifier.

        Raises
        ------
        KeyError
            If no index is stored under ``id``.
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
            The constructed volatility surface, ready for interpolated vol queries.

        Raises
        ------
        KeyError
            If no surface is stored under ``id``.
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
            If no surface is stored under ``id``.
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
            The constructed volatility cube, ready for node and SABR queries.

        Raises
        ------
        KeyError
            If no cube is stored under ``id``.
        """
        ...

    def get_credit_index(self, id: str) -> CreditIndexData:
        """
        Retrieve credit-index data by identifier.

        Parameters
        ----------
        id : str
            Bundle identifier.

        Returns
        -------
        CreditIndexData
            The stored credit-index bundle for that identifier.

        Raises
        ------
        KeyError
            If no bundle is stored under ``id``.
        """
        ...

    @property
    def fx(self) -> Optional[FxMatrix]:
        """
        FX quotes available for currency conversion.

        Returns
        -------
        Optional[FxMatrix]
            The attached :class:`FxMatrix`, or ``None`` when the container holds no FX data and cross-currency conversion is unavailable.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def fx_required(self) -> FxMatrix:
        """
        Attached FX matrix, raising if none is present.

        Returns
        -------
        FxMatrix
            The constructed FX matrix holding the supplied quotes.

        Raises
        ------
        KeyError
            If no FX matrix has been inserted.
        """
        ...

    def convert_money(
        self,
        amount: Money,
        target_currency: Union[Currency, str],
        as_of: DateLike,
    ) -> Money:
        """
        Convert a monetary amount into another currency with the attached FX matrix.

        Same-currency amounts are returned unchanged without consulting FX.

        Parameters
        ----------
        amount : Money
            Amount to convert.
        target_currency : Currency or str
            Destination currency.
        as_of : datetime.date or str
            Date used for the FX rate lookup.

        Returns
        -------
        Money
            Converted amount in ``target_currency``.

        Raises
        ------
        KeyError
            If no FX matrix is attached or the pair cannot be resolved.
        """
        ...

    def contains(self, id: str) -> bool:
        """
        Whether any object is stored under ``id``.

        Parameters
        ----------
        id : str
            Identifier to look up across curves, surfaces, scalars, series,
            indices, credit indices and collateral mappings.

        Returns
        -------
        bool
            ``True`` when some object is stored under that identifier in any
            of those categories, ``False`` otherwise.

        Notes
        -----
        This method does not raise; an unknown identifier returns ``False``.
        """
        ...

    def curve_ids(self) -> list[str]:
        """
        Identifiers of all stored term-structure curves.

        Returns
        -------
        list[str]
            Sorted identifiers (surfaces, scalars and series are excluded).

        Notes
        -----
        This method does not raise; an empty context returns an empty list.
        """
        ...

    def is_empty(self) -> bool:
        """
        Whether nothing has been inserted.

        Returns
        -------
        bool
            ``True`` when the context holds no curves, surfaces, scalars,
            series, indices or collateral mappings.

        Notes
        -----
        This method does not raise.
        """
        ...

    def stats(self) -> dict[str, Any]:
        """
        Counts of stored objects by category.

        Returns
        -------
        dict[str, Any]
            Keys ``curve_counts`` (dict of curve type to count),
            ``total_curves``, ``has_fx``, ``surface_count``,
            ``vol_cube_count``, ``price_count``, ``series_count``,
            ``inflation_index_count``, ``credit_index_count``,
            ``dividend_schedule_count``, ``fx_delta_vol_surface_count`` and
            ``collateral_mapping_count``.

        Raises
        ------
        RuntimeError
            If the statistics dictionary cannot be constructed.
        """
        ...

    def roll_forward(self, days: int) -> MarketContext:
        """
        Roll every dated term structure forward by ``days`` calendar days.

        Parameters
        ----------
        days : int
            Calendar days to roll (may be negative).

        Returns
        -------
        MarketContext
            New context; ``self`` is unchanged.

        Raises
        ------
        ValueError
            If a curve cannot be rebuilt after rolling.
        """
        ...

    def __contains__(self, id: str) -> bool: ...
    def __len__(self) -> int: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

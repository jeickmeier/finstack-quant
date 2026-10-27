"""Stub declarations mirroring ``finstack-quant-py/src/bindings/core/market_data/curves/forward.rs``."""

from __future__ import annotations

import datetime
from typing import Any, Optional, Sequence, Union
import pandas as pd
from finstack_quant.core.dates import DayCount
from finstack_quant.core.market_data import DateLike, TimeOrDate

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
        day_count: Optional[Union[DayCount, str]] = None,
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
        day_count : DayCount | str, optional
            Day-count label (``"act_360"``, ``"act_365f"``, ...). When
            omitted, Rust infers a market default from ``id``.
        interp : str, optional
            Interpolation style; default ``"linear"``.
        extrapolation : str, optional
            Extrapolation policy; default ``"flat_zero"``, which holds the
            first and last forward rate flat (market-standard projection-curve
            tail). ``"flat_forward"`` extends the boundary rate slope instead.
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

        The stored rate is annualized on the curve's day-count basis. Convert
        its accrual growth to the contractual index basis before calculating a
        coupon with a different accrual convention. Rust instrument pricing
        performs this conversion from the contractual schedule.

        Parameters
        ----------
        t : float, datetime.date or str
            Year fraction from ``base_date``, or a date converted with the
            curve day count; dates on or before ``base_date`` read the rate at
            curve time zero.

        Returns
        -------
        float
            Simple forward rate as a decimal, annualized on the curve day count.

        Raises
        ------
        ValueError
            If the date's day-count fraction cannot be computed.
        """
        ...

    def rate_between(self, t1: float, t2: float) -> float:
        """
        Discount-factor-implied simple forward rate (decimal) over ``(t1, t2)``.

        The denominator is the curve-time span ``t2 - t1``. To express the
        implied growth on another contractual index basis, multiply by that
        span and divide by the index accrual fraction for the same dated period.

        Parameters
        ----------
        t1 : float
            Start year fraction from the curve base date using its day count.
        t2 : float
            End year fraction on the same curve basis; must be finite and greater than ``t1``.

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

        This integral average remains annualized on the curve's day count.
        For an overnight observation on another index basis, multiply by
        ``t2 - t1`` and divide by its contractual observation accrual fraction.
        Use ``rate_between`` for a simple term-period rate implied by projection DFs.

        Parameters
        ----------
        t1 : float
            Start year fraction from the curve base date using its day count.
        t2 : float
            End year fraction on the same curve basis.

        Returns
        -------
        float
            Finite period-average rate as a decimal. Equal endpoints and intervals
            no longer than 1e-12 years return the start rate. Negative finite times
            retain the curve's extrapolation policy.

        Raises
        ------
        ValueError
            If endpoints are non-finite or reversed, or the interval length or result is non-finite.
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

    @property
    def rate_calibration(self) -> Optional[dict[str, Any]]:
        """
        Calibration recipe stamped on this curve by the calibration engine.

        The dict is the serde form of the Rust ``RateCalibrationRecipe``, the
        same object found under ``"rate_calibration"`` in :meth:`to_json`:
        ``currency``, ``method`` (bootstrap or global solve), ``curve_day_count``,
        ``ois_compounding``, ``role`` (discount, or projection with its linked
        discount curve id) and ``quotes``, the complete typed quote set with
        rates and spreads as decimals and futures as prices.

        Returns
        -------
        Optional[dict[str, Any]]
            The recipe, or ``None`` for a curve that was built directly rather than
            produced by a calibration step.

        Raises
        ------
        ValueError
            If the recipe cannot be converted to Python objects.
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...
    def _repr_html_(self) -> Optional[str]: ...

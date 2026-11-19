"""Stub declarations mirroring ``finstack-quant-py/src/bindings/core/market_data/curves/forward.rs``."""

from __future__ import annotations

import datetime
from typing import Any, NotRequired, Optional, Sequence, TypedDict, Union
import pandas as pd
from finstack_quant.core.dates import DayCount, Tenor
from finstack_quant.core.market_data import DateLike, TimeOrDate
from finstack_quant.core.market_data._curves.discount import DiscountCurve

class _ForwardCurveOptions(TypedDict):
    id: str
    base_date: DateLike
    knots: Sequence[tuple[float, float]]
    interp: str
    extrapolation: str
    day_count: Union[DayCount, str]
    calendar_id: NotRequired[Optional[str]]
    index_tenor: NotRequired[Optional[Union[Tenor, str]]]

class ForwardCurve:
    """
    Projection-factor curve for one floating-rate index.

    Stores strictly positive projection factors ``P_x(t)`` at knot times in
    years from ``base_date``, with ``P_x(0) = 1``. The growth
    ``P_x(t1) / P_x(t2)`` over an index period drives its projected fixing;
    :meth:`rate_between` converts it to a simple rate on an explicit
    contractual accrual fraction. :meth:`df` accepts a year fraction or a
    date; dates are converted with the curve day count by Rust. Instances are
    immutable; equality compares the canonical JSON wire form.

    Examples
    --------
    >>> import math
    >>> from finstack_quant.core.market_data import ForwardCurve
    >>> curve = ForwardCurve({
    ...     "id": "USD-SOFR-3M",
    ...     "base_date": "2025-01-01",
    ...     "knots": [(0.0, 1.0), (1.0, math.exp(-0.04))],
    ...     "interp": "log_linear",
    ...     "extrapolation": "extend",
    ...     "day_count": "act_360",
    ...     "index_tenor": "3M",
    ... })
    >>> round(curve.rate_between(0.0, 0.25, 0.25), 6)
    0.040201
    >>> str(curve.index_tenor)
    '3M'

    """

    def __init__(self, options: _ForwardCurveOptions) -> None:
        """
        Construct a projection-factor curve from a dictionary of explicit options.

        Parameters
        ----------
        options : dict
            Supply a unique ``id``, a ``base_date`` as a date or ISO string,
            strictly ascending ``knots`` of finite ``(time_years, factor)``
            pairs, an ``interp`` style, an ``extrapolation`` policy and a
            ``day_count`` convention or label. Factors must be positive. A
            zero-time factor must equal one, and ``(0.0, 1.0)`` is prepended
            when absent. Supported interpolation styles are ``"log_linear"``,
            ``"linear"``, ``"monotone_convex"``, ``"cubic_hermite"`` and
            ``"piecewise_quadratic_forward"``. Tail policies are ``"flat"``,
            ``"extend"`` and ``"error"``. Optional ``calendar_id`` is a registered ID
            or a ``+``-joined union and is required for ``"bus_252"``. Optional
            ``index_tenor`` accepts a ``Tenor`` or label such as ``"3M"``. It
            supplies index accrual metadata used by term-rate consumers.

        Raises
        ------
        TypeError
            If a key is missing or unknown or an option has the wrong type.
        ValueError
            If the knots are invalid, a convention label is unknown, or
            the day count requires an omitted holiday calendar.
        KeyError
            If the calendar ID or any member of a calendar union is unknown.
        """
        ...

    @staticmethod
    def flat(id: str, base_date: DateLike, continuous_rate: float, day_count: Union[DayCount, str]) -> ForwardCurve:
        """
        Construct a flat continuously-compounded projection curve.

        Parameters
        ----------
        id : str
            Unique curve identifier.
        base_date : datetime.date or str
            Valuation date anchoring ``t = 0``.
        continuous_rate : float
            Continuously-compounded projection zero rate as a decimal (``0.04`` is 4%).
        day_count : DayCount or str
            Explicit supported day-count convention label or object.

        Returns
        -------
        ForwardCurve
            Log-linear curve with ``df(t) == exp(-continuous_rate * t)``,
            ``"extend"`` tails and no :attr:`index_tenor`.

        Raises
        ------
        ValueError
            If the rate is non-finite, its one-year factor overflows or
            underflows, or ``day_count`` is unknown or requires a calendar.

        Examples
        --------
        >>> from finstack_quant.core.market_data import ForwardCurve
        >>> round(ForwardCurve.flat("USD-SOFR-3M", "2025-01-01", 0.04, "act_360").df(2.0), 6)
        0.923116

        """
        ...

    @staticmethod
    def from_discount_curve(
        id: str,
        discount: DiscountCurve,
        index_tenor: Optional[Union[Tenor, str]] = None,
    ) -> ForwardCurve:
        """
        Promote a discount curve into a projection curve without refitting.

        The forward curve reuses the discount curve's factors, interpolator,
        day count, calendar, tail policy and any retained shock, so
        :meth:`df` matches ``discount.df`` everywhere. Use it for single-curve
        setups where the index projects off the discount curve.

        Parameters
        ----------
        id : str
            Identifier of the resulting forward curve.
        discount : DiscountCurve
            Source curve whose factor state is copied; its calibration recipe
            is dropped.
        index_tenor : Tenor or str, optional
            Accrual tenor of the projected index (for example ``"3M"``);
            ``None`` leaves the curve without index-period metadata.

        Returns
        -------
        ForwardCurve
            Projection curve sharing ``discount``'s factor function.

        Raises
        ------
        ValueError
            If ``index_tenor`` is not a valid tenor label.
        TypeError
            If ``index_tenor`` is neither a ``Tenor`` nor a string.

        Examples
        --------
        >>> from finstack_quant.core.market_data import DiscountCurve, ForwardCurve
        >>> ois = DiscountCurve.flat("USD-OIS", "2025-01-01", 0.04, "act_365f")
        >>> fwd = ForwardCurve.from_discount_curve("USD-SOFR-3M", ois, "3M")
        >>> fwd.df(2.0) == ois.df(2.0)
        True

        """
        ...

    @staticmethod
    def from_zero_rates(
        id: str,
        base_date: DateLike,
        points: Sequence[tuple[float, float]],
        compounding: str,
        day_count: Union[DayCount, str],
        interp: str,
        extrapolation: str,
    ) -> ForwardCurve:
        """
        Construct a forward curve from zero-rate pillars.

        Parameters
        ----------
        id : str
            Unique curve identifier.
        base_date : datetime.date or str
            Valuation date anchoring ``t = 0``.
        points : Sequence[tuple[float, float]]
            Strictly ascending ``(time_years, zero_rate)`` pillars with rates
            as decimals. A ``(0, 1.0)`` factor anchor is added when no ``t = 0``
            pillar is given.
        compounding : str
            ``"continuous"``, ``"simple"``, ``"periodic(1)"``,
            ``"semi_annual"``, ``"quarterly"`` or ``"monthly"``.
        day_count : DayCount or str
            Explicit curve day-count convention.
        interp : str
            Explicit interpolation style.
        extrapolation : str
            Explicit tail extrapolation policy (``"flat"``, ``"extend"``, or
            ``"error"``).

        Returns
        -------
        ForwardCurve
            Curve built with the specified explicit conventions.

        Raises
        ------
        ValueError
            If ``points`` is empty, unsorted or non-finite, a label is
            unknown, or projection-factor construction fails.

        Examples
        --------
        >>> from finstack_quant.core.market_data import ForwardCurve
        >>> curve = ForwardCurve.from_zero_rates(
        ...     "USD-SOFR-3M",
        ...     "2025-01-01",
        ...     [(1.0, 0.04), (2.0, 0.04)],
        ...     "periodic(1)",
        ...     "act_360",
        ...     "log_linear",
        ...     "extend",
        ... )
        >>> round(curve.df(2.0), 6)
        0.924556

        """
        ...

    @staticmethod
    def from_dates(
        id: str,
        base_date: DateLike,
        points: Sequence[tuple[DateLike, float]],
        day_count: Union[DayCount, str],
        interp: str,
        extrapolation: str,
    ) -> ForwardCurve:
        """
        Construct a forward curve from dated projection-factor pillars.

        Parameters
        ----------
        id : str
            Unique curve identifier.
        base_date : datetime.date or str
            Valuation date anchoring ``t = 0``.
        points : Sequence[tuple[datetime.date or str, float]]
            Strictly ascending ``(date, projection_factor)`` pillars on or
            after ``base_date`` with strictly positive finite factors.
        day_count : DayCount or str
            Explicit day count used to convert pillar dates to years.
        interp : str
            Explicit interpolation style.
        extrapolation : str
            Explicit tail extrapolation policy (``"flat"``, ``"extend"``, or
            ``"error"``).

        Returns
        -------
        ForwardCurve
            Curve with one pillar per dated point.

        Raises
        ------
        ValueError
            If ``points`` is empty, a pillar precedes ``base_date``, mapped
            pillar times do not strictly increase, or a label or factor is invalid.

        Examples
        --------
        >>> from finstack_quant.core.market_data import ForwardCurve
        >>> curve = ForwardCurve.from_dates(
        ...     "USD-SOFR-3M",
        ...     "2025-01-01",
        ...     [("2026-01-01", 0.96)],
        ...     "act_360",
        ...     "log_linear",
        ...     "extend",
        ... )
        >>> round(curve.df("2026-01-01"), 4)
        0.96

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
            The validated forward curve.

        Raises
        ------
        ValueError
            If the JSON is malformed, has unknown fields, or fails validation.

        Examples
        --------
        >>> from finstack_quant.core.market_data import ForwardCurve
        >>> curve = ForwardCurve.flat("USD-SOFR-3M", "2025-01-01", 0.04, "act_360")
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

    def df(self, t: TimeOrDate) -> float:
        """
        Projection factor ``P_x(t)`` at a year fraction or date.

        Parameters
        ----------
        t : float, datetime.date or str
            Year fraction from ``base_date``, or a date converted with the
            curve day count; times before the base date follow the tail policy.

        Returns
        -------
        float
            Unitless, strictly positive projection factor (``1.0`` at ``t = 0``).

        Raises
        ------
        ValueError
            If ``t`` is non-finite, outside the knots under ``"error"``
            extrapolation, or the date's day-count fraction cannot be computed.
        """
        ...

    def df_between_dates(self, from_date: DateLike, to_date: DateLike) -> float:
        """
        Relative projection factor between two dates: ``P_x(to_date) / P_x(from_date)``.

        Parameters
        ----------
        from_date : datetime.date or str
            Start date.
        to_date : datetime.date or str
            End date; may precede ``from_date`` (the ratio inverts). Returns
            ``1.0`` when both dates coincide.

        Returns
        -------
        float
            Unitless relative projection factor.

        Raises
        ------
        ValueError
            If either date's projection factor cannot be admitted or the ratio overflows.

        Examples
        --------
        >>> from finstack_quant.core.market_data import ForwardCurve
        >>> curve = ForwardCurve.flat("USD-SOFR-3M", "2025-01-01", 0.04, "act_360")
        >>> round(curve.df_between_dates("2025-01-01", "2025-04-01"), 6)
        0.99005

        """
        ...

    def rate_between(self, t1: float, t2: float, accrual_year_fraction: float) -> float:
        """
        Simple forward rate (decimal) over ``[t1, t2]`` on a contractual accrual fraction.

        Evaluates ``(P_x(t1) / P_x(t2) - 1) / accrual_year_fraction``. The
        interval endpoints are curve times; the accrual fraction is the
        index's own day-count fraction for the same dated period, which may
        differ from ``t2 - t1`` when the index and curve conventions differ.

        Parameters
        ----------
        t1 : float
            Start year fraction from the curve base date using its day count.
        t2 : float
            End year fraction on the same curve basis; must be finite and greater than ``t1``.
        accrual_year_fraction : float
            Strictly positive accrual year fraction of the index period under
            its contractual day count (for example ``91 / 360``).

        Returns
        -------
        float
            Simple forward rate as a decimal, annualized on ``accrual_year_fraction``.

        Raises
        ------
        ValueError
            If the interval is empty, reversed or non-finite, the accrual
            fraction is not finite and positive, or a factor or the rate cannot
            be computed.

        Examples
        --------
        >>> from finstack_quant.core.market_data import ForwardCurve
        >>> curve = ForwardCurve.flat("USD-SOFR-3M", "2025-01-01", 0.04, "act_360")
        >>> round(curve.rate_between(0.0, 0.25, 91 / 360), 6)
        0.039759

        """
        ...

    def rate_between_dates(self, start: DateLike, end: DateLike, accrual_year_fraction: float) -> float:
        """
        Simple forward rate (decimal) over ``[start, end]`` on a contractual accrual fraction.

        Parameters
        ----------
        start : datetime.date or str
            Period start date.
        end : datetime.date or str
            Exclusive period end date strictly after ``start``.
        accrual_year_fraction : float
            Strictly positive accrual year fraction of the index period under
            its contractual day count.

        Returns
        -------
        float
            Simple forward rate as a decimal, annualized on ``accrual_year_fraction``.

        Raises
        ------
        ValueError
            If ``end <= start``, mapped coordinates reverse order, ``accrual_year_fraction``
            is not finite and positive, or a factor or the rate cannot be computed.

        Examples
        --------
        >>> from finstack_quant.core.market_data import ForwardCurve
        >>> curve = ForwardCurve.flat("USD-SOFR-3M", "2025-01-01", 0.04, "act_360")
        >>> round(curve.rate_between_dates("2025-01-01", "2025-04-01", 90 / 360), 6)
        0.040201

        """
        ...

    def instantaneous_forward(self, t: float) -> float:
        """
        Instantaneous projection forward ``-P_x'(t) / P_x(t)`` at a year fraction.

        Parameters
        ----------
        t : float
            Curve time in years from ``base_date``; at a knot a one-sided
            derivative of the interpolator is used.

        Returns
        -------
        float
            Continuously-compounded instantaneous forward rate as a decimal.

        Raises
        ------
        ValueError
            If ``t`` is non-finite, the factor at ``t`` cannot be evaluated, or
            the derivative overflows.

        Examples
        --------
        >>> from finstack_quant.core.market_data import ForwardCurve
        >>> curve = ForwardCurve.flat("USD-SOFR-3M", "2025-01-01", 0.04, "act_360")
        >>> round(curve.instantaneous_forward(3.0), 6)
        0.04

        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export knots as a pandas ``DataFrame``.

        Returns
        -------
        pd.DataFrame
            Columns ``t`` (years) and ``factor`` (projection factor); one row
            per knot, ascending.

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
    def index_tenor(self) -> Optional[Tenor]:
        """
        Accrual tenor of the projected index.

        Returns
        -------
        Optional[Tenor]
            The index tenor (for example ``3M``), or ``None`` when the curve
            carries no index-period metadata.

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
            Strictly ascending year fractions measured from ``base_date``,
            starting at ``0.0``; interpolation happens between neighbouring entries.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def factors(self) -> list[float]:
        """
        Projection factors at the curve pillars.

        Returns
        -------
        list[float]
            One strictly positive factor ``P_x(t)`` per knot, ``1.0`` at ``t = 0``.

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
    def calendar_id(self) -> Optional[str]:
        """
        Registered holiday calendar of the curve's time axis.

        Returns
        -------
        Optional[str]
            Canonical calendar id (for example ``"brbd"`` on a ``"bus_252"``
            curve), or ``None`` when the day count needs no calendar.

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
            Style label such as ``"log_linear"`` or ``"monotone_convex"``, fixing how values between knots are produced.

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
            Policy label such as ``"extend"``, ``"flat"`` or ``"error"``; it governs queries outside the knot range.

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

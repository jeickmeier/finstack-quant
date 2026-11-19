"""Stub declarations mirroring ``finstack-quant-py/src/bindings/core/market_data/curves/discount.rs``."""

from __future__ import annotations

import datetime
from typing import Any, Optional, Sequence, Union
import pandas as pd
from finstack_quant.core.dates import DayCount
from finstack_quant.core.market_data import DateLike, TimeOrDate

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
    >>> curve = DiscountCurve(
    ...     "USD-OIS",
    ...     datetime.date(2025, 1, 1),
    ...     [(0.0, 1.0), (1.0, 0.95), (5.0, 0.80)],
    ...     interp="monotone_convex",
    ...     extrapolation="extend",
    ...     day_count="act_365f",
    ... )
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
        interp: str,
        extrapolation: str,
        day_count: Union[DayCount, str],
        calendar_id: Optional[str] = None,
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
            unitless and strictly positive. A ``(0.0, 1.0)`` anchor is
            conventional.
        interp : str
            Interpolation style (``"monotone_convex"``, ``"linear"``,
            ``"log_linear"``, ``"cubic_hermite"``,
            ``"piecewise_quadratic_forward"``). Keyword-only and required.
        extrapolation : str
            Extrapolation policy (``"extend"``, ``"flat"``, or
            ``"error"``, which raises outside the pillar range). Keyword-only
            and required.
        day_count : DayCount | str
            Day-count label used to convert query dates to curve time
            (never inferred from ``id``). Keyword-only and required.
        calendar_id : str, optional
            Registered holiday calendar of the curve's time axis (for example
            ``"brbd"``), or a ``+``-joined union. Required with
            ``day_count="bus_252"``: every date-based query, including
            ``df_between_dates``, counts business days on this calendar.
            ``None`` (default) suits every calendar-free day count.

        Raises
        ------
        ValueError
            If a knot is non-finite, non-positive, or duplicated, a label is
            unknown, or a ``"bus_252"`` curve has no ``calendar_id``.
        KeyError
            If ``calendar_id`` names no registered calendar.
        TypeError
            If ``base_date`` is neither a date nor a string.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.core.market_data import DiscountCurve
        >>> curve = DiscountCurve(
        ...     "USD-OIS",
        ...     datetime.date(2025, 1, 1),
        ...     [(0.0, 1.0), (1.0, 0.95)],
        ...     interp="monotone_convex",
        ...     extrapolation="extend",
        ...     day_count="act_360",
        ... )
        >>> curve.day_count
        'act_360'
        >>> cdi = DiscountCurve(
        ...     "BRL-CDI",
        ...     "2025-01-06",
        ...     [(0.0, 1.0), (1.0, 0.9)],
        ...     interp="log_linear",
        ...     extrapolation="extend",
        ...     day_count="bus_252",
        ...     calendar_id="brbd",
        ... )
        >>> cdi.calendar_id
        'brbd'

        """
        ...

    @staticmethod
    def flat(
        id: str,
        base_date: DateLike,
        continuous_rate: float,
        day_count: Union[DayCount, str],
    ) -> DiscountCurve:
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
        day_count : DayCount | str
            Explicit supported day-count convention label or object.

        Returns
        -------
        DiscountCurve
            Curve with ``df(t) == exp(-continuous_rate * t)``.

        Raises
        ------
        ValueError
            If the rate is non-finite or has magnitude greater than ``1.0``,
            or ``day_count`` is invalid or requires an absent calendar.

        Examples
        --------
        >>> from finstack_quant.core.market_data import DiscountCurve
        >>> round(DiscountCurve.flat("USD-OIS", "2025-01-01", 0.05, "act_365f").df(2.0), 6)
        0.904837

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
        compounding : str
            ``"continuous"``, ``"simple"``, ``"periodic(1)"``,
            ``"semi_annual"``, ``"quarterly"`` or ``"monthly"``.
        day_count : DayCount | str
            Explicit curve day-count convention.
        interp : str
            Explicit interpolation style.
        extrapolation : str
            Explicit tail extrapolation policy (``"flat"``, ``"extend"``, or
            ``"error"``).

        Returns
        -------
        DiscountCurve
            Curve built with the specified explicit conventions.

        Raises
        ------
        ValueError
            If ``points`` is empty, a pillar is non-finite, a label is
            unknown, or construction fails.

        Examples
        --------
        >>> from finstack_quant.core.market_data import DiscountCurve
        >>> curve = DiscountCurve.from_zero_rates(
        ...     "USD-OIS",
        ...     "2025-01-01",
        ...     [(1.0, 0.05), (2.0, 0.05)],
        ...     "periodic(1)",
        ...     "act_365f",
        ...     "monotone_convex",
        ...     "extend",
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
        day_count: Union[DayCount, str],
        interp: str,
        extrapolation: str,
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
        day_count : DayCount | str
            Explicit day count used to convert pillar dates to years.
        interp : str
            Explicit interpolation style.
        extrapolation : str
            Explicit tail extrapolation policy (``"flat"``, ``"extend"``, or
            ``"error"``).

        Returns
        -------
        DiscountCurve
            The constructed discount curve, ready for ``df`` and zero-rate queries.

        Raises
        ------
        ValueError
            If ``points`` is empty, a pillar precedes ``base_date``, or an
            enum label or discount factor is invalid.

        Examples
        --------
        >>> from finstack_quant.core.market_data import DiscountCurve
        >>> curve = DiscountCurve.from_dates(
        ...     "USD-OIS",
        ...     "2025-01-01",
        ...     [("2026-01-01", 0.95)],
        ...     "act_365f",
        ...     "monotone_convex",
        ...     "extend",
        ... )
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
        >>> curve = DiscountCurve.flat("USD-OIS", "2025-01-01", 0.05, "act_365f")
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
            Non-negative year fraction from ``base_date``, or a date on or
            after ``base_date`` converted with the curve day count.

        Returns
        -------
        float
            Unitless discount factor.

        Raises
        ------
        ValueError
            If ``t < 0``, a date precedes ``base_date`` or is not a valid ISO
            date, or an ``"error"`` tail rejects out-of-range queries.
        TypeError
            If ``t`` is neither a number nor date-like.
        """
        ...

    def zero_rate(self, t: TimeOrDate, compounding: str) -> float:
        """
        Zero rate at a year fraction or date under an explicit compounding convention.

        Parameters
        ----------
        t : float, datetime.date or str
            Strictly positive year fraction from ``base_date``, or a date
            strictly after ``base_date``. For a date, the horizon is the
            difference between its source-clock year fraction and the base
            date’s source-clock year fraction.
        compounding : str
            ``"continuous"``, ``"simple"``, ``"periodic(1)"``,
            ``"semi_annual"``, ``"quarterly"`` or ``"monthly"``.

        Returns
        -------
        float
            Zero rate as a decimal.

        Raises
        ------
        ValueError
            If ``t <= 0``, ``t`` is a date on or before ``base_date``, or
            ``compounding`` is not a recognised label.
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
    def calendar_id(self) -> Optional[str]:
        """
        Registered holiday calendar of the curve's time axis.

        Returns
        -------
        str or None
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

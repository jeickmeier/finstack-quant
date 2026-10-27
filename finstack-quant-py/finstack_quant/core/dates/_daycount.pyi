"""Stub declarations mirroring ``finstack-quant-py/src/bindings/core/dates/daycount.rs``."""

from __future__ import annotations

import datetime
from typing import Optional, Union
from finstack_quant.core.dates._tenor import Tenor

# Day-count conventions

class DayCount:
    """
    Day-count convention for year-fraction calculations.

    Immutable, hashable enum-style type with class attributes for each
    supported convention.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.dates import DayCount
    >>> day_count = DayCount.ACT_360
    >>> day_count.year_fraction(datetime.date(2024, 1, 1), datetime.date(2024, 7, 1))
    0.5055555555555555
    """

    ONE_ONE: DayCount
    """One unit per nonempty accrual period, zero for an empty interval; reversed dates raise ValueError."""
    ACT_360: DayCount
    """Actual/360 (money market)."""
    ACT_365F: DayCount
    """Actual/365 Fixed."""
    ACT_365L: DayCount
    """Actual/365L (ICMA Rule 251).

    Requires both ``frequency`` and ``coupon_period`` in ``DayCountContext``.
    Annual periods use denominator 366 exactly when February 29 falls in
    ``(coupon_start, coupon_end]``; otherwise 365. Non-annual periods use 366
    exactly when ``coupon_end``'s year is a leap year; otherwise 365. This is
    explicitly not ACT/ACT AFB, which uses a different sub-period-splitting
    algorithm.
    """
    NL_365: DayCount
    """NL/365 (Actual/365 No Leap): actual days excluding every February 29
    in ``(start, end]``, divided by 365."""
    THIRTY_360: DayCount
    """30/360 US (Bond Basis)."""
    THIRTY_E_360: DayCount
    """30E/360 (Eurobond Basis)."""
    THIRTY_E_360_ISDA: DayCount
    """30E/360 ISDA."""
    ACT_ACT: DayCount
    """Actual/Actual (ISDA)."""
    ACT_ACT_ISMA: DayCount
    """Actual/Actual (ICMA/ISMA)."""
    ACT_ACT_AFB: DayCount
    """Actual/Actual AFB (Actual/Actual Euro).

    Walks whole years backwards from the end date (QuantLib
    ``ActualActual::AFB``). A year-step landing on 28 February of a leap
    year is bumped to 29 February. The residual uses denominator 366 if
    29 February lies in ``[start, residual_end)``, else 365. No context
    is required.
    """
    THIRTY_360_IT: DayCount
    """30/360 Italian.

    Day 31 becomes 30, and any February day after the 27th becomes 30
    (QuantLib ``Thirty360::Italian``). Distinct from US SIA and 30E/360.
    """
    BUS_252: DayCount
    """Business/252 (Brazilian market convention)."""

    @classmethod
    def from_name(cls, name: str) -> DayCount:
        """
        Parse a day-count convention from its string name.

        Parameters
        ----------
        name : str
            Convention identifier (e.g. ``"act_360"``, ``"act_365f"``,
            ``"30_360"``, ``"act_act_afb"``, ``"30_360_it"``, ``"bus_252"``).

        Returns
        -------
        DayCount

            Day-count convention for the canonical name, ignoring surrounding
            whitespace and ASCII case. Other spellings are rejected.

        Raises
        ------
        ValueError
            If *name* is not recognised.

        Examples
        --------
        >>> from finstack_quant.core.dates import DayCount
        >>> DayCount.from_name("act_360") == DayCount.ACT_360
        True
        >>> DayCount.from_name("act_act_afb") == DayCount.ACT_ACT_AFB
        True
        >>> DayCount.from_name("30_360_it") == DayCount.THIRTY_360_IT
        True

        """
        ...

    def year_fraction(
        self,
        start: datetime.date | str,
        end: datetime.date | str,
        ctx: Optional[DayCountContext] = None,
    ) -> float:
        """
        Compute the year fraction between two dates.

        Parameters
        ----------
        start : datetime.date | str
            Accrual start (inclusive); ``datetime.date``, ``pandas.Timestamp``
            or ISO ``YYYY-MM-DD`` string.
        end : datetime.date | str
            Accrual end (exclusive); must not precede *start*.
        ctx : DayCountContext | None
            Context for conventions that need one: ``ACT_ACT_ISMA`` and
            ``ACT_365L`` read ``frequency``, ``ACT_365L`` also the enclosing
            contractual ``coupon_period`` (partial accrual retains that
            coupon's denominator), ``BUS_252`` reads ``calendar_id``.

        Returns
        -------
        float
            Non-negative year fraction (``0.0`` when ``start == end``).

        Raises
        ------
        ValueError
            If *start* > *end* or the convention needs context that was not
            supplied. ACT/365L raises
            if frequency or enclosing coupon dates are missing, or the
            requested dates lie outside that coupon. ICMA raises if reference
            endpoints do not share an unadjusted nominal month grid.
        KeyError
            If *ctx* names an unknown calendar.

        Examples
        --------
        >>> from finstack_quant.core.dates import DayCount, DayCountContext
        >>> DayCount.ACT_ACT_ISMA.year_fraction("2025-01-15", "2025-07-15", DayCountContext(frequency="6M"))
        0.5

        """
        ...

    def signed_year_fraction(
        self,
        start: datetime.date | str,
        end: datetime.date | str,
        ctx: Optional[DayCountContext] = None,
    ) -> float:
        """
        Compute the signed year fraction (negative when start > end).

        Parameters
        ----------
        start : datetime.date | str
            Start date.
        end : datetime.date | str
            End date; may precede *start*.
        ctx : DayCountContext | None
            Context for conventions that need one (see ``year_fraction``).

        Returns
        -------
        float
            Signed year fraction.

        Raises
        ------
        ValueError
            If required context is missing.
        KeyError
            If *ctx* names an unknown calendar.

        Examples
        --------
        >>> from finstack_quant.core.dates import DayCount
        >>> DayCount.ACT_360.signed_year_fraction("2024-07-01", "2024-01-01")
        -0.5055555555555555

        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __hash__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

class DayCountContext:
    """
    Optional context for day-count calculations.

    Certain conventions require additional information:

    - **Bus/252** requires a holiday calendar (resolved by ``calendar_id``).
    - **Act/Act (ISMA)** requires the coupon ``frequency`` and, for
      irregular or mid-coupon accruals, the unadjusted regular reference
      ``coupon_period``.
    - **Act/365L** requires ``frequency`` and the full enclosing contractual
      ``coupon_period``, including for partial accrual.
    - **30E/360 ISDA** uses ``end_is_termination_date`` for its
      end-of-February rule.

    Equality is structural, and instances round-trip through
    :meth:`to_json` / :meth:`from_json` and ``pickle``.

    Parameters
    ----------
    calendar_id : str | None
        Registered calendar id (e.g. ``"target2"``; ``"nyse+gblo"`` joins
        calendars). Resolved on each use, so an unknown id raises
        ``KeyError`` at calculation time.
    frequency : Tenor | str | None
        Coupon frequency for ISMA and ACT/365L (``Tenor`` or ``"6M"``).
    bus_basis : int | None
        Custom positive business-day divisor (defaults to 252 when omitted).
    coupon_period : tuple[datetime.date | str, datetime.date | str] | None
        Unadjusted regular ICMA reference period or ACT/365L enclosing
        contractual coupon ``(start, end)``; ``start`` must precede ``end``.
        ICMA endpoints must share the contractual nominal month grid,
        rather than business-day-adjusted payment dates.
    end_is_termination_date : bool
        Whether the accrual end is the instrument termination date.

    Examples
    --------
    >>> from finstack_quant.core.dates import DayCountContext
    >>> context = DayCountContext("usny", "3M", 252)
    >>> (context.calendar_id, context.frequency.months, context.bus_basis, context.calendar_id)
    ('usny', 3, 252, 'usny')

    """

    def __init__(
        self,
        calendar_id: Optional[str] = None,
        frequency: Union[Tenor, str, None] = None,
        bus_basis: Optional[int] = None,
        coupon_period: Optional[tuple[datetime.date | str, datetime.date | str]] = None,
        end_is_termination_date: bool = False,
    ) -> None:
        """
        Create a day-count context.

        Parameters
        ----------
        calendar_id : str | None
            Registered calendar id; not resolved until a calculation runs.
        frequency : Tenor | str | None
            Coupon frequency (``Tenor`` or tenor string such as ``"6M"``).
        bus_basis : int | None
            Custom positive business-day divisor for Bus/252.
        coupon_period : tuple[datetime.date | str, datetime.date | str] | None
            Unadjusted regular ICMA reference period or ACT/365L enclosing
            contractual coupon ``(start, end)``. ICMA validates that both
            endpoints share one nominal month grid at calculation time.
        end_is_termination_date : bool
            Whether the accrual end is the instrument termination date.

        Raises
        ------
        ValueError
            If *coupon_period* is supplied and its start is not before its
            end (validated in Rust), *frequency* does not parse, or
            *bus_basis* is zero.

        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form (strict field names).

        Returns
        -------
        str
            JSON object with ``calendar_id``, ``frequency``, ``bus_basis``,
            ``coupon_period`` (ISO date pair or ``null``) and
            ``end_is_termination_date``.

        Raises
        ------
        ValueError
            If the context cannot be serialized.

        Examples
        --------
        >>> from finstack_quant.core.dates import DayCountContext
        >>> DayCountContext.from_json(DayCountContext("usny").to_json()).calendar_id
        'usny'
        """
        ...

    @staticmethod
    def from_json(json: str) -> DayCountContext:
        """
        Deserialize from the canonical JSON wire form.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        DayCountContext
            The reconstructed context.

        Raises
        ------
        ValueError
            If *json* is malformed, has unknown fields, or carries an
            inverted ``coupon_period`` or a zero ``bus_basis``.
        Examples
        --------
        >>> from finstack_quant.core.dates import DayCountContext
        >>> DayCountContext.from_json(DayCountContext("usny").to_json()).calendar_id
        'usny'

        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...
    @property
    def calendar_id(self) -> Optional[str]:
        """
        Optional calendar identifier.

        Returns
        -------
        str | None
            Optional calendar identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def frequency(self) -> Optional[Tenor]:
        """
        Optional coupon frequency.

        Returns
        -------
        Tenor | None
            Optional coupon frequency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def bus_basis(self) -> Optional[int]:
        """
        Optional custom business-day divisor.

        Returns
        -------
        int | None
            Optional custom business-day divisor.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def coupon_period(self) -> Optional[tuple[datetime.date, datetime.date]]:
        """
        Optional unadjusted ICMA reference or ACT/365L enclosing coupon
        as ``(start, end)`` dates.

        Returns
        -------
        tuple[datetime.date, datetime.date] | None

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def end_is_termination_date(self) -> bool:
        """
        Whether the accrual end is the instrument termination date.

        Returns
        -------
        bool
            Whether the accrual end is the instrument termination date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str: ...

class Thirty360Convention:
    """
    30/360 sub-convention (US SIA / Bond Basis, ISDA, or European).

    Immutable, hashable enum-style type.

    Examples
    --------
    >>> from finstack_quant.core.dates import Thirty360Convention
    >>> str(Thirty360Convention.US_SIA)
    'us_sia'

    """

    US_SIA: Thirty360Convention
    """US 30/360 SIA / Bond Basis convention."""
    ISDA: Thirty360Convention
    """30/360 ISDA convention."""
    EUROPEAN: Thirty360Convention
    """European 30E/360 convention."""
    ITALIAN: Thirty360Convention
    """30/360 Italian convention (31→30 and February day after 27→30)."""

    @classmethod
    def from_name(cls, name: str) -> Thirty360Convention:
        """
        Parse this variant from its snake_case name, case-insensitively.

        Parameters
        ----------
        name : str
            One of ``"us_sia"``, ``"isda"``, ``"european"``, ``"italian"``.

        Returns
        -------
        Thirty360Convention
            The matching variant.

        Raises
        ------
        ValueError
            If *name* is not one of the four variant names.

        Examples
        --------
        >>> from finstack_quant.core.dates import Thirty360Convention
        >>> Thirty360Convention.from_name("ISDA") == Thirty360Convention.ISDA
        True

        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __hash__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

def days_30_360(
    start: datetime.date | str,
    end: datetime.date | str,
    convention: Union[Thirty360Convention, str],
) -> int:
    """
    30/360 day count between *start* (inclusive) and *end* (exclusive).

    Parameters
    ----------
    start : datetime.date | str
        Accrual start.
    end : datetime.date | str
        Accrual end; an earlier *end* gives a negative count.
    convention : Thirty360Convention | str
        Variant governing the month-end and February rules
        (``"us_sia"``, ``"isda"``, ``"european"``, ``"italian"``).

    Returns
    -------
    int
        Signed 30/360 day count (divide by 360 for the year fraction).

    Raises
    ------
    ValueError
        If a date or the convention name is invalid.
    TypeError
        If an argument has an unsupported type.

    Examples
    --------
    >>> from finstack_quant.core.dates import days_30_360
    >>> days_30_360("2025-01-31", "2025-03-31", "isda")
    60

    """
    ...

def days_30e_360_isda(
    start: datetime.date | str,
    end: datetime.date | str,
    end_is_termination_date: bool,
) -> int:
    """
    30E/360 ISDA day count between *start* (inclusive) and *end* (exclusive).

    Parameters
    ----------
    start : datetime.date | str
        Accrual start.
    end : datetime.date | str
        Accrual end; an earlier *end* gives a negative count.
    end_is_termination_date : bool
        Whether *end* is the instrument termination date, which keeps a
        February month-end at its actual day (ISDA 2006 §4.16(h)).

    Returns
    -------
    int
        Signed 30E/360 ISDA day count.

    Raises
    ------
    ValueError
        If a date is invalid.
    TypeError
        If a date argument has an unsupported type.

    Examples
    --------
    >>> from finstack_quant.core.dates import days_30e_360_isda
    >>> days_30e_360_isda("2024-01-31", "2024-02-29", False)
    30

    """
    ...

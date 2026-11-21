"""Stub declarations mirroring ``finstack-quant-py/src/bindings/core/market_data/curves/inflation.rs``."""

from __future__ import annotations

import datetime
from typing import Any, Optional, Sequence, Union
import pandas as pd
import polars as pl
from finstack_quant.core.dates import DayCount
from finstack_quant.core.market_data import DateLike, TimeOrDate
from finstack_quant.core.table import ArrowTable

class InflationCurve:
    """
    CPI inflation curve for inflation-linked pricing and breakeven analysis.

    Stores ``(t, cpi_level)`` knots in years from ``base_date`` as absolute
    index levels (e.g. ``300.0``). ``cpi`` accepts a year fraction or a date.

    Examples
    --------
    >>> from finstack_quant.core.market_data import InflationCurve
    >>> curve = InflationCurve(
    ...     "US-CPI",
    ...     "2025-01-01",
    ...     [(0.0, 300.0), (1.0, 306.0), (2.0, 312.0)],
    ...     day_count="act_365f",
    ...     interp="log_linear",
    ...     extrapolation="extend",
    ... )
    >>> round(curve.inflation_rate(0.0, 1.0), 4)
    0.02
    >>> curve.cpi_levels
    [300.0, 306.0, 312.0]

    """

    @staticmethod
    def from_dates(
        id: str,
        base_date: DateLike,
        points: Sequence[tuple[DateLike, float]],
        *,
        day_count: DayCount | str,
        interp: str,
        extrapolation: str,
        calendar_id: str | None = None,
    ) -> InflationCurve:
        """Construct an inflation curve from dated projected CPI levels.

        Parameters
        ----------
        id : str
            Identifier used in market-data lookups.
        base_date : DateLike
            Date defining zero years on the source clock.
        points : Sequence[tuple[DateLike, float]]
            Strictly increasing observation dates with the first observation on the base date.
            Finite positive unlagged CPI levels in index points.
        day_count : DayCount or str
            Calendar-to-year-fraction convention such as act_365f.
        interp : str
            Interpolation label for CPI levels, such as log_linear.
        extrapolation : str
            Outside-grid policy, such as flat.
        calendar_id : str or None
            Holiday calendar, required for day counts such as bus_252.

        Returns
        -------
        InflationCurve
            Validated curve with the supplied dated pillars.

        Raises
        ------
        TypeError
            If a date or quote has an incompatible Python type.
        ValueError
            If the dated grid, clock, policies or quotes fail validation.

        Examples
        --------
        >>> from finstack_quant.core.market_data import InflationCurve
        >>> curve = InflationCurve.from_dates(
        ...     "CURVE",
        ...     "2025-01-01",
        ...     [("2025-01-01", 300.0), ("2026-01-01", 306.0)],
        ...     day_count="act_365f",
        ...     interp="log_linear",
        ...     extrapolation="flat",
        ... )
        >>> curve.cpi("2026-01-01")
        306.0
        """
        ...

    def __init__(
        self,
        id: str,
        base_date: DateLike,
        knots: Sequence[tuple[float, float]],
        *,
        day_count: Union[DayCount, str],
        interp: str,
        extrapolation: str,
        calendar_id: Optional[str] = None,
    ) -> None:
        """
        Construct an inflation curve from CPI knot points.

        Parameters
        ----------
        id : str
            Unique curve identifier (e.g. ``"US-CPI"``).
        base_date : datetime.date or str
            Valuation date anchoring ``t = 0``.
        knots : Sequence[tuple[float, float]]
            Strictly ascending ``(time_years, cpi_level)`` pairs beginning with
            explicit ``(0.0, I0)`` and at least one positive-time pillar.
            Levels must be finite and strictly positive.
        day_count : DayCount | str
            Required source clock, for example ``"act_365f"``.
        interp : str
            Required ``"linear"``, ``"cubic_hermite"`` or ``"log_linear"``.
        extrapolation : str
            Required right-tail policy: ``"flat"``, ``"extend"`` or ``"error"``.
        calendar_id : str, optional
            Registered holiday calendar identifier; required for Bus/252.
            ``None`` omits the calendar.

        Raises
        ------
        ValueError
            If knots are invalid, the explicit origin or positive-time pillar
            is missing, a convention is unsupported, or the clock is invalid.
        TypeError
            If required conventions are omitted or inputs have unsupported types.

        Examples
        --------
        >>> from finstack_quant.core.market_data import InflationCurve
        >>> curve = InflationCurve(
        ...     "US-CPI",
        ...     "2025-01-01",
        ...     [(0.0, 300.0), (1.0, 306.0)],
        ...     day_count="act_365f",
        ...     interp="log_linear",
        ...     extrapolation="error",
        ... )
        >>> curve.base_cpi
        300.0

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
            The validated unlagged CPI projection curve.

        Raises
        ------
        ValueError
            If the JSON is malformed, has unknown fields, or fails validation.

        Examples
        --------
        >>> from finstack_quant.core.market_data import InflationCurve
        >>> curve = InflationCurve(
        ...     "US-CPI",
        ...     "2025-01-01",
        ...     [(0.0, 300.0), (1.0, 306.0)],
        ...     day_count="act_365f",
        ...     interp="log_linear",
        ...     extrapolation="extend",
        ... )
        >>> InflationCurve.from_json(curve.to_json()).to_json() == curve.to_json()
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
            If time is non-finite or negative, a date precedes ``base_date``,
            a query violates ``"error"`` extrapolation, or CPI is invalid.
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

    def to_arrow(self) -> ArrowTable:
        """
        Export knots as an :class:`~finstack_quant.core.table.ArrowTable`.

        Returns
        -------
        ArrowTable
            Columns ``t`` (years) and ``cpi``; one row per knot.

        Raises
        ------
        ValueError
            If the underlying Arrow record batch cannot be built.
        """
        ...

    def to_pandas(self) -> pd.DataFrame:
        """
        Export knots as a pandas ``DataFrame``.

        Returns
        -------
        pd.DataFrame
            Columns ``t`` (years) and ``cpi``; one row per knot.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """
        Export knots as a ``polars.DataFrame``.

        Returns
        -------
        pl.DataFrame
            One row per knot constructed via the Arrow PyCapsule interface.

        Raises
        ------
        ImportError
            If ``polars`` is not installed.
        """
        ...

    def to_pyarrow(self) -> Any:
        """
        Export knots as a ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            Zero-copy table built through the Arrow PyCapsule interface.

        Raises
        ------
        ImportError
            If ``pyarrow`` is not installed.
        """
        ...

    def __arrow_c_stream__(self, requested_schema: Any | None = None) -> Any: ...
    def __arrow_c_schema__(self) -> Any: ...
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
    def calendar_id(self) -> Optional[str]:
        """
        Registered holiday calendar used by the source clock.

        Returns
        -------
        str or None
            Calendar identifier, or ``None`` when no calendar is attached.

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
            Policy label such as ``"extend"``, ``"flat"`` or ``"error"``; it governs queries outside the knot range.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Reconstruct the curve through its validated canonical JSON when unpickling."""
        ...
    def __repr__(self) -> str: ...
    def _repr_html_(self) -> Optional[str]: ...

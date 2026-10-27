"""Stub declarations mirroring ``finstack-quant-py/src/bindings/core/market_data/curves/inflation.rs``."""

from __future__ import annotations

import datetime
from typing import Any, Optional, Sequence, Union
import pandas as pd
from finstack_quant.core.dates import DayCount
from finstack_quant.core.market_data import DateLike, TimeOrDate

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
        day_count: Optional[Union[DayCount, str]] = None,
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
            Inserted as the zero-time interpolation knot when absent.
        knots : Sequence[tuple[float, float]]
            ``(time_years, cpi_level)`` pairs; levels must be positive.
        day_count : DayCount | str, optional
            Day-count label; default ``"act_365f"``.
        indexation_lag_months : int, optional
            Indexation lag in months applied by :meth:`cpi_with_lag`; default ``3``.
        interp : str, optional
            Interpolation style; default ``"log_linear"``.
        extrapolation : str, optional
            Extrapolation policy; default ``"flat_zero"``.

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

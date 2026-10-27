"""Stub declarations mirroring ``finstack-quant-py/src/bindings/core/market_data/scalars.rs``."""

from __future__ import annotations

import datetime
from decimal import Decimal
from typing import Any, Optional, Sequence, Union
import pandas as pd
from finstack_quant.core.currency import Currency
from finstack_quant.core.market_data import DateLike

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
            Dated values; a ``Decimal`` converts to ``float``. Dates must be unique; any order is accepted.
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
        >>> str(ScalarTimeSeries("PX", [("2025-01-01", 100.0)], currency="USD").currency)
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
    >>> released = index.with_publication_dates([("2025-01-01", "2025-02-12")])
    >>> released.get_publication_date("2025-01-31").isoformat()
    '2025-02-12'
    >>> len(released.get_publication_dates())
    1

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
            Dated index levels; a ``Decimal`` converts to ``float``.
        currency : Currency or str
            Currency of the index.
        interpolation : str, optional
            ``"step"`` (default) or ``"linear"``.
        lag : str or int, optional
            Contractual observation lag applied before lookups: ``"none"`` (default),
            market strings such as ``"3M"`` or ``"90D"``, or an integer number
            of months.
        seasonality : Sequence[float], optional
            Twelve multiplicative factors, January through December.

        Raises
        ------
        ValueError
            If ``observations`` is empty or has duplicate dates, a label is
            unknown, a value is non-finite, or ``seasonality`` does not have
            exactly 12 entries.

        Examples
        --------
        >>> from finstack_quant.core.market_data import InflationIndex
        >>> InflationIndex("US-CPI", [("2025-01-01", 300.0)], "USD", lag="3M").lag
        '3M'

        """
        ...

    def with_publication_dates(self, dates: Sequence[tuple[DateLike, DateLike]]) -> InflationIndex:
        """
        Return a new index with an explicit monthly publication schedule.

        Parameters
        ----------
        dates : Sequence[tuple[datetime.date or str, datetime.date or str]]
            First-of-month reference dates paired with inclusive publication
            dates. Replaces any prior schedule independently of observation lag.
            This declares monthly reference observations for valuation even
            with no lag or a day lag; observation label days are not fixing dates.

        Returns
        -------
        InflationIndex
            New index retaining the observations and other conventions.

        Raises
        ------
        ValueError
            If a date is malformed, a reference key is not month start, a month
            is duplicated, or publication precedes its reference month.
        TypeError
            If an entry is not a pair of supported date-like values.
        """
        ...

    def get_publication_date(self, reference_date: DateLike) -> Optional[datetime.date]:
        """
        Look up the configured availability date of a reference month.

        Parameters
        ----------
        reference_date : datetime.date or str
            Any day in the reference month; its day component is ignored.

        Returns
        -------
        datetime.date or None
            Inclusive availability date, or ``None`` when unspecified. Missing
            metadata does not authorize forecasting missing historical fixings.

        Raises
        ------
        ValueError
            If the reference date is malformed.
        TypeError
            If the reference date is not date-like or an ISO date string.
        """
        ...

    def get_publication_dates(self) -> list[tuple[datetime.date, datetime.date]]:
        """
        Return the configured publication schedule in reference-month order.

        Returns
        -------
        list[tuple[datetime.date, datetime.date]]
            First-of-month reference dates and inclusive publication dates.

        Raises
        ------
        ValueError
            If an internal date cannot be converted to a Python date.
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
    def _repr_html_(self) -> Optional[str]: ...

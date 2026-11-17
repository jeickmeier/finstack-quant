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
    >>> series = ScalarTimeSeries("SOFR", [("2025-01-01", 0.04), ("2025-01-03", 0.05)], "linear", "flat")
    >>> round(series.get_value("2025-01-02"), 4)
    0.045
    >>> (series.first_date, series.extrapolation, len(series))
    (datetime.date(2025, 1, 1), 'flat', 2)

    """

    def __init__(
        self,
        id: str,
        observations: Sequence[tuple[DateLike, Union[float, int, Decimal]]],
        interpolation: str,
        extrapolation: str,
        currency: Optional[Union[Currency, str]] = None,
    ) -> None:
        """
        Construct a scalar time series from strictly ascending dated observations.

        Parameters
        ----------
        id : str
            Non-empty trimmed series identifier.
        observations : Sequence[tuple[datetime.date or str, float, int or Decimal]]
            Chronologically strictly ascending ``(date, value)`` pairs; a
            ``Decimal`` converts to ``float``. Empty and single-point series
            are accepted; unsorted or duplicate dates raise ``ValueError``.
        interpolation : str
            Required in-range fill policy: ``"step"`` or ``"linear"``.
        extrapolation : str
            Required right-tail policy after the latest observation:
            ``"flat"`` (hold last value) or ``"error"`` (raise ``KeyError``).
        currency : Currency or str, optional
            Currency tag for monetary series; ``None`` for unitless values.

        Raises
        ------
        ValueError
            If ``id`` is blank/untrimmed, dates are unsorted or duplicated, a
            value is non-finite, or ``interpolation``/``extrapolation`` is not
            a recognised label.
        TypeError
            If a value is not a float, int or ``Decimal``.

        Examples
        --------
        >>> from finstack_quant.core.market_data import ScalarTimeSeries
        >>> str(ScalarTimeSeries("PX", [("2025-01-01", 100.0)], "step", "error", currency="USD").currency)
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
        >>> series = ScalarTimeSeries("SOFR", [("2025-01-01", 0.03)], "step", "flat")
        >>> ScalarTimeSeries.from_json(series.to_json()).get_value("2025-01-01")
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

    def get_value(self, date: DateLike) -> float:
        """
        Value on a date under the series interpolation and extrapolation policies.

        Parameters
        ----------
        date : datetime.date or str
            Lookup date; empty series, dates before the first stored
            observation, and dates after the last observation under
            ``"error"`` extrapolation raise ``KeyError``.

        Returns
        -------
        float
            Interpolated (or stepped/carried) value.

        Raises
        ------
        KeyError
            If the series is empty, ``date`` precedes the first observation,
            or ``date`` exceeds the last observation when ``extrapolation == "error"``.
        ValueError
            If ``date`` cannot be parsed as a valid date.
        """
        ...

    def get_value_exact(self, date: DateLike) -> float:
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
            Either ``"step"`` (hold the last observation) or ``"linear"`` (interpolate between neighbouring dates).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def extrapolation(self) -> str:
        """
        How queries strictly after the latest observation are handled.

        Returns
        -------
        str
            Either ``"flat"`` (hold the last observation) or ``"error"`` (reject queries past ``last_date``).

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
            The first observation date in the series, or ``None`` when the series is empty; queries before it fail.

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
            The last observation date in the series, or ``None`` when the series is empty; queries after it follow :attr:`extrapolation`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __len__(self) -> int: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...
    def _repr_html_(self) -> Optional[str]: ...

class InflationObservation:
    """Monthly CPI fixing record; snapshot construction validates its financial invariants.

    Examples
    --------
    >>> from finstack_quant.core.market_data import InflationObservation
    >>> observation = InflationObservation("2025-01-01", 300.0, "2025-02-12")
    >>> observation.value
    300.0
    """

    def __init__(self, reference_month: DateLike, value: Union[float, int, Decimal], available_on: DateLike) -> None:
        """Construct a fixing record without applying lag or interpolation.

        Parameters
        ----------
        reference_month : datetime.date or str
            First calendar day of the CPI reference month.
        value : float, int or Decimal
            Published CPI index level; Decimal converts to float.
        available_on : datetime.date or str
            Inclusive availability date for the selected observation version.

        Raises
        ------
        TypeError
            If dates are unsupported or value is bool or not numeric.
        ValueError
            If a date is malformed or a Decimal is non-finite. InflationIndex
            validates positivity, canonical month keys and cutoff constraints.
        """
        ...

    @property
    def reference_month(self) -> datetime.date:
        """Reference month key.

        Returns
        -------
        datetime.date
            Stored monthly key, without adjustment.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def value(self) -> float:
        """Published CPI level.

        Returns
        -------
        float
            Absolute index points, not a growth rate.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def available_on(self) -> datetime.date:
        """Observation-version availability cutoff.

        Returns
        -------
        datetime.date
            Inclusive date the fixing became available.

        Notes
        -----
        This accessor does not raise.
        """
        ...

class InflationExpectedRelease:
    """Expected monthly release assumption, distinct from observed availability.

    Examples
    --------
    >>> from finstack_quant.core.market_data import InflationExpectedRelease
    >>> release = InflationExpectedRelease("2025-01-01", "2025-02-12", "2025-01-01")
    >>> release.expected_release
    datetime.date(2025, 2, 12)
    """

    def __init__(self, reference_month: DateLike, expected_release: DateLike, known_on: DateLike) -> None:
        """Construct schedule metadata; attaching it to a snapshot validates the dates.

        Parameters
        ----------
        reference_month : datetime.date or str
            First calendar day of the reference month.
        expected_release : datetime.date or str
            Scheduled release date, no earlier than the reference month.
        known_on : datetime.date or str
            Inclusive date this release assumption was known.

        Raises
        ------
        ValueError
            If a date is malformed.
        TypeError
            If an argument is not a supported date-like value.
        """
        ...

    @property
    def reference_month(self) -> datetime.date:
        """Reference month of the scheduled print.

        Returns
        -------
        datetime.date
            Stored reference-month key.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def expected_release(self) -> datetime.date:
        """Scheduled date of the monthly release.

        Returns
        -------
        datetime.date
            Expected release date, not proof of fixing availability.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def known_on(self) -> datetime.date:
        """Date this release assumption entered the information set.

        Returns
        -------
        datetime.date
            Inclusive knowledge date for the schedule fact.

        Notes
        -----
        This accessor does not raise.
        """
        ...

class InflationIndex:
    """
    Immutable monthly CPI fixing snapshot with an explicit knowledge cutoff.

    Examples
    --------
    >>> from finstack_quant.core.market_data import InflationIndex, InflationObservation
    >>> index = InflationIndex("US-CPI", [InflationObservation("2025-01-01", 300.0, "2025-02-12")], "USD", "2025-03-01")
    >>> index.qualified_monthly_observation(2025, 1, "2025-02-12")
    300.0

    """

    def __init__(
        self,
        id: str,
        observations: Sequence[InflationObservation],
        currency: Union[Currency, str],
        knowledge_date: DateLike,
    ) -> None:
        """
        Construct a snapshot from strictly ordered canonical monthly records.

        Parameters
        ----------
        id : str
            Index identifier (e.g. ``"US-CPI-U"``).
        observations : Sequence[InflationObservation]
            Strictly ascending unique first-of-month records with finite positive
            levels and ``reference_month <= available_on <= knowledge_date``.
            Empty histories are valid. No carry, interpolation or lag is applied.
        currency : Currency or str
            Currency of the index.
        knowledge_date : datetime.date or str
            Inclusive end-of-day snapshot cutoff.

        Raises
        ------
        TypeError
            If observations are not InflationObservation records or inputs have unsupported types.
        ValueError
            If the identifier is blank, months are not canonical/strictly ordered,
            levels are not finite positive values, availability violates the
            cutoff, or a date/currency is invalid.

        Examples
        --------
        >>> from finstack_quant.core.market_data import InflationIndex
        >>> InflationIndex("US-CPI", [], "USD", "2025-03-01").date_range() is None
        True

        """
        ...

    def with_expected_releases(self, expected_releases: Sequence[InflationExpectedRelease]) -> InflationIndex:
        """
        Return a new snapshot with a replacement expected-release schedule.

        Parameters
        ----------
        expected_releases : Sequence[InflationExpectedRelease]
            Strictly ascending unique first-of-month schedule facts, with
            ``expected_release >= reference_month`` and ``known_on <= knowledge_date``.
            An empty sequence removes the schedule; facts do not assert availability.

        Returns
        -------
        InflationIndex
            New snapshot retaining observations and knowledge cutoff.

        Raises
        ------
        ValueError
            If months are noncanonical or not strictly ordered, release precedes
            the reference month, or a fact was known after the snapshot cutoff.
        TypeError
            If an entry is not an InflationExpectedRelease record.
        """
        ...

    def get_expected_release(self, reference_month: DateLike) -> Optional[InflationExpectedRelease]:
        """
        Look up the schedule assumption for an exact canonical reference month.

        Parameters
        ----------
        reference_month : datetime.date or str
            First calendar day of the requested month; other days are rejected.

        Returns
        -------
        InflationExpectedRelease or None
            Stored schedule fact, or ``None`` when unspecified.

        Raises
        ------
        ValueError
            If the reference date is malformed or not the first day of a month.
        TypeError
            If the reference date is not date-like or an ISO date string.
        """
        ...

    def get_expected_releases(self) -> list[InflationExpectedRelease]:
        """
        Return the configured expected releases in reference-month order.

        Returns
        -------
        list[InflationExpectedRelease]
            Stored schedule facts, empty when no schedule is configured.

        Notes
        -----
        This accessor does not raise.
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
            The validated snapshot, including observations, knowledge cutoff and schedule facts.

        Raises
        ------
        ValueError
            If the JSON is malformed or fails validation.

        Examples
        --------
        >>> from finstack_quant.core.market_data import InflationIndex
        >>> index = InflationIndex("US-CPI", [], "USD", "2025-03-01")
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

    def get_observation(self, reference_month: DateLike) -> InflationObservation:
        """
        Return the exact stored record for a canonical reference month.

        Parameters
        ----------
        reference_month : datetime.date or str
            First calendar day of the requested month; no interpolation or carry.

        Returns
        -------
        InflationObservation
            Stored monthly fixing and its availability date.

        Raises
        ------
        KeyError
            If the month has no stored observation.
        ValueError
            If the date is malformed or is not first-of-month.
        """
        ...

    def qualified_monthly_observation(self, year: int, month: int, knowledge_date: DateLike) -> float:
        """
        Resolve an exact monthly fixing eligible on a query knowledge date.

        Parameters
        ----------
        year : int
            Calendar year of the reference month.
        month : int
            Calendar month number, 1 through 12.
        knowledge_date : datetime.date or str
            Inclusive query cutoff, no later than the snapshot knowledge date.

        Returns
        -------
        float
            Published positive CPI index level.

        Raises
        ------
        KeyError
            If the monthly observation is absent.
        ValueError
            If a date is invalid, the query cutoff exceeds the snapshot cutoff,
            or the observation was unavailable on the query date.
        """
        ...

    def date_range(self) -> Optional[tuple[datetime.date, datetime.date]]:
        """
        ``(first_date, last_date)`` of the stored observations.

        Returns
        -------
        tuple[datetime.date, datetime.date] or None
            First and last reference months, or ``None`` for empty snapshots.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export as a pandas ``DataFrame`` indexed by observation date.

        Returns
        -------
        pd.DataFrame
            Columns ``value`` and ``available_on`` on a reference-month
            ``DatetimeIndex``; only stored observations appear.

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
    def knowledge_date(self) -> datetime.date:
        """
        Inclusive knowledge cutoff of the stored snapshot.

        Returns
        -------
        datetime.date
            Latest query knowledge date authorized by this snapshot.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def observations(self) -> list[InflationObservation]:
        """
        The stored observation history.

        Returns
        -------
        list[InflationObservation]
            Monthly records in strictly ascending reference-month order.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __len__(self) -> int: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Reconstruct the immutable snapshot through its validated canonical JSON when unpickling."""
        ...
    def __repr__(self) -> str: ...
    def _repr_html_(self) -> Optional[str]: ...

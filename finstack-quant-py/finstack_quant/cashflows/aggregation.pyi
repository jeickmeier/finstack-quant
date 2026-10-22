"""
Currency-preserving aggregation of dated cashflows into periods and totals.

Typed bindings for ``finstack_quant_cashflows::aggregation``.
:func:`aggregate_by_period` groups dated flows into reporting periods while
preserving currency separation; :func:`aggregate_cashflows_checked` sums flows
into a single currency, rejecting mismatches; and :func:`calendar_year_ladder`
rolls non-principal / principal / PV totals by calendar year.

Examples
--------
>>> import datetime
>>> from finstack_quant.cashflows.aggregation import aggregate_by_period
>>> from finstack_quant.core.dates import build_periods
>>> from finstack_quant.core.money import Money
>>> flows = [(datetime.date(2025, 1, 1), Money(10.0, "USD")), (datetime.date(2025, 1, 2), Money(5.0, "USD"))]
>>> aggregate_by_period(flows, build_periods("2025M01..M01").periods)["2025M01"]["USD"].amount
15.0

"""

from __future__ import annotations

import datetime
from typing import Any

import pandas as pd

from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import Period
from finstack_quant.core.money import Money

__all__ = [
    "PeriodAggregation",
    "aggregate_by_period",
    "aggregate_cashflows_checked",
    "calendar_year_ladder",
]

class PeriodAggregation:
    """
    Per-period, per-currency totals from :func:`aggregate_by_period` or
    ``CashFlowSchedule.pv_by_period``.

    Read-only mapping ``{period_id_label: {currency_code: Money}}``
    (``agg["2025Q1"]["USD"]``, ``"2025Q1" in agg``, ``len(agg)``) plus a tidy
    ``to_dataframe()`` with one ``(period, currency, amount)`` row per cell.
    Amounts are never FX-converted. Aggregators omit periods receiving no flows;
    empty buckets supplied through JSON remain present.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.cashflows.aggregation import aggregate_by_period
    >>> from finstack_quant.core.dates import build_periods
    >>> from finstack_quant.core.money import Money
    >>> agg = aggregate_by_period(
    ...     [(datetime.date(2025, 3, 15), Money(100.0, "USD"))], build_periods("2025Q1..Q4").periods
    ... )
    >>> agg.periods
    ['2025Q1']
    >>> agg.get_period("2025Q1")["USD"].amount
    100.0
    """

    @property
    def periods(self) -> list[str]:
        """
        Stored period id labels in reporting-period order, including empty buckets.

        Returns
        -------
        list[str]
            Period labels such as ``"2025Q1"``.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    def get_period(self, period: str) -> dict[str, Money] | None:
        """Read the currency totals for one exact period label.

        Parameters
        ----------
        period : str
            Exact case-sensitive display label, such as ``"2025Q1"``. Whitespace
            is not trimmed and aliases are not parsed.

        Returns
        -------
        dict[str, Money] or None
            Independent dictionary of currency totals, with no FX conversion.
            Returns an empty dictionary for a stored empty bucket, or ``None``
            for an absent or malformed label. Editing the dictionary does not
            change the aggregation.

        Notes
        -----
        This accessor does not raise for absent or malformed labels.
        """
        ...

    def get_amount(self, period: str, currency: Currency | str) -> Money | None:
        """
        Total for one ``(period, currency)`` cell.

        Parameters
        ----------
        period : str
            Exact case-sensitive display label (e.g. ``"2025Q1"``); whitespace
            is not trimmed. An absent or malformed label returns ``None``.
        currency : Currency or str
            ISO 4217 currency of the requested total.

        Returns
        -------
        Money or None
            Total in ``currency`` for ``period``; ``None`` when absent.

        Raises
        ------
        ValueError
            If ``currency`` is not a valid ISO 4217 code.
        """
        ...

    def to_dict(self) -> dict[str, dict[str, Money]]:
        """
        Nested ``{period_id_label: {currency_code: Money}}`` dictionary.

        Returns
        -------
        dict[str, dict[str, Money]]
            Plain-dict copy of the totals.

        Notes
        -----
        This method does not raise.
        """
        ...

    def __getitem__(self, period: str) -> dict[str, Money]:
        """
        ``{currency_code: Money}`` for one period label.

        Parameters
        ----------
        period : str
            Exact case-sensitive display label; whitespace is not trimmed.

        Returns
        -------
        dict[str, Money]
            Per-currency totals of that period.

        Raises
        ------
        KeyError
            If the exact period label is absent.
        """
        ...

    def __contains__(self, period: str) -> bool:
        """
        Whether the exact period label is present, including an empty bucket.

        Parameters
        ----------
        period : str
            Exact case-sensitive display label; whitespace is not trimmed.

        Returns
        -------
        bool
            ``True`` when the period is present.

        Notes
        -----
        This method does not raise.
        """
        ...

    def __len__(self) -> int:
        """
        Number of stored periods, including empty buckets.

        Returns
        -------
        int
            Count of period entries.

        Notes
        -----
        This method does not raise.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to JSON (``{period: {currency: money}}``).

        Returns
        -------
        str
            JSON document; round-trips through :meth:`from_json`.

        Raises
        ------
        ValueError
            If a total cannot be represented in JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> PeriodAggregation:
        """
        Deserialize from the JSON produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            JSON document.

        Returns
        -------
        PeriodAggregation
            Reconstructed totals whose currency keys match their monetary amounts.

        Raises
        ------
        ValueError
            If the JSON is malformed or a currency key differs from its Money currency.

        Examples
        --------
        >>> from finstack_quant.cashflows.aggregation import PeriodAggregation
        >>> len(PeriodAggregation.from_json("{}"))
        0
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Pickle support via the JSON wire form.

        Returns
        -------
        tuple
            ``(from_json, (json,))`` reconstructor pair.

        Raises
        ------
        ValueError
            If a total cannot be represented in JSON.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Tidy ``pandas.DataFrame`` with columns ``period, currency, amount``.

        Returns
        -------
        pandas.DataFrame
            One row per non-empty ``(period, currency)`` cell in
            reporting-period order; ``amount`` is a float in ``currency``
            units.

        Notes
        -----
        This method does not raise.
        """
        ...

    def __repr__(self) -> str: ...

def aggregate_by_period(
    flows: list[tuple[datetime.date, Money]],
    periods: list[Period],
) -> PeriodAggregation:
    """
    Aggregate dated flows into reporting periods (half-open ``[start, end)``).

    Parameters
    ----------
    flows : list[tuple[datetime.date, Money]]
        Dated cashflows; unsorted input is sorted internally.
    periods : list[Period]
        Sorted, disjoint reporting periods.

    Returns
    -------
    PeriodAggregation
        Mapping-like ``{period_id_label: {currency_code: nominal_sum}}`` with
        ``to_dataframe()``; periods with no flows are omitted.

    Raises
    ------
    ValueError
        If periods are unsorted, overlapping, or contain duplicate ids, or
        if a per-currency total is non-finite or exceeds the Decimal range.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.cashflows.aggregation import aggregate_by_period
    >>> from finstack_quant.core.dates import build_periods
    >>> from finstack_quant.core.money import Money
    >>> flows = [(datetime.date(2025, 3, 15), Money(100.0, "USD"))]
    >>> periods = build_periods("2025Q1..Q4").periods
    >>> aggregate_by_period(flows, periods)["2025Q1"]["USD"].amount
    100.0
    """
    ...

def aggregate_cashflows_checked(
    flows: list[tuple[datetime.date, Money]],
    target: Currency | str,
) -> Money:
    """
    Currency-checked single-currency aggregation with an explicit target currency.

    Parameters
    ----------
    flows : list[tuple[datetime.date, Money]]
        Dated cashflows; every flow must be in ``target`` currency.
    target : Currency or str
        Required currency for every flow and the returned total.

    Returns
    -------
    Money
        Single total in ``target`` currency. Empty ``flows`` returns a zero
        total.

    Raises
    ------
    ValueError
        If any flow currency differs from ``target``; currency mismatches
        are rejected, never silently converted.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.cashflows.aggregation import aggregate_cashflows_checked
    >>> from finstack_quant.core.money import Money
    >>> flows = [(datetime.date(2025, 1, 15), Money(50_000.0, "USD"))]
    >>> aggregate_cashflows_checked(flows, "USD").amount
    50000.0
    """
    ...

def calendar_year_ladder(
    dates: list[datetime.date],
    kinds: list[str],
    amounts: list[float],
    pvs: list[float],
) -> list[tuple[int, float, float, float]]:
    """
    Group dated cashflows into a calendar-year non-principal / principal / PV ladder.

    Returns raw ``(year, non_principal, principal, pv)`` tuples over parallel
    array inputs; use :meth:`finstack_quant.cashflows.builder.CashFlowSchedule.calendar_year_ladder`
    for a ``pandas.DataFrame`` view of a built schedule.

    Parameters
    ----------
    dates : list[datetime.date]
        Payment dates; the Gregorian year of each date is the bucket.
    kinds : list[str]
        Canonical cashflow kind labels (``"fixed"``, ``"notional"``,
        ``"pre_payment"``). Surrounding whitespace and ASCII case are ignored.
        Unknown labels raise ``ValueError``.
    amounts : list[float]
        Signed finite cashflow amounts, one per date, in native currency units.
    pvs : list[float]
        Finite present values, one per date, in the same units as ``amounts``.

    Returns
    -------
    list[tuple[int, float, float, float]]
        One ``(year, non_principal, principal, pv)`` row per calendar year,
        sorted by year.

    Raises
    ------
    ValueError
        If the four lists have different lengths, a kind label is unknown, or
        an amount or PV is non-finite.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.cashflows.aggregation import calendar_year_ladder
    >>> calendar_year_ladder(
    ...     [datetime.date(2027, 3, 15), datetime.date(2034, 3, 15)],
    ...     ["fixed", "notional"],
    ...     [100.0, 1000.0],
    ...     [90.0, 700.0],
    ... )
    [(2027, 100.0, 0.0, 90.0), (2034, 0.0, 1000.0, 700.0)]
    """
    ...

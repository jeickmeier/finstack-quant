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
    Amounts are never FX-converted; empty periods are omitted.

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
    """

    @property
    def periods(self) -> list[str]:
        """
        Period id labels with at least one flow, in reporting-period order.

        Returns
        -------
        list[str]
            Period labels such as ``"2025Q1"``.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    def get_amount(self, period: str, currency: Currency | str) -> Money | None:
        """
        Total for one ``(period, currency)`` cell.

        Parameters
        ----------
        period : str
            Period id label (e.g. ``"2025Q1"``).
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
            Period id label.

        Returns
        -------
        dict[str, Money]
            Per-currency totals of that period.

        Raises
        ------
        KeyError
            If ``period`` has no flows.
        """
        ...

    def __contains__(self, period: str) -> bool:
        """
        Whether ``period`` has at least one flow.

        Parameters
        ----------
        period : str
            Period id label.

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
        Number of non-empty periods.

        Returns
        -------
        int
            Count of periods with flows.

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
            Reconstructed totals.

        Raises
        ------
        ValueError
            If the JSON is malformed.

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

    Parameters
    ----------
    dates : list[datetime.date]
        Payment dates; the Gregorian year of each date is the bucket.
    kinds : list[str]
        Cashflow kind labels (``"fixed"``, ``"notional"``, ``"coupon"``,
        ``"principal"``, …). ASCII case is ignored. Unknown labels raise
        ``ValueError``.
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
    ...     ["coupon", "principal"],
    ...     [100.0, 1000.0],
    ...     [90.0, 700.0],
    ... )
    [(2027, 100.0, 0.0, 90.0), (2034, 0.0, 1000.0, 700.0)]
    """
    ...

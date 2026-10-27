"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/typed_structured_credit/tranche_cashflows.rs``."""

from __future__ import annotations

import datetime
from typing import Any
import pandas as pd
from finstack_quant.core.money import Money

class TrancheCashflows:
    """
    Projected cashflows of one tranche from a deterministic simulation
    (:meth:`StructuredCredit.tranche_cashflows`'s return value): the total
    flows and their interest, principal, PIK, deferred-interest and
    write-down components, plus the accrual periods behind them.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.dates import DayCount
    >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import (
    ...     AssetPool,
    ...     PoolAsset,
    ...     StandardDealParams,
    ...     StructuredCredit,
    ...     Tranche,
    ...     TrancheStructure,
    ... )
    >>> as_of, maturity = datetime.date(2024, 1, 15), datetime.date(2031, 1, 15)
    >>> pool = AssetPool("POOL-1", "clo", Currency("USD")).with_assets([
    ...     PoolAsset.fixed_rate_bond("LOAN-1", Money(80_000_000.0, Currency("USD")), 0.07, maturity, DayCount.ACT_360)
    ... ])
    >>> note = (
    ...     Tranche
    ...     .builder()
    ...     .id("A")
    ...     .attach_pct(0.0)
    ...     .detach_pct(100.0)
    ...     .seniority("senior")
    ...     .original_balance(Money(80_000_000.0, Currency("USD")))
    ...     .coupon_fixed(0.05)
    ...     .maturity(maturity)
    ...     .build()
    ... )
    >>> deal = StructuredCredit.new_standard(
    ...     "clo",
    ...     StandardDealParams(
    ...         "CLO-1",
    ...         pool,
    ...         TrancheStructure([note]),
    ...         as_of,
    ...         maturity,
    ...         "USD-SOFR-DISC",
    ...         calendar_id="nyse",
    ...     ),
    ... )
    >>> market = MarketContext().insert(DiscountCurve.flat("USD-SOFR-DISC", as_of, 0.03))
    >>> flows = deal.tranche_cashflows("A", market, as_of)
    >>> flows.tranche_id, list(flows.to_dataframe().columns)
    ('A', ['date', 'cashflow', 'interest', 'principal', 'pik', 'deferred', 'writedown'])
    """

    @staticmethod
    def from_json(json: str) -> TrancheCashflows:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``TrancheCashflows`` (the shape ``to_json`` writes).

        Returns
        -------
        TrancheCashflows
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import TrancheCashflows
        >>> try:
        ...     TrancheCashflows.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            JSON-encoded ``TrancheCashflows``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust value.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(TrancheCashflows.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            A one-line summary of the value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        One row per payment date as a pandas ``DataFrame``.

        Columns: ``date`` (ISO 8601 string), ``cashflow`` (total paid),
        ``interest``, ``principal``, ``pik``, ``deferred`` and ``writedown``,
        all in currency units; a component absent on a date is ``0.0``.

        Returns
        -------
        pd.DataFrame
            The projected flows in date order.

        Raises
        ------
        ValueError
            If component amounts have different currencies. Empty results
            preserve column order and numeric dtypes.
        """
        ...

    @property
    def tranche_id(self) -> str:
        """
        Tranche identifier.

        Returns
        -------
        str
            The class identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def cashflows(self) -> list[tuple[datetime.date, Money]]:
        """
        Total cash paid per payment date.

        Returns
        -------
        list[tuple[datetime.date, Money]]
            ``(date, amount)`` pairs.

        Raises
        ------
        ValueError
            If a date cannot be converted.
        """
        ...

    @property
    def interest_flows(self) -> list[tuple[datetime.date, Money]]:
        """
        Interest paid per payment date.

        Returns
        -------
        list[tuple[datetime.date, Money]]
            ``(date, amount)`` pairs.

        Raises
        ------
        ValueError
            If a date cannot be converted.
        """
        ...

    @property
    def principal_flows(self) -> list[tuple[datetime.date, Money]]:
        """
        Principal paid per payment date.

        Returns
        -------
        list[tuple[datetime.date, Money]]
            ``(date, amount)`` pairs.

        Raises
        ------
        ValueError
            If a date cannot be converted.
        """
        ...

    @property
    def pik_flows(self) -> list[tuple[datetime.date, Money]]:
        """
        Interest capitalized (PIK) per payment date.

        Returns
        -------
        list[tuple[datetime.date, Money]]
            ``(date, amount)`` pairs.

        Raises
        ------
        ValueError
            If a date cannot be converted.
        """
        ...

    @property
    def deferred_flows(self) -> list[tuple[datetime.date, Money]]:
        """
        Interest deferred per payment date.

        Returns
        -------
        list[tuple[datetime.date, Money]]
            ``(date, amount)`` pairs.

        Raises
        ------
        ValueError
            If a date cannot be converted.
        """
        ...

    @property
    def writedown_flows(self) -> list[tuple[datetime.date, Money]]:
        """
        Principal written down per payment date.

        Returns
        -------
        list[tuple[datetime.date, Money]]
            ``(date, amount)`` pairs.

        Raises
        ------
        ValueError
            If a date cannot be converted.
        """
        ...

    @property
    def accrual_periods(self) -> list[dict[str, Any]]:
        """
        Accrual periods as ``TrancheAccrualPeriod`` serde dicts.

        Returns
        -------
        list[dict[str, Any]]
            One dict per accrual period.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def detailed_flows(self) -> list[dict[str, Any]]:
        """
        Detailed classified flows as ``CashFlow`` serde dicts.

        Returns
        -------
        list[dict[str, Any]]
            One dict per flow.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def final_balance(self) -> Money:
        """
        Balance outstanding after the last projected payment.

        Returns
        -------
        Money
            The final balance.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_interest(self) -> Money:
        """
        Total interest paid over the projection.

        Returns
        -------
        Money
            The interest total.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_principal(self) -> Money:
        """
        Total principal paid over the projection.

        Returns
        -------
        Money
            The principal total.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_pik(self) -> Money:
        """
        Total interest capitalized over the projection.

        Returns
        -------
        Money
            The PIK total.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_deferred(self) -> Money:
        """
        Total interest deferred as a claim over the projection.

        Returns
        -------
        Money
            The deferred total.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_writedown(self) -> Money:
        """
        Total principal written down over the projection.

        Returns
        -------
        Money
            The write-down total.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

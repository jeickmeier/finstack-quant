"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/typed_structured_credit/diagnostics.rs``."""

from __future__ import annotations

import datetime
from typing import Any
import pandas as pd
from finstack_quant.core.money import Money

class SimulationDiagnostics:
    """
    Deal-level accounting of one deterministic simulation
    (the diagnostics member of :meth:`StructuredCredit.run_simulation`'s result):
    the per-period pool, collections, cash-account and coverage-test record
    plus the reserve-account and draw-funding totals.

    Examples
    --------
    >>> import datetime
    >>> import json
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.dates import DayCount
    >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import (
    ...     AssetPool,
    ...     RepLine,
    ...     StandardDealParams,
    ...     StructuredCredit,
    ...     Tranche,
    ...     TrancheStructure,
    ... )
    >>> as_of, maturity = datetime.date(2024, 1, 15), datetime.date(2031, 1, 15)
    >>> pool = AssetPool("POOL-1", "abs", Currency("USD")).with_rep_lines([
    ...     RepLine(
    ...         "LINE-1",
    ...         Money(80_000_000.0, Currency("USD")),
    ...         0.07,
    ...         maturity,
    ...         12,
    ...         DayCount.ACT_360,
    ...         asset_type={"type": "first_lien_loan"},
    ...     )
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
    ...     "abs", StandardDealParams("ABS-1", pool, TrancheStructure([note]), as_of, maturity, "USD-SOFR-DISC")
    ... )
    >>> envelope = json.loads(deal.to_json())
    >>> envelope["instrument"]["spec"]["calendar_id"] = "nyse"
    >>> deal = StructuredCredit.from_json(json.dumps(envelope))
    >>> market = MarketContext().insert(DiscountCurve.flat("USD-SOFR-DISC", as_of, 0.03))
    >>> diagnostics = deal.run_simulation(market, as_of).diagnostics
    >>> diagnostics.unfunded_draws.amount
    0.0
    >>> list(diagnostics.to_dataframe().columns)[:3]
    ['date', 'pool_balance', 'pool_factor']
    >>> len(diagnostics.periods) == len(diagnostics.to_dataframe())
    True
    """

    @staticmethod
    def from_json(json: str) -> SimulationDiagnostics:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``SimulationDiagnostics`` (the shape ``to_json`` writes).

        Returns
        -------
        SimulationDiagnostics
            The decoded diagnostics.

        Raises
        ------
        ValueError
            If ``json`` is not valid JSON for the diagnostics shape.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import SimulationDiagnostics
        >>> try:
        ...     SimulationDiagnostics.from_json("{}")
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
            JSON-encoded diagnostics.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust diagnostics.

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
            ``(SimulationDiagnostics.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...
    @property
    def periods(self) -> list[dict[str, Any]]:
        """
        Per-period deal record as ``PeriodDiagnostics`` serde dicts.

        Returns
        -------
        list[dict[str, Any]]
            ``payment_date``, ``pool_balance``, ``pool_factor``, ``wac``, ``weighted_avg_spread_bp``, ``warf``, the period's collections, defaults, recoveries, reinvested par, fees paid, the cash-account balances, ``delinquent_balance``, ``servicer_advances_outstanding``, ``excess_spread`` and the ``coverage_tests`` the executor evaluated.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        One row per simulated period as a pandas ``DataFrame``.

        Columns: ``date`` (ISO 8601 string), ``pool_balance``, ``pool_factor``,
        ``wac`` (decimal), ``weighted_avg_spread_bp``, ``warf``,
        ``interest_collections``, ``principal_collections``, ``defaults``,
        ``recoveries``, ``reinvested_par``, ``fees_paid``, ``reserve_balance``,
        ``reserve_interest``, ``spread_account``, ``funding_account``,
        ``delinquent_balance``, ``servicer_advances_outstanding`` and
        ``excess_spread`` (annualized decimal);
        amounts in their stored currency units without FX conversion. Rows keep
        stored period order and duplicate dates. Reserve interest uses the first
        matching date in ``reserve_interest_paid``; missing matches are zero.
        An empty result retains all nineteen columns with a string date and
        float64 numeric dtypes.

        Returns
        -------
        pd.DataFrame
            The period record.

        Raises
        ------
        ValueError
            If the canonical table cannot be built or converted.
        """
        ...

    def coverage_tests_dataframe(self) -> pd.DataFrame:
        """
        Coverage-test evaluations as a long pandas ``DataFrame``.

        Columns: ``date`` (ISO 8601 string), ``test_id``, ``ratio``,
        ``trigger_level``, ``cushion`` (ratio minus trigger) and ``passing``.

        Returns
        -------
        pd.DataFrame
            One row per test per period.

        Raises
        ------
        ValueError
            If the rows cannot be serialized.
        """
        ...
    @property
    def reserve_balance_path(self) -> list[tuple[datetime.date, Money]]:
        """
        Reserve-account balance at the end of each simulated period.

        Returns
        -------
        list[tuple[datetime.date, Money]]
            ``(date, balance)`` pairs in period order.

        Raises
        ------
        ValueError
            If a date cannot be converted.
        """
        ...
    @property
    def reserve_interest_paid(self) -> list[tuple[datetime.date, Money]]:
        """
        Reserve interest earned each period, before routing.

        Returns
        -------
        list[tuple[datetime.date, Money]]
            ``(date, interest)`` pairs in period order.

        Raises
        ------
        ValueError
            If a date cannot be converted.
        """
        ...
    @property
    def draws_from_reserve(self) -> Money:
        """
        Collateral draws funded from the reserve account over the simulation.

        Returns
        -------
        Money
            Currency-tagged amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def draws_from_principal(self) -> Money:
        """
        Collateral draws funded from principal collections over the simulation.

        Returns
        -------
        Money
            Currency-tagged amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def unfunded_draws(self) -> Money:
        """
        Collateral draws that could not be funded over the simulation.

        Returns
        -------
        Money
            Currency-tagged amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def reserve_replenished(self) -> Money:
        """
        Revolver repayments diverted to replenish the reserve over the simulation.

        Returns
        -------
        Money
            Currency-tagged amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def early_amortization_date(self) -> datetime.date | None:
        """
        Payment date of the early-amortization event (or coverage-test
        acceleration) that ended the revolving period.

        Returns
        -------
        datetime.date | None
            ``None`` when no event fired.

        Raises
        ------
        ValueError
            If the date cannot be converted.
        """
        ...

    @property
    def tranche_draws(self) -> list[tuple[str, datetime.date, Money]]:
        """
        Lender draws applied to notes.

        Returns
        -------
        list[tuple[str, datetime.date, Money]]
            ``(tranche_id, payment date, amount)`` triples in date order.

        Raises
        ------
        ValueError
            If a date cannot be converted.
        """
        ...

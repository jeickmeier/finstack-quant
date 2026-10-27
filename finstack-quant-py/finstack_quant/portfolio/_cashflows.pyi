"""Stub declarations mirroring ``finstack-quant-py/src/bindings/portfolio/cashflows.rs``."""

from __future__ import annotations

import datetime
from typing import Any
import pandas as pd
from finstack_quant.core.currency import Currency
from finstack_quant.core.market_data import MarketContext
from finstack_quant.portfolio._container import Portfolio

class PortfolioCashflows:
    """
    Typed wrapper around a ``PortfolioCashflows`` ladder.

    Returned by :func:`aggregate_full_cashflows`; survives multiple drill-in
    calls (``events_json``, ``by_date_json``, ``issues_json``,
    :meth:`collapse_to_base_by_date_kind`) without re-parsing.

    Examples
    --------
    >>> from finstack_quant.portfolio import PortfolioCashflows
    >>> cashflows = PortfolioCashflows.from_json(
    ...     '{"events":[],"by_position":{},"by_date":{},"position_summaries":{},"issues":[],"fx_collapse_policy":"cip_forward"}'
    ... )
    >>> (len(cashflows.by_position), len(cashflows.issues))
    (0, 0)
    """

    @staticmethod
    def from_json(cashflows_json: str) -> PortfolioCashflows:
        """
        Deserialize a cashflow ladder from JSON.

        Parameters
        ----------
        cashflows_json : str
            Canonical classified-cashflow payload returned by the aggregation
            API or an equivalent serialized ladder.

        Returns
        -------
        PortfolioCashflows
            Validated `PortfolioCashflows` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.portfolio import PortfolioCashflows
        >>> cashflows = PortfolioCashflows.from_json(
        ...     '{"events":[],"by_position":{},"by_date":{},"position_summaries":{},"issues":[],"fx_collapse_policy":"cip_forward"}'
        ... )
        >>> (len(cashflows.by_position), len(cashflows.issues))
        (0, 0)
        """
        ...

    def to_json(self) -> str:
        """
        Serialize the full cashflow ladder to canonical JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PortfolioCashflows`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def events_json(self) -> str:
        """
        Return all dated cashflow events as JSON.
        Returns
        -------
        str
            Compact JSON array of all dated cashflow events in ladder order.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def by_date_json(self) -> str:
        """
        Return cashflows grouped by date as JSON.
        Returns
        -------
        str
            Compact JSON for totals grouped by payment date, currency, and cashflow kind.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def issues_json(self) -> str:
        """
        Return cashflow aggregation or FX-conversion issues as JSON.
        Returns
        -------
        str
            Compact JSON array of cashflow-schedule extraction issues.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the flat ``events`` ladder as a pandas DataFrame.

        One row per dated cashflow event, in the ladder's canonical
        payment-date order. ``amount`` is flattened to a float column plus a
        ``currency`` column rather than a nested dict, so a multi-currency
        ladder stays currency-safe: group by ``currency`` before summing.

        Columns: ``position_id``, ``instrument_id``, ``instrument_type``,
        ``date`` (ISO 8601 string), ``amount`` (position-scaled), ``currency``,
        ``kind`` (``"fixed"``, ``"float_reset"``, ``"notional"``, ...),
        ``reset_date`` (ISO 8601 string, ``None`` outside floating coupons),
        ``accrual_factor``, ``rate`` (``None`` when the event carries no rate).

        Returns
        -------
        pd.DataFrame
            One row per event; ``len(df)`` equals ``len(cashflows)``.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    @property
    def events(self) -> list[dict[str, Any]]:
        """
        Flat position-scaled events in payment-date order.

        Returns
        -------
        list[dict[str, Any]]
            JSON-shaped events with ``position_id``, ``instrument_id``,
            ``instrument_type``, ``date``, ``amount`` (``{"amount",
            "currency"}``), ``kind``, ``reset_date``, ``accrual_factor``,
            ``rate``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def by_position(self) -> dict[str, list[dict[str, Any]]]:
        """
        Per-position event drill-down.

        Returns
        -------
        dict[str, list[dict[str, Any]]]
            Events keyed by position id.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def by_date(self) -> dict[str, dict[str, dict[str, dict[str, str]]]]:
        """
        Totals by payment date, then ISO currency, then cashflow kind.

        Returns
        -------
        dict[str, dict[str, dict[str, dict[str, str]]]]
            ``{date_iso: {currency: {kind: {"amount", "currency"}}}}``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def issues(self) -> list[dict[str, Any]]:
        """
        Extraction issues recorded during aggregation.

        Returns
        -------
        list[dict[str, Any]]
            Entries with ``position_id``, ``instrument_id``,
            ``instrument_type`` and ``message``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def fx_collapse_policy(self) -> str:
        """
        FX policy stamped for :meth:`collapse_to_base_by_date_kind`.

        Returns
        -------
        str
            Serde name of the applied ``CashflowFxPolicy``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_issues_dataframe(self) -> pd.DataFrame:
        """
        Export the extraction issues as a ``DataFrame``.

        Returns
        -------
        pd.DataFrame
            Columns ``position_id``, ``instrument_id``, ``instrument_type``,
            ``message``; zero rows when there are no issues.

        Raises
        ------
        ValueError
            If the frame cannot be built.
        """
        ...

    def net_in_currency_by_date(self, currency: Currency | str) -> list[tuple[str, float]]:
        """
        Net same-currency amounts across kinds for each payment date.

        Parameters
        ----------
        currency : Currency | str
            ISO-4217 currency whose per-date kind buckets are summed.

        Returns
        -------
        list[tuple[str, float]]
            ``(ISO date, net amount)`` pairs in ladder order; dates with no
            flows in ``currency`` are omitted.

        Raises
        ------
        ValueError
            If ``currency`` is not a valid ISO-4217 code.
        """
        ...

    def collapse_to_base_by_date_kind(
        self,
        market: MarketContext | str,
        base_currency: Currency | str,
        as_of: datetime.date | str,
        discount_curves: dict[str, str] | None = None,
    ) -> pd.DataFrame:
        """
        Collapse the ladder to a base-currency ``(date, kind)`` table.

        Payments on or before ``as_of`` use spot FX at ``as_of``. Later
        payments use the CIP forward ``F(T) = S × DF_from(T) / DF_base(T)``.

        Parameters
        ----------
        market : MarketContext or str
            Market context object or JSON providing the FX matrix and
            discount curves used for spot and CIP-forward conversion.
        base_currency : Currency | str
            Currency into which each classified cashflow is converted.
        as_of : datetime.date | str
            Spot/forward split date, either a date-like object or an ISO
            8601 string.
        discount_curves : dict[str, str] | None
            Optional map of ISO currency code to discount-curve id. A missing
            map or missing currency uses the ISO code as the curve id
            (``market.get_discount("EUR")``).

        Returns
        -------
        pd.DataFrame
            Columns ``date`` (ISO 8601 string), ``kind``, ``amount`` (float in
            ``base_currency``), ``currency``; one row per ``(date, kind)``.

        Raises
        ------
        TypeError
            If ``market`` is neither a ``MarketContext`` nor a JSON string.
        ValueError
            If the market JSON, currency, date, or a ``discount_curves`` key is
            invalid.
        PortfolioError
            If an FX rate, discount curve, or discount factor required for
            base-currency conversion is unavailable.
        """
        ...

    def collapse_to_base_by_date_kind_json(
        self,
        market: MarketContext | str,
        base_currency: Currency | str,
        as_of: datetime.date | str,
        discount_curves: dict[str, str] | None = None,
    ) -> str:
        """
        Wire twin of :meth:`collapse_to_base_by_date_kind`.

        Parameters
        ----------
        market : MarketContext or str
            Market context object or JSON providing FX and discount curves.
        base_currency : Currency | str
            Target currency of the collapsed ladder.
        as_of : datetime.date | str
            Spot/forward split date.
        discount_curves : dict[str, str] | None
            Optional ``{currency_code: curve_id}`` overrides.

        Returns
        -------
        str
            Nested ``{date: {kind: {"amount", "currency"}}}`` JSON.

        Raises
        ------
        PortfolioError
            If an FX rate or discount factor required for conversion is
            unavailable.
        """
        ...

    def __len__(self) -> int:
        """Number of cashflow events.
        Returns
        -------
        int
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

def aggregate_full_cashflows(
    portfolio: Portfolio | str,
    market: MarketContext | str,
    allow_partial: bool | None = None,
) -> PortfolioCashflows:
    """
    Build the full classified cashflow ladder for the portfolio.

    Returns a typed :class:`PortfolioCashflows` wrapper; call ``to_json()``
    to get the raw ladder or use the typed accessors to drill in.

    Parameters
    ----------
    portfolio : Portfolio or str
        Built portfolio or canonical ``PortfolioSpec`` JSON to expand.
    market : MarketContext or str
        Market context object or JSON needed for instrument cashflow generation.
    allow_partial : bool or None, default None
        ``None`` uses the Rust ``CashflowAggregationOptions`` default,
        ``False``: any schedule-construction issue aborts the call. When
        ``True``, remaining positions still contribute to the ladder and
        issues are returned on :class:`PortfolioCashflows`.

    Returns
    -------
    PortfolioCashflows
        Typed event ladder with per-position, per-date, and issue drill-downs.

    Raises
    ------
    TypeError
        If ``portfolio`` or ``market`` is neither its typed wrapper nor a JSON string.
    ValueError
        If supplied JSON is malformed or cashflow aggregation cannot represent
        a monetary result.
    PortfolioError
        If a JSON portfolio cannot be constructed from its decoded specification,
        or if any position fails schedule construction while ``allow_partial``
        is ``False``.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import Portfolio, aggregate_full_cashflows
    >>> spec = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
    >>> cashflows = aggregate_full_cashflows(Portfolio.from_spec(spec), MarketContext())
    >>> (len(cashflows.by_position), len(cashflows.issues))
    (0, 0)
    """
    ...

def net_in_currency_by_date(
    cashflows: PortfolioCashflows | str,
    currency: Currency | str,
) -> list[tuple[str, float]]:
    """
    Net same-currency cashflow amounts across kinds for each payment date.

    Parameters
    ----------
    cashflows : PortfolioCashflows | str
        A :class:`PortfolioCashflows` ladder (no parse), full cashflow-ladder
        JSON, or a ``{date: {ccy: {kind: money}}}`` object (optionally wrapped
        as ``{"by_date": ...}``). Kind keys are opaque strings. Money objects
        require a finite decimal-string ``amount`` and a ``currency`` matching
        their enclosing bucket. Numeric JSON amounts are rejected by the
        canonical Money wire contract. All ISO payment dates and currency
        buckets are validated, including non-output currencies.
    currency : Currency | str
        ISO-4217 code selecting which per-date currency bucket to net.

    Returns
    -------
    list[tuple[str, float]]
        ``(ISO date, net amount)`` pairs sorted by date. Dates with no flows
        in ``currency`` are omitted.

    Raises
    ------
    TypeError
        If ``cashflows`` is neither a ``PortfolioCashflows`` nor a string.
    ValueError
        If the requested ``currency`` is unknown.
    PortfolioError
        If cashflow JSON is malformed, a date/currency bucket is invalid, a
        container is not an object, a money amount is malformed or non-finite,
        a money currency differs from its bucket, or a bucket total overflows.

    Examples
    --------
    >>> from finstack_quant.portfolio import net_in_currency_by_date
    >>> net_in_currency_by_date(
    ...     '{"by_date":{"2025-01-15":{"USD":{"notional":{"amount":"-100","currency":"USD"}}}}}',
    ...     "USD",
    ... )
    [('2025-01-15', -100.0)]
    """
    ...

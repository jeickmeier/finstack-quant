"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/typed_structured_credit/tranche_analytics.rs``."""

from __future__ import annotations

import datetime
from typing import Any
import pandas as pd
from finstack_quant.core.market_data import MarketContext
from finstack_quant.valuations.instruments._typed_structured_credit.structured_credit import StructuredCredit

class OasResult:
    """
    Result of an option-adjusted-spread calculation for a structured-credit
    tranche (``tranche_oas``'s return value).

    Examples
    --------
    >>> import json
    >>> from finstack_quant.valuations.instruments import OasResult
    >>> payload = json.dumps({
    ...     "oas": 0.0125,
    ...     "model_price": 99.5,
    ...     "market_price": 98.75,
    ...     "num_paths": 256,
    ...     "price_std_error": 0.05,
    ... })
    >>> result = OasResult.from_json(payload)
    >>> result.oas
    0.0125
    """

    @staticmethod
    def from_json(json: str) -> OasResult:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``OasResult``.

        Returns
        -------
        OasResult
            The decoded result.

        Raises
        ------
        ValueError
            If ``json`` is not valid JSON for the ``OasResult`` shape.

        Examples
        --------
        >>> import json
        >>> from finstack_quant.valuations.instruments import OasResult
        >>> result = OasResult.from_json(
        ...     json.dumps({
        ...         "oas": 0.0125,
        ...         "model_price": 99.5,
        ...         "market_price": 98.75,
        ...         "num_paths": 256,
        ...         "price_std_error": 0.05,
        ...     })
        ... )
        >>> (result.oas, result.model_price, result.num_paths)
        (0.0125, 99.5, 256)

        """
        ...

    def to_json(self) -> str:
        """
        Serialize back to the same JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            JSON-encoded ``OasResult``.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def oas(self) -> float:
        """
        Option-adjusted spread, as an annual decimal (``0.01`` = 100 bp).

        Returns
        -------
        float
            The option-adjusted spread.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def model_price(self) -> float:
        """
        Model price at the solved OAS, as a percentage of current balance.

        Returns
        -------
        float
            The model price.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def market_price(self) -> float:
        """
        Target market price, as a percentage of current balance.

        Returns
        -------
        float
            The target market price.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def num_paths(self) -> int:
        """
        Number of Monte-Carlo scenarios used.

        Returns
        -------
        int
            The scenario count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def price_std_error(self) -> float:
        """
        Monte-Carlo standard error of the mean price, as a percentage of
        current balance.

        Returns
        -------
        float
            The standard error.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export as a single-row pandas DataFrame.

        Columns: ``oas`` (annual decimal), ``model_price`` and
        ``market_price`` (percentage of current balance), ``num_paths``,
        ``price_std_error`` (percentage of current balance).

        Returns
        -------
        pd.DataFrame
            Single-row DataFrame of the OAS solve, so a book of tranches
            stacks with ``pd.concat``.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            Reconstruction callable and canonical JSON arguments.

        Raises
        ------
        ValueError
            If serialization of the stored value fails.
        """
        ...

class TrancheMetrics:
    """
    Per-tranche valuation, yield, spread and risk metrics
    (``tranche_metrics``'s return value).

    Prices are quoted per the tranche's current balance; an impaired note
    reports zero prices and spreads and ``ytm`` ``None``.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.valuations.instruments import TrancheMetrics
    >>> payload = json.dumps({
    ...     "tranche_id": "A",
    ...     "currency": "USD",
    ...     "pv": 1000.0,
    ...     "clean_price_pct": 100.0,
    ...     "dirty_price_pct": 100.0,
    ...     "accrued": 0.0,
    ...     "factor": 1.0,
    ...     "wal": 3.0,
    ...     "ytm": 0.05,
    ...     "z_spread_bp": 0.0,
    ...     "cs01": -1.0,
    ...     "spread_duration": 3.0,
    ...     "spread_convexity": 12.0,
    ...     "effective_duration": 3.0,
    ...     "effective_convexity": 12.0,
    ...     "target_price_pct": 100.0,
    ... })
    >>> metrics = TrancheMetrics.from_json(payload)
    >>> metrics.tranche_id
    'A'
    """

    @staticmethod
    def from_json(json: str) -> TrancheMetrics:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``TrancheMetrics``.

        Returns
        -------
        TrancheMetrics
            The decoded metrics bundle.

        Raises
        ------
        ValueError
            If ``json`` is not valid JSON for the ``TrancheMetrics`` shape.

        Examples
        --------
        >>> import json
        >>> from finstack_quant.valuations.instruments import TrancheMetrics
        >>> payload = {
        ...     "tranche_id": "A",
        ...     "currency": "USD",
        ...     "pv": 1000.0,
        ...     "clean_price_pct": 100.0,
        ...     "dirty_price_pct": 100.0,
        ...     "accrued": 0.0,
        ...     "factor": 1.0,
        ...     "wal": 3.0,
        ...     "ytm": 0.05,
        ...     "z_spread_bp": 0.0,
        ...     "cs01": -1.0,
        ...     "spread_duration": 3.0,
        ...     "spread_convexity": 12.0,
        ...     "effective_duration": 3.0,
        ...     "effective_convexity": 12.0,
        ...     "target_price_pct": 100.0,
        ... }
        >>> metrics = TrancheMetrics.from_json(json.dumps(payload))
        >>> (metrics.tranche_id, metrics.currency, metrics.pv)
        ('A', 'USD', 1000.0)

        """
        ...

    def to_json(self) -> str:
        """
        Serialize back to the same JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            JSON-encoded ``TrancheMetrics``.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def tranche_id(self) -> str:
        """
        Identifier of the tranche.

        Returns
        -------
        str
            The tranche identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def currency(self) -> str:
        """
        ISO-4217 code of the currency ``pv``, ``accrued`` and ``cs01`` are
        denominated in.

        Returns
        -------
        str
            The ISO-4217 currency code.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def pv(self) -> float:
        """
        Present value of the tranche, in ``currency`` units.

        Returns
        -------
        float
            The present value.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def clean_price_pct(self) -> float:
        """
        Model clean settlement price, as a percentage of the tranche's current
        balance (the factor-adjusted secondary-market quote basis).

        Returns
        -------
        float
            The model clean price.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def dirty_price_pct(self) -> float:
        """
        Model dirty settlement price (clean plus accrued), as a percentage of
        the tranche's current balance.

        Returns
        -------
        float
            The model dirty price.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def accrued(self) -> float:
        """
        Accrued coupon at buyer settlement, in ``currency`` units.

        Returns
        -------
        float
            The accrued interest.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def factor(self) -> float:
        """
        Pool factor of the note: current balance over original balance, so a
        price on original face is ``clean_price_pct * factor``.

        Returns
        -------
        float
            The pool factor in ``[0, 1]`` for an amortizing note (``1.0`` at
            new issue; above ``1.0`` only after PIK accretion).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def wal(self) -> float:
        """
        Weighted-average life, in years.

        Returns
        -------
        float
            The weighted-average life.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def ytm(self) -> float | None:
        """
        Decimal yield reproducing the ``target_price_pct`` dirty value,
        compounded at the note's coupon frequency with time in the note's own
        day count (a par note yields its coupon).

        Returns
        -------
        float | None
            ``None`` for an impaired note.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def z_spread_bp(self) -> float:
        """
        Z-spread to ``target_price_pct``, in basis points.

        Returns
        -------
        float
            The z-spread in basis points.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def cs01(self) -> float:
        """
        Credit-spread DV01 -- currency change for a +1 bp z-spread shock, in
        ``currency`` units. Negative for a long tranche.

        Returns
        -------
        float
            The credit-spread DV01.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def spread_duration(self) -> float:
        """
        Spread duration, in years (``-cs01 / (pv * 1bp)``).

        Returns
        -------
        float
            The spread duration.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def spread_convexity(self) -> float:
        """
        Spread convexity at the solved z-spread.

        Returns
        -------
        float
            Second-order z-spread sensitivity of the projected cashflows, in
            years squared, on the same kernel as ``spread_duration``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def effective_duration(self) -> float:
        """
        Effective (rate) duration of the tranche.

        Returns
        -------
        float
            Years. The cashflows are re-projected with every rate curve
            (discount and forward) bumped ±1 bp in parallel, so a floater's
            coupon resets move with the curve and its duration is short; a
            fixed-coupon note's equals its modified duration.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def effective_convexity(self) -> float:
        """
        Effective convexity of the tranche.

        Returns
        -------
        float
            Years squared, from the same ±1 bp re-projection as
            ``effective_duration``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def target_price_pct(self) -> float:
        """
        Price the yield, z-spread and CS01 were solved against, as a
        percentage of current balance.

        Returns
        -------
        float
            The target price.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def wal_to_call(self) -> float | None:
        """
        Weighted-average life to the assumed call, in years.

        Returns
        -------
        float | None
            ``None`` when the deal carries no call covering this tranche.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def z_spread_to_call_bp(self) -> float | None:
        """
        Z-spread of the to-call flows to ``target_price_pct``, in basis points.

        Returns
        -------
        float | None
            ``None`` without a call.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def dm_to_call_bp(self) -> float | None:
        """
        Discount margin to call for a floating-rate tranche, in basis points.

        Returns
        -------
        float | None
            ``None`` for fixed-rate tranches or without a call.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def dm_bp(self) -> float | None:
        """
        Discount margin to maturity at ``target_price_pct``.

        Returns
        -------
        float | None
            Basis points of constant spread over the deal discount curve that
            reprices the floater's projected cashflows; ``None`` for
            fixed-rate tranches.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def metrics(self) -> dict[str, float]:
        """
        Requested registry metrics valued on this note.

        Returns
        -------
        dict[str, float]
            Keyed by metric id; empty when none were requested. Every reprice
            values this tranche, so tranche sensitivities sum to the deal's.
            Units follow each metric.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export as a single-row pandas DataFrame.

        Columns: ``tranche_id``, ``currency``, ``pv``, ``clean_price_pct``,
        ``dirty_price_pct``, ``accrued``, ``factor``, ``wal``, ``ytm``,
        ``z_spread_bp``, ``cs01``, ``spread_duration``, ``spread_convexity``,
        ``effective_duration``, ``effective_convexity``, ``target_price_pct``,
        ``wal_to_call``, ``z_spread_to_call_bp``, ``dm_to_call_bp``, ``dm_bp`` --
        the same fields and units as the properties of the same name (``ytm`` is
        ``NaN`` for an impaired note, the ``*_to_call`` columns without a call,
        ``dm_bp`` for fixed-rate tranches). Requested registry metrics stay in
        :attr:`metrics`.

        Returns
        -------
        pd.DataFrame
            Single-row DataFrame, so a capital structure stacks with
            ``pd.concat``. ``pv`` and ``cs01`` are in ``currency`` units and
            are only additive across tranches sharing one currency.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            Reconstruction callable and canonical JSON arguments.

        Raises
        ------
        ValueError
            If serialization of the stored value fails.
        """
        ...

class ScenarioTable:
    """
    Scenario/yield table for a single structured-credit tranche
    (``tranche_scenario_table``'s return value).

    Examples
    --------
    >>> import json
    >>> from finstack_quant.valuations.instruments import ScenarioTable
    >>> payload = json.dumps({
    ...     "tranche_id": "A",
    ...     "cells": [{"cpr": 0.06, "cdr": 0.02, "severity": 0.6, "price": 98.2, "wal": 4.1, "writedown": 0.0}],
    ... })
    >>> table = ScenarioTable.from_json(payload)
    >>> table.tranche_id
    'A'
    """

    @staticmethod
    def from_json(json: str) -> ScenarioTable:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``ScenarioTable``.

        Returns
        -------
        ScenarioTable
            The decoded scenario table.

        Raises
        ------
        ValueError
            If ``json`` is not valid JSON for the ``ScenarioTable`` shape.

        Examples
        --------
        >>> import json
        >>> from finstack_quant.valuations.instruments import ScenarioTable
        >>> payload = {
        ...     "tranche_id": "A",
        ...     "cells": [{"cpr": 0.06, "cdr": 0.02, "severity": 0.6, "price": 98.2, "wal": 4.1, "writedown": 0.0}],
        ... }
        >>> table = ScenarioTable.from_json(json.dumps(payload))
        >>> (table.tranche_id, table.cells[0]["price"])
        ('A', 98.2)

        """
        ...

    def to_json(self) -> str:
        """
        Serialize back to the same JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            JSON-encoded ``ScenarioTable``.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def tranche_id(self) -> str:
        """
        Identifier of the tranche evaluated.

        Returns
        -------
        str
            The tranche identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def cells(self) -> list[dict[str, float]]:
        """
        Evaluated cells, in CPR-major, then CDR, then severity order.

        Each cell is a dict with keys ``cpr`` (annual decimal), ``cdr``
        (annual decimal), ``severity`` (decimal), ``price`` (percentage of
        original balance), ``wal`` (years), and ``writedown`` (currency
        units).

        Returns
        -------
        list[dict[str, float]]
            One dict per evaluated scenario cell.

        Notes
        -----
        This read-only property returns a copy of the stored cells and does not raise.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the evaluated cells as a pandas DataFrame.

        Columns: ``tranche_id``, ``cpr``, ``cdr``, ``severity``, ``price``
        (percentage of current balance), ``wal`` (years), ``writedown``
        (currency units). One row per cell, in CPR-major then CDR then
        severity order -- the same cells and order as ``cells``.

        Returns
        -------
        pd.DataFrame
            One row per scenario cell. A grid that evaluated no cells yields a
            zero-row frame that still carries the columns above.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            Reconstruction callable and canonical JSON arguments.

        Raises
        ------
        ValueError
            If serialization of the stored value fails.
        """
        ...

def tranche_discount_margin(
    instrument: StructuredCredit | str,
    tranche_id: str,
    market: MarketContext | str,
    as_of: datetime.date | str,
    market_price_pct: float,
) -> float:
    """Solve a z-spread-equivalent discount margin for a floating-rate tranche.

    Contractual cashflows are projected without changing coupon projection,
    then a constant additive spread is applied to the discount curve. The
    result is zero at the model price, negative for a richer (higher) price,
    and positive for a cheaper (lower) price; it is not the contractual quoted
    margin.

    Parameters
    ----------
    instrument : StructuredCredit or str
        Tagged JSON for a ``StructuredCredit`` deal, or a typed
        ``StructuredCredit`` instance.
    tranche_id : str
        Identifier of the floating-rate tranche whose contractual cashflows
        are spread-discounted.
    market : MarketContext or str
        Typed ``MarketContext`` or serialized market-context JSON supplying
        the discount curve and any forward curves or historical fixings
        required for cashflow projection.
    as_of : datetime.date | str
        Valuation date used for projection and discounting, either a date-like
        object or an ISO 8601 string.
    market_price_pct : float
        Clean settlement price as a percentage of the tranche's current
        balance (100.0 = par). Accrued interest is added once at the deal's
        ``quote_settlement_date`` (valuation date when omitted).

    Returns
    -------
    float
        Z-spread-equivalent discount margin in decimal (``0.015`` = 150 bp).

    Raises
    ------
    KeyError
        If ``tranche_id`` is not part of the deal.
    ValueError
        If the JSON or date is malformed, the deal fails validation, the
        tranche is missing or fixed-rate, ``market_price_pct`` is not finite and
        positive, required market data is unavailable, or the spread solve
        fails or exceeds ±5000 bp.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import tranche_discount_margin
    >>> try:
    ...     tranche_discount_margin("{}", "A", "{}", "2026-01-01", 100.0)
    ... except ValueError as exc:
    ...     print("schema" in str(exc))
    True

    """
    ...

def tranche_breakeven_cdr(
    instrument: StructuredCredit | str,
    tranche_id: str,
    market: MarketContext | str,
    as_of: datetime.date | str,
) -> float:
    """Solve the constant default rate at which a tranche first takes a writedown.

    Parameters
    ----------
    instrument : StructuredCredit or str
        Tagged JSON for a ``StructuredCredit`` deal, or a typed
        ``StructuredCredit`` instance.
    tranche_id : str
        Identifier of the tranche within the deal.
    market : MarketContext or str
        Typed ``MarketContext`` or serialized market-context JSON supplying
        curves and fixings.
    as_of : datetime.date | str
        Valuation date, either a date-like object or an ISO 8601 string.

    Returns
    -------
    float
        Break-even annual CDR in decimal.

    Raises
    ------
    KeyError
        If ``tranche_id`` is not part of the deal.
    ValueError
        If the instrument JSON is malformed, the deal fails validation, the
        tranche id is not part of the deal, or required market data is missing.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import tranche_breakeven_cdr
    >>> try:
    ...     tranche_breakeven_cdr("{}", "A", "{}", "2026-01-01")
    ... except ValueError as exc:
    ...     print("schema" in str(exc))
    True

    """
    ...

def tranche_oas(
    instrument: StructuredCredit | str,
    tranche_id: str,
    market: MarketContext | str,
    as_of: datetime.date | str,
    market_price_pct: float,
    config: dict[str, Any] | str | None = None,
) -> OasResult:
    """Compute option-adjusted spread for a tranche. Returns an ``OasResult``.

    Parameters
    ----------
    instrument : StructuredCredit or str
        Tagged JSON for a ``StructuredCredit`` deal, or a typed
        ``StructuredCredit`` instance.
    tranche_id : str
        Identifier of the tranche within the deal.
    market : MarketContext or str
        Typed ``MarketContext`` or serialized market-context JSON supplying
        curves and fixings.
    as_of : datetime.date | str
        Valuation date, either a date-like object or an ISO 8601 string.
    market_price_pct : float
        Clean settlement price as a percentage of current balance (100.0 = par).
        Accrued interest is added once at the deal's ``quote_settlement_date``
        (valuation date when omitted); earlier payments belong to the seller.
    config : dict or str or None, optional
        Serialized ``OasConfig``. All fields are required when supplied.

    Returns
    -------
    OasResult
        Typed OAS result. Call :meth:`OasResult.to_json` on it for the wire
        payload.

    Raises
    ------
    KeyError
        If ``tranche_id`` is not part of the deal.
    ValueError
        If the instrument JSON is malformed, the deal fails validation, the
        tranche id is not part of the deal, or required market data is missing.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import tranche_oas
    >>> try:
    ...     tranche_oas("{}", "A", "{}", "2026-01-01", 100.0)
    ... except ValueError as exc:
    ...     print("schema" in str(exc))
    True

    """
    ...

def tranche_metrics(
    instrument: StructuredCredit | str,
    tranche_id: str,
    market: MarketContext | str,
    as_of: datetime.date | str,
    market_price_pct: float | None = None,
    metrics: list[str] | None = None,
) -> TrancheMetrics:
    """Per-tranche valuation, yield, spread and risk metrics. Returns a ``TrancheMetrics``.

    The one per-note analytics entry point: prices, accrued, yield, spreads
    and durations come from the tranche's own to-maturity cashflows, and
    ``metrics`` adds registry metrics valued on the note so tranche
    sensitivities sum to the deal's.

    Parameters
    ----------
    instrument : StructuredCredit or str
        Tagged JSON for a ``StructuredCredit`` deal, or a typed
        ``StructuredCredit`` instance.
    tranche_id : str
        Identifier of the tranche within the deal.
    market : MarketContext or str
        Typed ``MarketContext`` or serialized market-context JSON supplying
        curves and fixings.
    as_of : datetime.date | str
        Valuation date, either a date-like object or an ISO 8601 string.
    market_price_pct : float or None, optional
        Clean settlement price as a percentage of current balance. When omitted,
        the deal's market quote is used, or its model clean settlement price
        if no quote is supplied. PV remains measured at valuation.
    metrics : list[str] or None, optional
        Registry metric ids (for example ``"dv01"``, ``"default01"``) to value
        on this note; returned in :attr:`TrancheMetrics.metrics`. Omitted or
        empty computes none.

    Returns
    -------
    TrancheMetrics
        Typed metrics bundle. Call :meth:`TrancheMetrics.to_json` on it for
        the wire payload.

    Raises
    ------
    KeyError
        If ``tranche_id`` is not part of the deal.
    ValueError
        If the instrument JSON is malformed, the deal fails validation, the
        tranche id is not part of the deal, required market data is missing, or a
        ``metrics`` id is unknown.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import tranche_metrics
    >>> try:
    ...     tranche_metrics("{}", "A", "{}", "2026-01-01")
    ... except ValueError as exc:
    ...     print("schema" in str(exc))
    True

    """
    ...

def tranche_scenario_table(
    instrument: StructuredCredit | str,
    tranche_id: str,
    market: MarketContext | str,
    as_of: datetime.date | str,
    grid: dict[str, Any] | str,
) -> ScenarioTable:
    """Price a tranche across a CPR x CDR x severity grid. Returns a ``ScenarioTable``.

    Parameters
    ----------
    instrument : StructuredCredit or str
        Tagged JSON for a ``StructuredCredit`` deal, or a typed
        ``StructuredCredit`` instance.
    tranche_id : str
        Identifier of the tranche within the deal.
    market : MarketContext or str
        Typed ``MarketContext`` or serialized market-context JSON supplying
        curves and fixings.
    as_of : datetime.date | str
        Valuation date, either a date-like object or an ISO 8601 string.
    grid : dict or str
        Serialized ``ScenarioGrid``. Capped at 10,000 cells because each cell
        reprices the entire deal.

    Returns
    -------
    ScenarioTable
        Typed scenario table. Call :meth:`ScenarioTable.to_json` on it for the
        wire payload.

    Raises
    ------
    KeyError
        If ``tranche_id`` is not part of the deal.
    ValueError
        If the instrument JSON is malformed, the deal fails validation, the
        tranche id is not part of the deal, or required market data is missing.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import tranche_scenario_table
    >>> try:
    ...     tranche_scenario_table("{}", "A", "{}", "2026-01-01", "{}")
    ... except ValueError as exc:
    ...     print("schema" in str(exc))
    True

    """
    ...

"""Stub declarations mirroring ``finstack-quant-py/src/bindings/portfolio/margin.rs``."""

from __future__ import annotations

import datetime
from collections.abc import Sequence
from typing import Any, Literal, TypedDict
import pandas as pd
import polars as pl
from finstack_quant.core.config import FinstackConfig
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.core.table import ArrowTable
from finstack_quant.margin import FrtbSensitivities, ImCollateralResult, ImMethodology, NettingSetId, SimmSensitivities
from finstack_quant.portfolio._container import Portfolio

class NettingSetMargin:
    """
    Margin results for one netting set, in the portfolio base currency.

    Obtained from :attr:`PortfolioMarginResult.by_netting_set`. A netting set
    groups the positions margined under one CSA (bilateral) or one CCP
    (cleared). :attr:`sensitivities` echoes the netted SIMM inputs the initial
    margin was computed from, so the IM can be re-derived with
    :class:`finstack_quant.margin.SimmCalculator`.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import NettingSetMargin
    >>> doc = {
    ...     "netting_set_id": {"kind": "bilateral", "counterparty_id": "BANK_A", "csa_id": "CSA_01"},
    ...     "csa_id": "CSA_01",
    ...     "as_of": "2025-01-15",
    ...     "initial_margin": {"amount": "100", "currency": "USD"},
    ...     "variation_margin": {"amount": "25", "currency": "USD"},
    ...     "total_margin": {"amount": "125", "currency": "USD"},
    ...     "position_count": 1,
    ...     "im_methodology": "schedule",
    ...     "is_approximate": False,
    ...     "sensitivities": None,
    ...     "im_breakdown": {},
    ... }
    >>> margin = NettingSetMargin.from_json(json.dumps(doc))
    >>> (str(margin.netting_set_id), margin.initial_margin.amount, margin.variation_margin.amount)
    ('BANK_A:CSA_01', 100.0, 25.0)
    >>> (margin.total_margin.amount, margin.position_count, margin.is_cleared)
    (125.0, 1, False)
    """

    @staticmethod
    def from_json(json: str) -> NettingSetMargin:
        """
        Deserialize a ``NettingSetMargin`` from its canonical JSON wire form.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json` (strict field names).

        Returns
        -------
        NettingSetMargin
            The reconstructed value.

        Raises
        ------
        ValueError
            If the JSON is malformed, the three amounts are not in one
            currency, or ``total_margin`` does not equal
            ``initial_margin + max(variation_margin, 0)``.

        Examples
        --------
        >>> from finstack_quant.portfolio import NettingSetMargin
        >>> try:
        ...     NettingSetMargin.from_json("{}")
        ... except ValueError:
        ...     print("missing fields")
        missing fields
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this ``NettingSetMargin`` to its canonical JSON wire form.

        Returns
        -------
        str
            Compact JSON that :meth:`from_json` accepts; also backs ``pickle``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def netting_set_id(self) -> NettingSetId:
        """
        Netting set identifier.

        Returns
        -------
        NettingSetId
            Bilateral counterparty/CSA pair or cleared CCP the positions net under.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def csa_id(self) -> str | None:
        """
        Contractual CSA id used for one-time IM terms.

        Returns
        -------
        str or None
            CSA identifier, or ``None`` when the netting set has no OTC CSA.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def as_of(self) -> datetime.date:
        """
        Margin calculation date.

        Returns
        -------
        datetime.date
            Valuation date of the margin run.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def initial_margin(self) -> Money:
        """
        Gross model initial margin before CSA collateral terms.

        Returns
        -------
        Money
            Non-negative amount in the portfolio base currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def variation_margin(self) -> Money:
        """
        Signed desk variation-margin outflow.

        Returns
        -------
        Money
            Base-currency amount; positive means the desk posts, negative means it collects.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_margin(self) -> Money:
        """
        Initial margin plus positive variation margin.

        Returns
        -------
        Money
            ``initial_margin + max(variation_margin, 0)`` in the base currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def position_count(self) -> int:
        """
        Number of positions aggregated into the netting set.

        Returns
        -------
        int
            Count of positions whose mark-to-market entered the netting set.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def im_methodology(self) -> ImMethodology:
        """
        Initial-margin methodology used for the netting set.

        Returns
        -------
        ImMethodology
            Methodology enum; ``str()`` gives the wire label such as ``"simm"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def is_approximate(self) -> bool:
        """
        Whether the IM uses an approximate model.

        Returns
        -------
        bool
            ``True`` for historical SIMM or the CCP proxy, else ``False``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def sensitivities(self) -> SimmSensitivities | None:
        """
        Netted SIMM sensitivities the initial margin was computed from.

        Returns
        -------
        SimmSensitivities or None
            The SIMM input container for this netting set (a copy), or ``None`` when the netting set did not go through SIMM.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def im_breakdown(self) -> dict[str, Money]:
        """
        Initial margin by SIMM risk class.

        Returns
        -------
        dict[str, Money]
            Base-currency amounts keyed by risk-class name, sorted by name; empty when no breakdown was produced.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def is_cleared(self) -> bool:
        """
        Whether the netting set is cleared through a CCP.

        Returns
        -------
        bool
            ``True`` for a cleared netting set, ``False`` for a bilateral one.

        Notes
        -----
        This accessor does not raise; it returns the derived value.
        """
        ...

    def to_arrow(self) -> ArrowTable:
        """
        Initial margin by SIMM risk class as an :class:`~finstack_quant.core.table.ArrowTable`.

        Columns: ``netting_set_id`` (str), ``risk_class`` (str),
        ``initial_margin`` (float64, in ``currency``), ``currency`` (str).

        Returns
        -------
        ArrowTable
            One row per entry of :attr:`im_breakdown`, sorted by risk class;
            zero rows with the same columns when there is no breakdown.

        Raises
        ------
        ValueError
            If the rows cannot be converted into an Arrow table.
        """
        ...

    def to_pandas(self) -> pd.DataFrame:
        """
        Initial margin by SIMM risk class as a pandas ``DataFrame``.

        Returns
        -------
        pd.DataFrame
            One row per entry of :attr:`im_breakdown`, sorted by risk class;
            zero rows with the same columns when there is no breakdown.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """
        Initial margin by SIMM risk class as a Polars ``DataFrame``.

        Returns
        -------
        pl.DataFrame
            One row per entry of :attr:`im_breakdown`, sorted by risk class.

        Raises
        ------
        ImportError
            If ``polars`` is not installed.
        """
        ...

    def to_pyarrow(self) -> Any:
        """
        Initial margin by SIMM risk class as a ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            One row per entry of :attr:`im_breakdown`, sorted by risk class.

        Raises
        ------
        ImportError
            If ``pyarrow`` is not installed.
        """
        ...

    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...
    def __repr__(self) -> str:
        """Return the netting set id, margins and position count."""
        ...

class PortfolioMarginResult:
    """
    Portfolio-wide margin results in the portfolio base currency.

    Returned by :func:`portfolio_margin`. Totals are sums of
    the per-netting-set results in :attr:`by_netting_set`; one-way IM
    collateral accounts are reported per CSA in :attr:`by_csa`.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import Portfolio, portfolio_margin
    >>> book = Portfolio.builder("book", "USD", "2025-01-01").build()
    >>> result = portfolio_margin(book, MarketContext(), "2025-01-01")
    >>> (result.base_currency, result.total_initial_margin.amount, len(result.by_netting_set))
    ('USD', 0.0, 0)
    >>> PortfolioMarginResult.from_json(result.to_json()).total_positions
    0
    """

    @staticmethod
    def from_json(json: str) -> PortfolioMarginResult:
        """
        Deserialize a ``PortfolioMarginResult`` from its canonical JSON wire form.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json` (strict field names).

        Returns
        -------
        PortfolioMarginResult
            The reconstructed value.

        Raises
        ------
        ValueError
            If the JSON is malformed, an aggregate is not in the base
            currency, or the totals do not equal the netting-set and CSA sums.

        Examples
        --------
        >>> from finstack_quant.portfolio import PortfolioMarginResult
        >>> try:
        ...     PortfolioMarginResult.from_json("{}")
        ... except ValueError:
        ...     print("missing fields")
        missing fields
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this ``PortfolioMarginResult`` to its canonical JSON wire form.

        Netting sets are written as a ``netting_sets`` array in ascending
        identifier order.

        Returns
        -------
        str
            Compact JSON that :meth:`from_json` accepts; also backs ``pickle``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def as_of(self) -> datetime.date:
        """
        Margin calculation date.

        Returns
        -------
        datetime.date
            Valuation date of the margin run.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def base_currency(self) -> str:
        """
        Base currency every aggregate is reported in.

        Returns
        -------
        str
            ISO 4217 code of the portfolio base currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_initial_margin(self) -> Money:
        """
        Gross model initial margin across all netting sets, before CSA terms.

        Returns
        -------
        Money
            Base-currency sum of the netting sets' ``initial_margin``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_variation_margin(self) -> Money:
        """
        Signed desk variation-margin outflow across all netting sets.

        Returns
        -------
        Money
            Base-currency sum of the netting sets' ``variation_margin``; collections are negative.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_margin(self) -> Money:
        """
        Gross IM plus positive VM, summed over netting sets.

        Returns
        -------
        Money
            Base-currency sum of the netting sets' ``total_margin``. This is not a collateral transfer.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def by_csa(self) -> dict[str, ImCollateralResult]:
        """
        One-way IM collateral accounts by contractual CSA id.

        Returns
        -------
        dict[str, ImCollateralResult]
            Accounts keyed by CSA id (sorted), each in its own CSA's currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_required_im_collateral(self) -> Money:
        """
        Sum of target IM account balances.

        Returns
        -------
        Money
            Required IM collateral across CSAs, converted to the base currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_im_transfer(self) -> Money:
        """
        Signed IM transfers across CSAs.

        Returns
        -------
        Money
            Base-currency amount; positive means the desk posts.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_segregated_im(self) -> Money:
        """
        Required IM held in segregated custody.

        Returns
        -------
        Money
            Base-currency amount of required IM in segregated accounts.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def by_netting_set(self) -> dict[NettingSetId, NettingSetMargin]:
        """
        Per-netting-set margin results.

        Returns
        -------
        dict[NettingSetId, NettingSetMargin]
            Results keyed by netting set id, in ascending identifier-string order (the ``to_json`` order).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_positions(self) -> int:
        """
        Number of positions whose mark-to-market entered a netting set.

        Returns
        -------
        int
            Sum of the netting sets' ``position_count``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def positions_without_margin(self) -> int:
        """
        Number of distinct positions not registered for margin or degraded.

        Returns
        -------
        int
            Count including every entry of :attr:`degraded_positions`; subtract its length for the unregistered positions.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def degraded_positions(self) -> list[tuple[str, str]]:
        """
        Positions whose sensitivity or VM valuation failed during aggregation.

        Returns
        -------
        list[tuple[str, str]]
            ``(position_id, message)`` pairs in the order the failures were recorded.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_arrow(self) -> ArrowTable:
        """
        Per-netting-set margin as an :class:`~finstack_quant.core.table.ArrowTable`.

        Columns: ``netting_set_id`` (str), ``csa_id`` (str or ``None``),
        ``is_cleared`` (bool), ``initial_margin``, ``variation_margin``,
        ``total_margin`` (float64, in ``currency``), ``currency`` (str, the
        base currency), ``position_count`` (int64), ``im_methodology`` (str
        wire label) and ``is_approximate`` (bool).

        Returns
        -------
        ArrowTable
            One row per netting set in ascending identifier order; the
            ``initial_margin``, ``variation_margin`` and ``total_margin``
            columns sum to the corresponding portfolio totals. Zero rows with
            the same columns when no position is margined.

        Raises
        ------
        ValueError
            If the rows cannot be converted into an Arrow table.

        Examples
        --------
        >>> from finstack_quant.core.market_data import MarketContext
        >>> from finstack_quant.portfolio import Portfolio, portfolio_margin
        >>> book = Portfolio.builder("book", "USD", "2025-01-01").build()
        >>> result = portfolio_margin(book, MarketContext(), "2025-01-01")
        >>> result.to_arrow().column_names()[:4]
        ['netting_set_id', 'csa_id', 'is_cleared', 'initial_margin']
        """
        ...

    def to_pandas(self) -> pd.DataFrame:
        """
        Per-netting-set margin as a pandas ``DataFrame``.

        Returns
        -------
        pd.DataFrame
            One row per netting set in ascending identifier order.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.

        Examples
        --------
        >>> from finstack_quant.core.market_data import MarketContext
        >>> from finstack_quant.portfolio import Portfolio, portfolio_margin
        >>> book = Portfolio.builder("book", "USD", "2025-01-01").build()
        >>> result = portfolio_margin(book, MarketContext(), "2025-01-01")
        >>> list(result.to_pandas().columns)[:4]
        ['netting_set_id', 'csa_id', 'is_cleared', 'initial_margin']
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """
        Per-netting-set margin as a Polars ``DataFrame``.

        Returns
        -------
        pl.DataFrame
            One row per netting set in ascending identifier order.

        Raises
        ------
        ImportError
            If ``polars`` is not installed.
        """
        ...

    def to_pyarrow(self) -> Any:
        """
        Per-netting-set margin as a ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            One row per netting set in ascending identifier order.

        Raises
        ------
        ImportError
            If ``pyarrow`` is not installed.
        """
        ...

    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...
    def __repr__(self) -> str:
        """Return the as-of date, totals and netting-set count."""
        ...

def portfolio_margin(
    portfolio: Portfolio | str,
    market: MarketContext,
    as_of: datetime.date | str,
    current_im_collateral: dict[str, Money] | None = None,
) -> PortfolioMarginResult:
    """
    Calculate portfolio margin requirements by netting set.

    Positions are grouped by the netting set named on their instrument's
    margin specification; initial margin is computed per netting set from
    netted sensitivities and variation margin from netted mark-to-market, both
    reported in the portfolio base currency. Positions without margin metadata
    are counted in ``positions_without_margin``.

    Parameters
    ----------
    portfolio : Portfolio or str
        A :class:`Portfolio` object, or a JSON-serialized ``PortfolioSpec``.
        Its positions that carry margin metadata form the netting sets and its
        base currency is the reporting currency.
    market : MarketContext
        Market data for VM and SIMM sensitivity extraction,
        including the FX matrix needed to report netting sets in the base
        currency.
    as_of : datetime.date or str
        Valuation date of the margin run (date-like or ISO ``YYYY-MM-DD``).
    current_im_collateral : dict[str, Money] or None, default None
        One-way IM balances already held, keyed by CSA id, each in that CSA's
        currency. ``None`` or a missing key means zero. Excludes VM and the
        other party's segregated IM account.

    Returns
    -------
    PortfolioMarginResult
        Per-netting-set and portfolio-level margin in the base currency.

    Raises
    ------
    PortfolioError
        If positions in one netting set carry conflicting margin
        specifications, ``current_im_collateral`` names an unknown CSA id, or a
        required FX rate is unavailable.
    TypeError
        If ``as_of`` is neither a string nor date-like.
    ValueError
        If ``portfolio`` is a string that is not a valid ``PortfolioSpec``, or
        ``as_of`` is a string that is not an ISO date.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import Portfolio, portfolio_margin
    >>> book = Portfolio.builder("book", "USD", "2025-01-01").build()
    >>> result = portfolio_margin(book, MarketContext(), "2025-01-01")
    >>> (result.total_positions, result.positions_without_margin)
    (0, 0)
    """
    ...

class _DiscountGirrSource(TypedDict):
    kind: Literal["discount"]
    curve_id: str

class _ProjectionGirrSource(TypedDict):
    kind: Literal["projection"]
    curve_id: str

class _LinkedOisGirrSource(TypedDict):
    kind: Literal["linked_ois"]
    discount_curve_id: str
    forward_curve_id: str

class _CalibrationQuoteCoordinate(TypedDict):
    quote_id: str
    pillar_date: str

class GirrQuoteTarget:
    """Identify the exact executed quote that supplies one ordinary FRTB GIRR vertex.

    Examples
    --------
    >>> from finstack_quant.portfolio import GirrQuoteTarget
    >>> target = GirrQuoteTarget(
    ...     risk_currency="USD",
    ...     risk_curve_id="USD-OIS",
    ...     vertex="1Y",
    ...     source={"kind": "discount", "curve_id": "USD-OIS"},
    ...     quote={"quote_id": "USD-OIS-SWAP-1Y", "pillar_date": "2026-01-05"},
    ... )
    >>> target.to_dict()["vertex"]
    '1Y'
    """

    def __init__(
        self,
        *,
        risk_currency: str,
        risk_curve_id: str,
        vertex: str,
        source: _DiscountGirrSource | _ProjectionGirrSource | _LinkedOisGirrSource,
        quote: _CalibrationQuoteCoordinate,
    ) -> None:
        """Record the identity and execution coordinate of a quoted yield.

        Parameters
        ----------
        risk_currency : str
            ISO 4217 currency of the yield curve and regulatory risk factor.
        risk_curve_id : str
            Exact risk grouping key, equal to the curve's identifier or to
            the discount identifier of reciprocal OIS views.
        vertex : str
            Case-sensitive Basel vertex such as ``"0.25Y"`` or ``"10Y"``.
        source : dict
            Discount or projection curve identifier, or both reciprocal OIS
            curve identifiers. The chosen source determines which curve
            views are rebuilt and which observations are held.
        quote : dict
            Calibration quote identifier and its original ISO contractual
            pillar date after applying market conventions.

        Raises
        ------
        ValueError
            If the structured coordinates are malformed or the currency is unknown.
        """

    def to_dict(self) -> dict[str, object]:
        """Return the target as a native Python dictionary.

        Returns
        -------
        dict
            The full target with the nested source and executed quote coordinates.

        Raises
        ------
        ValueError
            If the target cannot be represented in its wire shape.
        """

class GirrQuoteDelta:
    """Ordinary FRTB yield-curve delta and the position reprices behind it.

    Examples
    --------
    >>> from finstack_quant.core.config import FinstackConfig
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import Portfolio, girr_quote_delta
    >>> book = Portfolio.builder("book", "USD", "2025-01-02").build()
    >>> result = girr_quote_delta(book, MarketContext(), FinstackConfig(), [])
    >>> result.to_dict()["quote_risk"]
    []
    """

    @property
    def as_of(self) -> datetime.date:
        """Return the valuation date held fixed for every quote reprice.

        Returns
        -------
        datetime.date
            The trading book's valuation date on the held pricing market.

        Notes
        -----
        This accessor does not raise.
        """

    @property
    def sensitivities(self) -> FrtbSensitivities:
        """Return the ordinary GIRR yield-curve sensitivities.

        Returns
        -------
        FrtbSensitivities
            An independent container with signed reporting-currency P&L per
            one percentage point of upward yield shift.

        Notes
        -----
        This accessor does not raise.
        """

    def to_dict(self) -> dict[str, object]:
        """Return the position evidence and source fingerprints as a dictionary.

        Returns
        -------
        dict
            Valuation date, pricing and market fingerprints, the observed
            quote targets, native marks, signed changes and aggregation inputs.

        Raises
        ------
        ValueError
            If the result cannot be represented in its wire shape.
        """

def girr_quote_delta(
    portfolio: Portfolio,
    market: MarketContext,
    pricing: FinstackConfig,
    targets: Sequence[GirrQuoteTarget],
) -> GirrQuoteDelta:
    """Compute ordinary FRTB GIRR delta from upward one-basis-point quote reprices.

    Parameters
    ----------
    portfolio : Portfolio
        Trading-book positions, signed quantities, valuation date and reporting currency.
    market : MarketContext
        Held pricing, FX and accepted executed calibration recipes.
    pricing : FinstackConfig
        Pricing configuration shared by the center and each shocked valuation.
    targets : sequence of GirrQuoteTarget
        Exact quote coordinates for every Basel vertex of each dependent
        yield curve. Empty input is valid for an empty book.

    Returns
    -------
    GirrQuoteDelta
        Signed position reprices and aggregation inputs by risk currency,
        yield curve and exact vertex.

    Raises
    ------
    PortfolioError
        If a required regulatory mapping is missing, unsupported, or ambiguous.
    ValueError
        If an executed calibration quote or its on-market replay is invalid.
    KeyError
        If a required curve or FX observation is unavailable.
    RuntimeError
        If calibration or position valuation fails to compute.

    Examples
    --------
    >>> from finstack_quant.core.config import FinstackConfig
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import Portfolio, girr_quote_delta
    >>> book = Portfolio.builder("book", "GBP", "2025-01-02").build()
    >>> girr_quote_delta(book, MarketContext(), FinstackConfig(), []).as_of.isoformat()
    '2025-01-02'
    """

class CsrQuoteClassification:
    """Sourced issuer identity, seniority, currency and ordinary CSR bucket.

    Examples
    --------
    >>> from finstack_quant.portfolio import CsrQuoteClassification
    >>> label = CsrQuoteClassification(
    ...     issuer="CORP",
    ...     currency="USD",
    ...     seniority="senior",
    ...     bucket=5,
    ...     source_id="issuer-review",
    ...     as_of="2025-01-02",
    ... )
    """

    def __init__(
        self,
        *,
        issuer: str,
        currency: str,
        seniority: Literal["senior_secured", "senior", "subordinated", "junior"],
        bucket: int,
        source_id: str,
        as_of: datetime.date | str,
    ) -> None:
        """Record the authoritative classification to check against the executed curve.

        Parameters
        ----------
        issuer : str
            Exact nonempty issuer ID in the calibration.
        currency : str
            ISO 4217 native currency of the CDS payment and reference quote.
        seniority : str
            Contractual seniority from the curve's admitted issuer classification.
        bucket : int
            Applicable one-based non-securitisation CSR bucket, in 1 through 18.
        source_id : str
            Nonempty identifier of the classification record.
        as_of : date or str
            Effective information cut as a date or ISO year-month-day string.

        Raises
        ------
        ValueError
            If a currency, seniority or date cannot be converted.
        OverflowError
            If the bucket cannot be represented as an unsigned byte.
        """

class CsrQuoteSource:
    """One issuer hazard curve's certified mapping to the five CDS spread vertices.

    Examples
    --------
    >>> from finstack_quant.portfolio import CsrQuoteClassification, CsrQuoteSource
    >>> label = CsrQuoteClassification(
    ...     issuer="CORP",
    ...     currency="USD",
    ...     seniority="senior",
    ...     bucket=5,
    ...     source_id="issuer-review",
    ...     as_of="2025-01-02",
    ... )
    >>> mapping = CsrQuoteSource(curve_id="CORP-HAZARD", classification=label, vertices={})
    >>> mapping.to_dict()["curve_id"]
    'CORP-HAZARD'
    """

    def __init__(
        self,
        *,
        curve_id: str,
        classification: CsrQuoteClassification,
        vertices: dict[str, _CalibrationQuoteCoordinate],
    ) -> None:
        """Author exact quote IDs and contractual dates for a classified curve.

        Parameters
        ----------
        curve_id : str
            Exact hazard curve identifier in the market.
        classification : CsrQuoteClassification
            Dated issuer, currency, seniority and bucket evidence.
        vertices : dict
            The keyed source mapping 0.5Y, 1Y, 3Y, 5Y and 10Y to each executed
            quote_id and ISO pillar_date. The repricer checks completeness and
            matches the coordinates to the executed calibration.

        Raises
        ------
        ValueError
            If a quote coordinate cannot be parsed.
        """

    def to_dict(self) -> dict[str, object]:
        """Export the classification and quote grid as a canonical dictionary.

        Returns
        -------
        dict
            Curve identity, classification evidence and authored quote coordinates.

        Raises
        ------
        ValueError
            If the source cannot be serialized.
        """

class CsrQuoteDelta:
    """Signed issuer CDS quote deltas and their position-level pricing evidence.

    Examples
    --------
    >>> from finstack_quant.core.config import FinstackConfig
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import Portfolio, csr_quote_delta
    >>> book = Portfolio.builder("credit", "USD", "2025-01-02").build()
    >>> report = csr_quote_delta(book, MarketContext(), FinstackConfig(), [])
    >>> report.as_of.isoformat()
    '2025-01-02'
    """

    @property
    def as_of(self) -> datetime.date:
        """Valuation date held through the center and all quote reprices.

        Returns
        -------
        date
            The trading book's valuation date.

        Notes
        -----
        This accessor does not raise.
        """

    @property
    def sensitivities(self) -> FrtbSensitivities:
        """Get the ordinary CSR non-securitisation FRTB inputs.

        Returns
        -------
        FrtbSensitivities
            Signed spread deltas in reporting currency per percentage point.

        Notes
        -----
        This accessor does not raise.
        """

    def to_dict(self) -> dict[str, object]:
        """Export inputs, quote-level reprices, and source hashes.

        Returns
        -------
        dict
            Canonical result with held market and pricing fingerprints.

        Raises
        ------
        ValueError
            If the result cannot be serialized.
        """

def csr_quote_delta(
    portfolio: Portfolio, market: MarketContext, pricing: FinstackConfig, sources: Sequence[CsrQuoteSource]
) -> CsrQuoteDelta:
    """Reprice live single-name CDS at exact non-securitisation CSR spread vertices.

    Parameters
    ----------
    portfolio : Portfolio
        Trading book with signed position quantities, reporting currency and valuation date.
    market : MarketContext
        Held market and executed issuer spread risk recipes.
    pricing : FinstackConfig
        Common pricing and conversion basis for center and shocks.
    sources : sequence of CsrQuoteSource
        Complete five-vertex sources for each live ordinary CDS hazard curve.
        Empty input is valid for an empty book.

    Returns
    -------
    CsrQuoteDelta
        Position changes for a positive one-basis-point bump and signed
        regulatory delta inputs scaled to one percentage point.

    Raises
    ------
    PortfolioError
        If a credit exposure or mapping is unsupported or inconsistent.
    KeyError
        If a required market observation or held FX rate is missing.
    ValueError
        If an executed quote or replay is invalid.
    RuntimeError
        If calibration or repricing fails.

    Examples
    --------
    >>> from finstack_quant.core.config import FinstackConfig
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import Portfolio, csr_quote_delta
    >>> book = Portfolio.builder("credit", "USD", "2025-01-02").build()
    >>> csr_quote_delta(book, MarketContext(), FinstackConfig(), []).as_of.isoformat()
    '2025-01-02'
    """

class PortfolioFrtbConfig:
    """Admitted quote coordinates for both ordinary portfolio delta grids.

    Examples
    --------
    >>> from finstack_quant.portfolio import PortfolioFrtbConfig
    >>> config = PortfolioFrtbConfig(girr_targets=[], csr_sources=[])
    """

    def __init__(self, *, girr_targets: Sequence[GirrQuoteTarget], csr_sources: Sequence[CsrQuoteSource]) -> None:
        """Collect the exact sources for the selected portfolio.

        Parameters
        ----------
        girr_targets : sequence of GirrQuoteTarget
            Complete regulatory yield vertices for each ordinary discount
            or projection curve.
        csr_sources : sequence of CsrQuoteSource
            Dated issuer classifications and spread grids for live
            single-name CDS exposures.

        Raises
        ------
        TypeError
            If the sequences contain objects of the wrong native type.
        """

class _FrtbUnmappedPosition(TypedDict):
    position_id: str
    reason: Literal["unsupported_instrument", "zero_quantity"]

class PortfolioFrtbSensitivitiesResult:
    """Ordinary GIRR and single-name CDS CSR results from one portfolio.

    Examples
    --------
    >>> from finstack_quant.core.config import FinstackConfig
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import Portfolio, PortfolioFrtbConfig, portfolio_frtb_sensitivities
    >>> portfolio = Portfolio.builder("empty-book", "USD", "2025-01-02").build()
    >>> config = PortfolioFrtbConfig(girr_targets=[], csr_sources=[])
    >>> portfolio_frtb_sensitivities(portfolio, MarketContext(), FinstackConfig(), config).positions_mapped
    0
    """

    @property
    def sensitivities(self) -> FrtbSensitivities:
        """Get the admitted ordinary delta amounts.

        Returns
        -------
        FrtbSensitivities
            Yield and issuer spread deltas in the portfolio reporting
            currency per percentage point.

        Notes
        -----
        This accessor does not raise.
        """

    @property
    def girr(self) -> GirrQuoteDelta:
        """Get the yield-curve repricing evidence.

        Returns
        -------
        GirrQuoteDelta
            Exact rate factors and signed position reprices.

        Notes
        -----
        This accessor does not raise.
        """

    @property
    def csr(self) -> CsrQuoteDelta:
        """Get the single-name CDS repricing evidence.

        Returns
        -------
        CsrQuoteDelta
            Sourced spread factors and signed position reprices.

        Notes
        -----
        This accessor does not raise.
        """

    @property
    def positions_mapped(self) -> int:
        """Get the count of active positions admitted to the workflow.

        Returns
        -------
        int
            Number of positions considered for ordinary GIRR or CDS CSR risk.

        Notes
        -----
        This accessor does not raise.
        """

    @property
    def positions_unmapped(self) -> list[_FrtbUnmappedPosition]:
        """Get the positions outside the two-grid workflow.

        Returns
        -------
        list
            Portfolio-order position IDs with typed availability reasons.

        Raises
        ------
        ValueError
            If the coverage evidence cannot be converted to Python.
        """

def portfolio_frtb_sensitivities(
    portfolio: Portfolio, market: MarketContext, pricing: FinstackConfig, config: PortfolioFrtbConfig
) -> PortfolioFrtbSensitivitiesResult:
    """Extract ordinary yield and single-name CDS spread deltas for a trading book.

    Parameters
    ----------
    portfolio : Portfolio
        Trading book with its stored valuation date, reporting currency,
        and signed quantities.
    market : MarketContext
        Held pricing and FX observations with original executed calibrations.
    pricing : FinstackConfig
        Shared pricing and conversion basis for all center and bumped marks.
    config : PortfolioFrtbConfig
        Exact rate bindings and dated issuer classifications and spread grids.

    Returns
    -------
    PortfolioFrtbSensitivitiesResult
        Signed regulatory delta inputs, reprices and availability by position.
        Additional risk families require their own qualified inputs.

    Raises
    ------
    PortfolioError
        If a required factor mapping is incomplete or inconsistent.
    KeyError
        If a required market input or FX rate is missing.
    ValueError
        If a calibration replay cannot use the declared binding.
    RuntimeError
        If an admitted position or calibration fails to reprice.

    Examples
    --------
    >>> from finstack_quant.core.config import FinstackConfig
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import Portfolio, PortfolioFrtbConfig, portfolio_frtb_sensitivities
    >>> empty = Portfolio.builder("empty", "USD", "2025-01-02").build()
    >>> portfolio_frtb_sensitivities(
    ...     empty, MarketContext(), FinstackConfig(), PortfolioFrtbConfig(girr_targets=[], csr_sources=[])
    ... ).positions_unmapped
    []
    """

"""Stub declarations mirroring ``finstack-quant-py/src/bindings/portfolio/margin.rs``."""

from __future__ import annotations

import datetime
import pandas as pd
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.margin import ImCollateralResult, ImMethodology, NettingSetId, SimmSensitivities
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

    def to_dataframe(self) -> pd.DataFrame:
        """
        Initial margin by SIMM risk class as a pandas DataFrame.

        Columns: ``netting_set_id`` (str), ``risk_class`` (str),
        ``initial_margin`` (float64, in ``currency``), ``currency`` (str).

        Returns
        -------
        pd.DataFrame
            One row per entry of :attr:`im_breakdown`, sorted by risk class;
            zero rows with the same columns when there is no breakdown.

        Raises
        ------
        ValueError
            If the rows cannot be converted into a pandas object.
        """
        ...

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

    def to_dataframe(self) -> pd.DataFrame:
        """
        Per-netting-set margin as a pandas DataFrame.

        Columns: ``netting_set_id`` (str), ``csa_id`` (str or ``None``),
        ``is_cleared`` (bool), ``initial_margin``, ``variation_margin``,
        ``total_margin`` (float64, in ``currency``), ``currency`` (str, the
        base currency), ``position_count`` (int64), ``im_methodology`` (str
        wire label) and ``is_approximate`` (bool).

        Returns
        -------
        pd.DataFrame
            One row per netting set in ascending identifier order; the
            ``initial_margin``, ``variation_margin`` and ``total_margin``
            columns sum to the corresponding portfolio totals. Zero rows with
            the same columns when no position is margined.

        Raises
        ------
        ValueError
            If the rows cannot be converted into a pandas object.

        Examples
        --------
        >>> from finstack_quant.core.market_data import MarketContext
        >>> from finstack_quant.portfolio import Portfolio, portfolio_margin
        >>> book = Portfolio.builder("book", "USD", "2025-01-01").build()
        >>> result = portfolio_margin(book, MarketContext(), "2025-01-01")
        >>> list(result.to_dataframe().columns)[:4]
        ['netting_set_id', 'csa_id', 'is_cleared', 'initial_margin']
        """
        ...

    def __repr__(self) -> str:
        """Return the as-of date, totals and netting-set count."""
        ...

def portfolio_margin(
    portfolio: Portfolio | str,
    market: MarketContext | str,
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
    market : MarketContext or str
        Market data (object or JSON) for VM and SIMM sensitivity extraction,
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

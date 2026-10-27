"""Stub declarations mirroring ``finstack-quant-py/src/bindings/portfolio/attribution.rs``."""

from __future__ import annotations

import datetime
from typing import Any
import pandas as pd
from finstack_quant.attribution import PnlAttribution
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.portfolio._container import Portfolio

class ReconciliationReport:
    """
    Reconciliation of an attribution's effect buckets against its total.

    Returned by :meth:`PortfolioAttribution.reconciliation_check` (factor
    buckets plus FX translation against ``total_pnl``, in the portfolio base
    currency) and by :meth:`FiAttributionResult.reconciliation_check` (the five
    Campisi effect totals against ``active_return``, in return units).

    Examples
    --------
    >>> from finstack_quant.portfolio import ReconciliationReport
    >>> doc = '{"total_residual": 0.0, "is_reconciled": true, "tolerance": 0.01}'
    >>> ReconciliationReport.from_json(doc).is_reconciled
    True
    """

    @property
    def total_residual(self) -> float:
        """
        Unexplained amount after all buckets are summed.

        Returns
        -------
        float
            Residual in the portfolio base currency (P&L attribution) or in
            return units (Campisi attribution).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def is_reconciled(self) -> bool:
        """
        Whether ``abs(total_residual) <= tolerance`` and attribution is valid.

        Returns
        -------
        bool
            ``False`` when a P&L attribution was flagged invalid, regardless
            of residual.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def tolerance(self) -> float:
        """
        Absolute tolerance used for the check.

        Returns
        -------
        float
            Tolerance in the units of ``total_residual``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Single-row DataFrame view of the report.

        Columns: ``total_residual``, ``is_reconciled``, ``tolerance``.

        Returns
        -------
        pd.DataFrame
            One row with the pinned column schema.

        Raises
        ------
        ValueError
            If the report cannot be serialized into a pandas object.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to a compact JSON string.

        Returns
        -------
        str
            Compact JSON document.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    @staticmethod
    def from_json(json: str) -> ReconciliationReport:
        """
        Deserialize from JSON produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            JSON text produced by :meth:`to_json`.

        Returns
        -------
        ReconciliationReport
            Reconstructed report.

        Examples
        --------
        >>> from finstack_quant.portfolio import ReconciliationReport
        >>> doc = '{"total_residual": 0.0, "is_reconciled": true, "tolerance": 0.01}'
        >>> ReconciliationReport.from_json(doc).is_reconciled
        True

        Raises
        ------
        ValueError
            If the payload is not valid ``ReconciliationReport`` JSON.
        """
        ...

class PortfolioAttribution:
    """
    Rust-computed portfolio P&L attribution in portfolio base currency.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import Portfolio, attribute_portfolio_pnl
    >>> spec = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
    >>> result = attribute_portfolio_pnl(
    ...     Portfolio.from_spec(spec), MarketContext(), MarketContext(), "2025-01-01", "2025-01-02", "parallel"
    ... )
    >>> result.total_pnl.amount == 0
    True
    """

    def to_json(self) -> str:
        """
        Serialize the complete canonical attribution payload.

        Returns
        -------
        str
            Canonical JSON representation of this `PortfolioAttribution`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> PortfolioAttribution:
        """
        Deserialize a `PortfolioAttribution` from JSON produced by :meth:`to_json`.

        Completes the wire round-trip, which is also what makes this type
        picklable.

        Parameters
        ----------
        json : str
            Canonical JSON produced by :meth:`to_json`.

        Returns
        -------
        PortfolioAttribution
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If ``json`` is malformed or does not match the serialized schema.

        Examples
        --------
        >>> from finstack_quant.portfolio import PortfolioAttribution
        >>> try:
        ...     PortfolioAttribution.from_json("{}")
        ... except ValueError as exc:
        ...     "missing field" in str(exc)
        True
        """
        ...

    def by_position_json(self) -> str:
        """
        Serialize nested attribution in Rust ``IndexMap`` position order.

        Returns
        -------
        str
            Compact position-keyed attribution JSON in canonical insertion order.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def reconciliation_check(self, tolerance: float) -> ReconciliationReport:
        """
        Check that the factor buckets plus FX translation reconcile to ``total_pnl``.

        Parameters
        ----------
        tolerance : float
            Absolute tolerance in base-currency units (``0.01`` for one cent).

        Returns
        -------
        ReconciliationReport
            Typed report with ``total_residual``, ``is_reconciled`` and
            ``tolerance``; ``is_reconciled`` is forced ``False`` when
            ``result_invalid`` is set.

        Notes
        -----
        This method does not raise; it returns the computed report.
        """
        ...

    @property
    def by_position(self) -> dict[str, PnlAttribution]:
        """
        Per-position attributions in each instrument's native currency.

        Returns
        -------
        dict[str, PnlAttribution]
            Typed attributions keyed by position id in canonical order. They
            exclude FX translation, so they do not sum to the base-currency
            aggregates.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_position_dataframe(self) -> pd.DataFrame:
        """
        Export per-position native-currency attributions, one row per position.

        Each row is ``position_id`` followed by the canonical Rust wide row of
        that position's attribution, the same columns as
        :meth:`finstack_quant.attribution.PnlAttribution.to_dataframe`.

        Returns
        -------
        pd.DataFrame
            Columns ``position_id``, ``instrument_id``, ``method``, ``t0``,
            ``t1``, ``currency`` (native), ``total_pnl``,
            ``mark_to_market_pnl``, ``pv_t0``, ``pv_t1``, ``carry``,
            ``rates_curves_pnl``, ``credit_curves_pnl``,
            ``inflation_curves_pnl``, ``correlations_pnl``, ``fx_pnl``,
            ``fx_translation_pnl``, ``vol_pnl``, ``cross_factor_pnl``,
            ``model_params_pnl``, ``market_scalars_pnl``, ``residual``,
            ``residual_pct``, ``num_repricings`` (int64) and
            ``result_invalid`` (bool). Amounts are float64; optional
            amounts are ``NaN`` when the method did not record them.

        Raises
        ------
        ValueError
            If a position attribution mixes currencies across its factors.
        """
        ...

    def explain(self) -> str:
        """
        Human-readable explanation tree of the portfolio-level buckets.

        Returns
        -------
        str
            Multi-line tree listing each bucket and its share of ``total_pnl``.

        Notes
        -----
        This method does not raise; it formats stored values.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the portfolio-level factor totals as a single-row DataFrame.

        Every ``Money`` aggregate is flattened to a float column plus one
        shared ``currency`` column; the per-position nested breakdown stays on
        :meth:`by_position_json`.

        Columns: ``currency``, ``total_pnl``, ``carry``, ``rates_curves_pnl``,
        ``credit_curves_pnl``, ``inflation_curves_pnl``, ``correlations_pnl``,
        ``fx_pnl``, ``fx_translation_pnl``, ``cross_factor_pnl``, ``vol_pnl``,
        ``model_params_pnl``, ``market_scalars_pnl``, ``residual``,
        ``result_invalid``.

        Returns
        -------
        pd.DataFrame
            Exactly one row; concatenate rows across attribution runs to build
            a P&L time series.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    @property
    def total_pnl(self) -> Money:
        """
        Total portfolio P&L between the two market snapshots.

        Returns
        -------
        Money
            Base-currency total; the factor components below decompose it.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def carry(self) -> Money:
        """
        Carry (time decay plus coupon/dividend income) component of total P&L.

        Returns
        -------
        Money
            Base-currency carry. Positive when the passage of time earns income.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def rates_curves_pnl(self) -> Money:
        """
        P&L attributed to interest-rate (discount and forward) curve moves.

        Returns
        -------
        Money
            Base-currency rates component. Excludes credit and inflation curves, which are reported separately.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def credit_curves_pnl(self) -> Money:
        """
        P&L attributed to credit-spread / hazard-rate curve moves.

        Returns
        -------
        Money
            Base-currency credit component, from spread and hazard-rate moves only.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def inflation_curves_pnl(self) -> Money:
        """
        P&L attributed to inflation curve moves.

        Returns
        -------
        Money
            Base-currency inflation component, from index-curve moves only.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def correlations_pnl(self) -> Money:
        """
        P&L attributed to correlation-input moves.

        Returns
        -------
        Money
            Base-currency correlation component, from correlation inputs only.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def fx_pnl(self) -> Money:
        """
        P&L attributed to FX spot/forward moves on FX-sensitive instruments.

        Returns
        -------
        Money
            Distinct from :attr:`fx_translation_pnl`, which covers restating
            unchanged foreign-currency values into the base currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def fx_translation_pnl(self) -> Money:
        """
        P&L from translating non-base-currency values at the new FX rates.

        Returns
        -------
        Money
            Base-currency translation effect only; economic FX exposure is reported in ``fx_pnl``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def cross_factor_pnl(self) -> Money:
        """
        Second-order P&L from joint moves of two or more factors.

        Returns
        -------
        Money
            Base-currency interaction term. Non-zero only when factors move jointly, and not attributable to any single factor.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def vol_pnl(self) -> Money:
        """
        P&L attributed to implied-volatility surface moves.

        Returns
        -------
        Money
            Base-currency volatility component, from implied-vol surface moves only.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def model_params_pnl(self) -> Money:
        """
        P&L attributed to changes in calibrated model parameters.

        Returns
        -------
        Money
            Base-currency model component, from recalibration rather than market moves.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def market_scalars_pnl(self) -> Money:
        """
        P&L attributed to scalar market inputs (spots, dividends, recoveries).

        Returns
        -------
        Money
            Base-currency scalar component, covering inputs that are not curves or surfaces.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def residual(self) -> Money:
        """
        Unexplained P&L left after every factor has been attributed.

        Returns
        -------
        Money
            ``total_pnl`` less the sum of all factor components. Check it
            against a tolerance with :meth:`reconciliation_check`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def result_invalid(self) -> bool:
        """
        Whether Rust flagged the attribution as unusable.

        Returns
        -------
        bool
            ``True`` when at least one position produced non-finite
            sensitivities or failed residual computation. Aggregating an
            invalid result across instruments is not meaningful.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str: ...

def attribute_portfolio_pnl(
    portfolio: Portfolio | str,
    market_t0: MarketContext | str,
    market_t1: MarketContext | str,
    as_of_t0: datetime.date | str,
    as_of_t1: datetime.date | str,
    method: str | dict[str, Any],
    config: dict[str, Any] | str | None = None,
) -> PortfolioAttribution:
    """
    Attribute portfolio P&L with Rust-owned aggregation and FX translation.

    Parameters
    ----------
    portfolio : Portfolio or str
        Built portfolio or canonical ``PortfolioSpec`` JSON being attributed.
    market_t0 : MarketContext or str
        Opening market context object or JSON at the start of the P&L interval.
    market_t1 : MarketContext or str
        Closing market context object or JSON at the end of the P&L interval.
    as_of_t0 : datetime.date | str
        ISO-8601 opening valuation date associated with ``market_t0``.
    as_of_t1 : datetime.date | str
        ISO-8601 closing valuation date associated with ``market_t1``.
    method : str or dict[str, Any]
        Attribution-method name or method configuration understood by Rust.
    config : dict[str, Any] or str or None
        Optional attribution configuration mapping or canonical JSON payload.

    Returns
    -------
    PortfolioAttribution
        Base-currency P&L decomposition by position, market move, and residual.

    Raises
    ------
    TypeError
        If a portfolio, market, method, or configuration input has an unsupported type.
    ValueError
        If supplied JSON, either ISO date, or the attribution method or
        configuration is invalid.
    PortfolioError
        If portfolio construction, valuation, FX conversion, or attribution fails.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import Portfolio, attribute_portfolio_pnl
    >>> spec = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
    >>> result = attribute_portfolio_pnl(
    ...     Portfolio.from_spec(spec), MarketContext(), MarketContext(), "2025-01-01", "2025-01-02", "parallel"
    ... )
    >>> result.total_pnl.amount == 0
    True
    """
    ...

"""Stub declarations mirroring ``finstack-quant-py/src/bindings/portfolio/metrics.rs``."""

from __future__ import annotations

from typing import Any
import pandas as pd
from finstack_quant.core.market_data import MarketContext
from finstack_quant.portfolio._valuation import PortfolioValuation

class PortfolioMetrics:
    """
    Typed wrapper around Rust-aggregated portfolio metrics.

    Examples
    --------
    >>> from finstack_quant.portfolio import PortfolioMetrics
    >>> metrics = PortfolioMetrics.from_json('{"aggregated":{},"by_position":{}}')
    >>> metrics.metric_series("pv")
    []
    """

    @staticmethod
    def from_json(metrics_json: str) -> PortfolioMetrics:
        """
        Deserialize canonical ``PortfolioMetrics`` JSON.

        Parameters
        ----------
        metrics_json : str
            Canonical metric payload returned by ``aggregate_metrics``.

        Returns
        -------
        PortfolioMetrics
            Validated `PortfolioMetrics` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.portfolio import PortfolioMetrics
        >>> metrics = PortfolioMetrics.from_json('{"aggregated":{},"by_position":{}}')
        >>> metrics.metric_series("pv")
        []
        """
        ...

    def to_json(self) -> str:
        """
        Serialize canonical ``PortfolioMetrics`` JSON.

        Returns
        -------
        str
            Canonical JSON representation of this `PortfolioMetrics`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def aggregated(self) -> dict[str, Any]:
        """
        Portfolio-wide aggregated metrics keyed by metric id.

        Returns
        -------
        dict[str, Any]
            Each value carries ``metric_id``, ``total`` and the ``by_entity``
            breakdown, in canonical Rust ``IndexMap`` insertion order. Only
            summable metrics are aggregated.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def by_position(self) -> dict[str, Any]:
        """
        Raw per-position metric values keyed by position id.

        Returns
        -------
        dict[str, Any]
            Each value carries the position's native ``currency`` — non-summable
            metrics are quoted in it — and its ``metrics`` mapping.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def skipped_metrics(self) -> list[Any]:
        """
        Metric values excluded from aggregation because they were non-finite.

        Returns
        -------
        list[Any]
            A non-empty list means aggregation is incomplete; each entry records
            the ``position_id``, ``metric_id`` and the offending ``value``.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def degraded_positions(self) -> list[str]:
        """
        Positions that carried no risk measures because their valuation fell
        back to PV-only.

        Returns
        -------
        list[str]
            Position identifiers contributing zero to every total without a
            ``skipped_metrics`` entry — the only signal that the aggregate is
            partial.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def unaggregated_metrics(self) -> list[str]:
        """
        Metric identifiers present in ``by_position`` that were not aggregated
        into a portfolio total because they are not summable across positions.

        Returns
        -------
        list[str]
            Sorted, de-duplicated metric ids (for example ``ytm`` or
            ``duration``); per-position values remain in ``by_position``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def metric_series(
        self,
        base: str,
    ) -> list[tuple[list[str], float, dict[str, float]]]:
        """
        Return decoded components, total, and entity values in wire order.

        Entity mappings preserve Rust ``IndexMap`` insertion order. Canonical wire keys decode to unique coordinates.

        Parameters
        ----------
        base : str
            Metric namespace prefix used to select matching metric series.

        Returns
        -------
        list[tuple[list[str], float, dict[str, float]]]
            Ordered ``(components, total, by_entity)`` entries below ``base``;
            the scalar base entry is excluded.

        Notes
        -----
        An unknown canonical prefix returns an empty list.

        Raises
        ------
        ValueError
            If the base metric key is not canonically encoded.
        """
        ...

    def get_metric(self, metric_id: str) -> dict[str, Any] | None:
        """
        One aggregated metric.

        Parameters
        ----------
        metric_id : str
            Fully qualified metric key (for example ``"dv01"``).

        Returns
        -------
        dict[str, Any] | None
            ``{"metric_id", "total", "by_entity"}`` or ``None`` when the
            metric was not aggregated.

        Notes
        -----
        This method does not raise; an absent key yields ``None``.
        """
        ...

    def get_position_metrics(self, position_id: str) -> dict[str, Any] | None:
        """
        One position's raw metrics.

        Parameters
        ----------
        position_id : str
            Position identifier.

        Returns
        -------
        dict[str, Any] | None
            ``{"currency", "metrics"}`` or ``None`` when absent.

        Notes
        -----
        This method does not raise; an absent key yields ``None``.
        """
        ...

    def get_total(self, metric_id: str) -> float | None:
        """
        Portfolio total of one aggregated metric.

        Parameters
        ----------
        metric_id : str
            Fully qualified metric key.

        Returns
        -------
        float | None
            Sum across positions, or ``None`` when absent.

        Notes
        -----
        This method does not raise; an absent key yields ``None``.
        """
        ...

    def require_total(self, metric_id: str) -> float:
        """
        Portfolio total of one aggregated metric, raising when it is absent.

        Binds the Rust method ``PortfolioMetrics::require_total``.

        Parameters
        ----------
        metric_id : str
            Fully qualified metric key (for example ``"dv01"`` or
            ``"bucketed_dv01::USD-OIS::10y"``).

        Returns
        -------
        float
            Sum across positions in the portfolio base currency.

        Raises
        ------
        KeyError
            If no aggregate was produced for ``metric_id``.

        Examples
        --------
        >>> from finstack_quant.portfolio import PortfolioMetrics
        >>> metrics = PortfolioMetrics.from_json('{"aggregated":{},"by_position":{}}')
        >>> try:
        ...     metrics.require_total("dv01")
        ... except KeyError:
        ...     print("missing")
        missing
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the portfolio-wide aggregated metrics as a pandas DataFrame.

        One row per entry of the ``aggregated`` map, in canonical Rust
        ``IndexMap`` insertion order. Built from the canonical Rust
        ``aggregated_metrics_to_table`` envelope so the frame cannot drift
        from the Rust table export. The per-entity breakdown is not
        flattened here — reach it through :meth:`metric_series`.

        Returns
        -------
        pd.DataFrame
            One row per aggregated metric; columns ``metric_id``, ``total``
            (sum across positions; only summable metrics are aggregated).

        Raises
        ------
        ValueError
            If the frame cannot be built.
        """
        ...

    def to_position_dataframe(self) -> pd.DataFrame:
        """
        Export the raw per-position metric values in long format.

        One row per ``(position, metric)`` pair — the row count is the total
        number of metric values across positions, not the number of positions.
        Pivot with ``df.pivot(index="position_id", columns="metric_id",
        values="value")`` for a wide view. Built from the canonical Rust
        ``metrics_to_table`` envelope so the frame cannot drift from the Rust
        table export.

        Columns: ``metric_id``, ``position_id``, ``currency`` (the position's
        native currency; non-summable metrics are quoted in it), ``value``.

        Returns
        -------
        pd.DataFrame
            Long-format per-position metric values.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def __repr__(self) -> str: ...

def aggregate_metrics(
    valuation: PortfolioValuation | str,
    market: MarketContext | str,
) -> PortfolioMetrics:
    """
    Aggregate portfolio metrics from a valuation.

    Accepts a typed :class:`PortfolioValuation` (fast path) or a JSON string.
    Metrics are aggregated in the valuation's base currency, with FX spots
    read at the valuation's ``as_of``.

    Parameters
    ----------
    valuation : PortfolioValuation or str
        Typed valuation or canonical valuation JSON to aggregate; its base
        currency and ``as_of`` set the aggregation currency and FX date.
    market : MarketContext or str
        Market context object or JSON supplying the FX spots that convert
        out-of-base metrics.

    Returns
    -------
    PortfolioMetrics
        Typed aggregate-metrics wrapper with ``aggregated`` /
        ``by_position`` getters and DataFrame exits; use
        serialize with ``to_json()`` for the wire string.

    Raises
    ------
    TypeError
        If ``valuation`` or ``market`` is neither its typed wrapper nor a JSON string.
    ValueError
        If supplied JSON is invalid.
    KeyError
        If an FX rate required for base-currency aggregation is unavailable.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import Portfolio, aggregate_metrics, value_portfolio
    >>> spec = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
    >>> valuation = value_portfolio(Portfolio.from_spec(spec), MarketContext())
    >>> aggregate_metrics(valuation, MarketContext()).aggregated
    {}
    """
    ...

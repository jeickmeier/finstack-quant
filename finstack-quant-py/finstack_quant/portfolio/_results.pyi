"""Stub declarations mirroring ``finstack-quant-py/src/bindings/portfolio/results.rs``."""

from __future__ import annotations

import datetime
from typing import Any
from finstack_quant.core.config import FinstackConfig
from finstack_quant.portfolio._metrics import PortfolioMetrics
from finstack_quant.portfolio._valuation import PortfolioValuation

class PortfolioResult:
    """
    Typed wrapper around a ``PortfolioResult`` envelope.

    Read scalars through its components: ``result.valuation.total_value`` and
    ``result.metrics.get_total(metric_id)`` / ``require_total(metric_id)``.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import PortfolioResult
    >>> valuation = {
    ...     "as_of": "2025-01-01",
    ...     "position_values": {},
    ...     "total_base_currency": {"amount": "0", "currency": "USD"},
    ...     "by_entity": {},
    ...     "fx_collapse_policy": "cashflow_date",
    ... }
    >>> rounding = {
    ...     "mode": "bankers",
    ...     "ingest_scale_by_currency": {},
    ...     "output_scale_by_currency": {},
    ...     "tolerances": {"generic_epsilon": 1e-10},
    ...     "version": 1,
    ... }
    >>> doc = {
    ...     "schema_version": 1,
    ...     "valuation": valuation,
    ...     "metrics": {"aggregated": {}, "by_position": {}},
    ...     "meta": {"numeric_mode": "f64", "rounding": rounding},
    ... }
    >>> PortfolioResult.from_json(json.dumps(doc)).valuation.total_value
    0.0
    """

    def __init__(
        self,
        valuation: PortfolioValuation,
        metrics: PortfolioMetrics,
        config: FinstackConfig | None = None,
        timestamp: datetime.datetime | None = None,
    ) -> None:
        """
        Assemble a result envelope from a valuation and its aggregated metrics.

        The ``meta`` stamp is built in Rust: the numeric mode, rounding context
        and library version come from ``config``, and
        ``meta["fx_policy_applied"]`` records the valuation's
        ``fx_collapse_policy`` (e.g. ``"cashflow_date"``).

        Parameters
        ----------
        valuation : PortfolioValuation
            Output of :func:`value_portfolio`.
        metrics : PortfolioMetrics
            Output of :func:`aggregate_metrics` for the same valuation.
        config : FinstackConfig, optional
            Configuration whose rounding context is stamped into ``meta``.
            ``None`` uses the default ``FinstackConfig``.
        timestamp : datetime.datetime, optional
            Timezone-aware audit timestamp for ``meta["timestamp"]``. ``None``
            (the default) omits it, so identical inputs serialize identically.

        Raises
        ------
        TypeError
            If ``timestamp`` is not a timezone-aware ``datetime.datetime``.

        Examples
        --------
        >>> import json
        >>> from finstack_quant.portfolio import PortfolioMetrics, PortfolioResult, PortfolioValuation
        >>> valuation = PortfolioValuation.from_json(
        ...     json.dumps({
        ...         "as_of": "2025-01-01",
        ...         "position_values": {},
        ...         "total_base_currency": {"amount": "0", "currency": "USD"},
        ...         "by_entity": {},
        ...         "fx_collapse_policy": "cashflow_date",
        ...     })
        ... )
        >>> metrics = PortfolioMetrics.from_json('{"aggregated": {}, "by_position": {}}')
        >>> result = PortfolioResult(valuation, metrics)
        >>> (result.valuation.total_value, result.meta["fx_policy_applied"], "timestamp" in result.meta)
        (0.0, 'cashflow_date', False)
        >>> result.to_json() == PortfolioResult(valuation, metrics).to_json()
        True
        >>> import datetime
        >>> stamp = datetime.datetime(2025, 1, 1, 12, tzinfo=datetime.timezone.utc)
        >>> PortfolioResult(valuation, metrics, timestamp=stamp).meta["timestamp"]
        '+002025-01-01T12:00:00.000000000Z'
        """
        ...

    @property
    def valuation(self) -> PortfolioValuation:
        """
        The valuation component.

        Returns
        -------
        PortfolioValuation
            Per-position values and totals.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def metrics(self) -> PortfolioMetrics:
        """
        The aggregated-metrics component.

        Returns
        -------
        PortfolioMetrics
            Portfolio-wide metric totals.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def meta(self) -> dict[str, Any]:
        """
        Calculation metadata stamp.

        Returns
        -------
        dict[str, Any]
            Numeric mode, rounding context and library version.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @staticmethod
    def from_json(result_json: str) -> PortfolioResult:
        """
        Deserialize a portfolio result envelope from JSON.

        Parameters
        ----------
        result_json : str
            Canonical portfolio-result JSON containing total value and metrics.

        Returns
        -------
        PortfolioResult
            Validated `PortfolioResult` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> import json
        >>> from finstack_quant.portfolio import PortfolioResult
        >>> valuation = {
        ...     "as_of": "2025-01-01",
        ...     "position_values": {},
        ...     "total_base_currency": {"amount": "0", "currency": "USD"},
        ...     "by_entity": {},
        ...     "fx_collapse_policy": "cashflow_date",
        ... }
        >>> rounding = {
        ...     "mode": "bankers",
        ...     "ingest_scale_by_currency": {},
        ...     "output_scale_by_currency": {},
        ...     "tolerances": {"generic_epsilon": 1e-10},
        ...     "version": 1,
        ... }
        >>> doc = {
        ...     "schema_version": 1,
        ...     "valuation": valuation,
        ...     "metrics": {"aggregated": {}, "by_position": {}},
        ...     "meta": {"numeric_mode": "f64", "rounding": rounding},
        ... }
        >>> PortfolioResult.from_json(json.dumps(doc)).valuation.total_value
        0.0
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this result envelope to canonical JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PortfolioResult`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

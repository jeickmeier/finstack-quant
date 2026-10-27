"""Stub declarations mirroring ``finstack-quant-py/src/bindings/portfolio/valuation.rs``."""

from __future__ import annotations

import datetime
from typing import Any
import pandas as pd
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.core.table import ArrowTable
from finstack_quant.portfolio._container import Portfolio

class PositionValue:
    """
    Valuation of a single position inside a ``PortfolioValuation``.

    Examples
    --------
    >>> from finstack_quant.portfolio import PositionValue
    >>> doc = '{"position_id":"p1","entity_id":"e1","value_native":{"amount":"1","currency":"USD"},"value_base":{"amount":"1","currency":"USD"},"metric_scale":1.0,"risk_metrics_complete":true}'
    >>> PositionValue.from_json(doc).position_id
    'p1'
    """

    @staticmethod
    def from_json(json: str) -> PositionValue:
        """
        Parse a position value from canonical JSON.

        Parameters
        ----------
        json : str
            JSON text produced by :meth:`to_json`.

        Returns
        -------
        PositionValue
            Reconstructed value.

        Examples
        --------
        >>> from finstack_quant.portfolio import PositionValue
        >>> doc = '{"position_id":"p1","entity_id":"e1","value_native":{"amount":"1","currency":"USD"},"value_base":{"amount":"1","currency":"USD"},"metric_scale":1.0,"risk_metrics_complete":true}'
        >>> PositionValue.from_json(doc).position_id
        'p1'

        Raises
        ------
        ValueError
            If the payload is not valid ``PositionValue`` JSON.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

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

    @property
    def position_id(self) -> str:
        """
        Position identifier.

        Returns
        -------
        str
            Position identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def entity_id(self) -> str:
        """
        Owning entity identifier.

        Returns
        -------
        str
            Owning entity identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def value_native(self) -> Money:
        """
        Value in the instrument's native currency.

        Returns
        -------
        Money
            Native-currency present value.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def value_base(self) -> Money:
        """
        Value converted to the portfolio base currency.

        Returns
        -------
        Money
            Base-currency present value.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def fx_rate(self) -> float | None:
        """
        Spot FX rate applied to convert ``value_native`` into ``value_base``.

        Returns
        -------
        float or None
            Units of the portfolio base currency per one unit of the native
            currency, observed at the valuation date, so
            ``value_native.amount * fx_rate == value_base.amount`` up to the
            base currency's rounding. ``None`` when the native currency is the
            base currency (no FX lookup is made) and on values loaded from
            JSON written before the field existed.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def fx_triangulated(self) -> bool | None:
        """
        Whether ``fx_rate`` was triangulated through the FX pivot currency.

        Returns
        -------
        bool or None
            ``True`` when the rate was built through the FX matrix's pivot
            currency, ``False`` when it came from a direct or inverse quote,
            and ``None`` exactly when ``fx_rate`` is ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def metric_scale(self) -> float:
        """
        Linear scale applied to summable risk metrics.

        Returns
        -------
        float
            Position size and sign used to scale summable metrics.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def risk_metrics_complete(self) -> bool:
        """
        Whether every risk metric requested of this position was computed.

        A portfolio metric list is a menu offered to a heterogeneous book, so
        metrics this position's instrument type has no calculator for are
        never requested of it and do not clear this flag; they are listed on
        :attr:`inapplicable_metrics` instead.

        Returns
        -------
        bool
            ``True`` when no requested metric fell back to PV-only valuation.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def risk_error(self) -> str | None:
        """
        Original metric-failure message when valuation fell back to PV-only.

        Returns
        -------
        str or None
            Failure message, or ``None`` when risk metrics completed.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def inapplicable_metrics(self) -> list[str]:
        """
        Requested metrics this position's instrument type cannot produce.

        :func:`value_portfolio` offers one metric list to a book of mixed
        instrument types, so each position is asked only for the entries it
        has a calculator for. This reports the remainder, making that
        narrowing visible. It is not a failure list: nothing here could have
        been computed and was not, so unlike
        :attr:`PortfolioValuation.degraded_positions` it does not mean a
        portfolio total is understated.

        Returns
        -------
        list[str]
            Metric identifiers, in request order. Empty when the position
            supports every requested metric.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> from finstack_quant.portfolio import PositionValue
        >>> doc = '{"position_id":"p1","entity_id":"e1","value_native":{"amount":"1","currency":"USD"},"value_base":{"amount":"1","currency":"USD"},"metric_scale":1.0,"risk_metrics_complete":true,"inapplicable_metrics":["delta"]}'
        >>> PositionValue.from_json(doc).inapplicable_metrics
        ['delta']
        """
        ...

    @property
    def valuation_result(self) -> dict[str, Any] | None:
        """
        Full instrument valuation result as a JSON-shaped dict, if retained.

        Returns
        -------
        dict[str, Any] or None
            Metrics-inclusive valuation payload, or ``None`` when omitted.

        Raises
        ------
        ValueError
            If the retained result cannot be converted to a Python object.
        """
        ...

    def __repr__(self) -> str:
        """
        Return a concise debug representation.

        Returns
        -------
        str
            ``PositionValue(position_id=..., value_base=..., ...)``.

        Notes
        -----
        This method does not raise.
        """
        ...

class PortfolioValuation:
    """
    Typed wrapper around a ``PortfolioValuation`` result.

    Wrap the JSON returned by :func:`value_portfolio` once and pass the typed
    object to :func:`aggregate_metrics` to skip re-parsing.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import PortfolioValuation
    >>> doc = {
    ...     "as_of": "2025-01-01",
    ...     "position_values": {},
    ...     "total_base_currency": {"amount": "0", "currency": "USD"},
    ...     "by_entity": {},
    ...     "fx_collapse_policy": "cashflow_date",
    ... }
    >>> PortfolioValuation.from_json(json.dumps(doc)).total_value
    0.0
    """

    @staticmethod
    def from_json(valuation_json: str) -> PortfolioValuation:
        """
        Deserialize a ``PortfolioValuation`` from JSON.

        Parameters
        ----------
        valuation_json : str
            Canonical valuation payload returned by ``value_portfolio`` or an
            equivalent serialized portfolio valuation.

        Returns
        -------
        PortfolioValuation
            Validated `PortfolioValuation` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> import json
        >>> from finstack_quant.portfolio import PortfolioValuation
        >>> doc = {
        ...     "as_of": "2025-01-01",
        ...     "position_values": {},
        ...     "total_base_currency": {"amount": "0", "currency": "USD"},
        ...     "by_entity": {},
        ...     "fx_collapse_policy": "cashflow_date",
        ... }
        >>> valuation = PortfolioValuation.from_json(json.dumps(doc))
        >>> (valuation.total_value, valuation.base_currency)
        (0.0, 'USD')
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this valuation to canonical JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PortfolioValuation`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def total_value(self) -> float:
        """
        Total portfolio value in ``base_currency``.

        Returns
        -------
        float
            Total portfolio value in ``base_currency``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def base_currency(self) -> str:
        """
        Base currency code for this valuation.

        Returns
        -------
        str
            Base currency code for this valuation.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def as_of(self) -> datetime.date:
        """
        Pricing date at which every position in this valuation was marked.

        Returns
        -------
        datetime.date
            Calendar date carried through from the valued portfolio; all
            discounting, accruals and FX conversions in the result are as of
            this date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def position_values(self) -> dict[str, PositionValue]:
        """
        Per-position valuations keyed by position id, in valuation order.

        Returns
        -------
        dict[str, PositionValue]
            Typed per-position values.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def by_entity(self) -> dict[str, Money]:
        """
        Base-currency totals by entity id.

        Returns
        -------
        dict[str, Money]
            Entity rollups in valuation order.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def degraded_positions(self) -> list[str]:
        """
        Positions that fell back to PV-only valuation.

        Returns
        -------
        list[str]
            Position ids whose requested risk metrics could not be computed.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def fx_collapse_policy(self) -> str:
        """
        FX conversion policy stamped on the base-currency rollups.

        Returns
        -------
        str
            Serde name of the applied ``FxConversionPolicy`` (for example
            ``"cashflow_date"``).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def get_position_value(self, position_id: str) -> PositionValue:
        """
        Look up one position's valuation.

        Parameters
        ----------
        position_id : str
            Position identifier.

        Returns
        -------
        PositionValue
            Typed valuation for the position.

        Raises
        ------
        KeyError
            If ``position_id`` was not valued.
        """
        ...

    def get_entity_value(self, entity_id: str) -> Money:
        """
        Look up one entity's base-currency total.

        Parameters
        ----------
        entity_id : str
            Entity identifier.

        Returns
        -------
        Money
            Base-currency rollup for the entity.

        Raises
        ------
        KeyError
            If ``entity_id`` has no valued positions.
        """
        ...

    def to_arrow_positions(self) -> ArrowTable:
        """
        Export per-position values via Arrow (zero-copy for consumers).

        Columns: ``position_id``, ``entity_id``, ``value_native``,
        ``value_base``, ``currency_native``, ``currency_base``, ``fx_rate``
        (float64, null for same-currency rows) and ``risk_error`` (string, null
        unless the position degraded to PV-only) (see
        ``finstack_quant_portfolio::dataframe::positions_to_table``).

        Returns
        -------
        ArrowTable
            Arrow table with one row per valued position.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export per-position values as a pandas DataFrame.

        One row per entry of ``position_values``. Built from the same
        ``positions_to_table`` envelope that backs :meth:`to_arrow_positions`,
        plus the two boolean risk diagnostics the Arrow table cannot carry.

        Columns: ``position_id``, ``entity_id``, ``value_native``,
        ``value_base``, ``currency_native``, ``currency_base``, ``fx_rate``
        (float64: base-currency units per native unit applied to the row, so
        ``value_native * fx_rate == value_base`` up to base-currency rounding;
        ``NaN`` when the native currency is the base currency),
        ``fx_triangulated`` (``True``/``False`` for a converted row, else
        ``None``), ``risk_metrics_complete`` (bool) and ``risk_error``
        (``None`` unless the position degraded to PV-only).

        Returns
        -------
        pd.DataFrame
            Values are floats and currency codes are strings. Prefer
            :meth:`to_arrow_positions` when a zero-copy handoff matters more
            than pandas ergonomics.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def __len__(self) -> int:
        """Number of valued positions.
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

def value_portfolio(
    portfolio: Portfolio | str,
    market: MarketContext | str,
    strict_risk: bool | None = None,
    metrics: list[str] | None = None,
) -> PortfolioValuation:
    """
    Value a portfolio and return its typed result.

    Typed ``Portfolio`` and ``MarketContext`` inputs avoid rebuilding runtime
    state, while the result can be passed directly to :func:`aggregate_metrics`.

    Parameters
    ----------
    portfolio : Portfolio or str
        Built portfolio or canonical ``PortfolioSpec`` JSON to value.
    market : MarketContext or str
        Market context object or JSON supplying curves, quotes, and FX data.
    strict_risk : bool or None, default None
        Whether absent or failed risk calculations abort the valuation rather
        than being recorded as diagnostics. ``None`` uses the Rust
        ``PortfolioValuationOptions`` default, ``True``: a standard-metric
        risk run fails closed. Set ``False`` only for an intentional
        PV-preserving fallback.
    metrics : list[str] or None, default None
        Metric identifiers to offer every position. ``None`` requests the
        standard portfolio risk set; an empty list performs PV-only
        valuation. Names resolve exactly as in
        :func:`finstack_quant.valuations.instruments.price_instrument`: every
        id :func:`~finstack_quant.valuations.instruments.list_standard_metrics`
        returns is accepted, and an unknown name raises ``ValueError`` listing
        the closest standard metrics.

        The list is a *menu*, not a per-position request: one list is chosen
        for a book of mixed instrument types, so each position is asked for
        exactly the entries its own instrument can compute (a composite
        position: the additive entries at least one of its legs supports),
        and the rest are reported on
        :attr:`PositionValue.inapplicable_metrics`. Narrowing covers
        structural inapplicability only — a metric an instrument does
        support but fails to compute is still governed by ``strict_risk``.
        Pricing one instrument directly with
        :func:`finstack_quant.valuations.instruments.price_instrument` keeps
        the opposite contract and rejects a metric that instrument cannot
        produce.

    Returns
    -------
    PortfolioValuation
        Typed valuation result backed directly by the Rust calculation.

    Raises
    ------
    ValueError
        If a requested metric name is neither a standard metric identifier
        nor registered in the standard metric registry.
    PortfolioError
        If portfolio construction, market lookup, FX conversion, pricing, or
        strict risk evaluation fails.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import Portfolio, value_portfolio
    >>> spec = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
    >>> value_portfolio(Portfolio.from_spec(spec), MarketContext()).total_value
    0.0
    """
    ...

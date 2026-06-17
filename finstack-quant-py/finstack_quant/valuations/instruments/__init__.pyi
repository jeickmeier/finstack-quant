from __future__ import annotations

from finstack_quant.core.market_data import MarketContext

__all__: list[str]

def validate_instrument_json(json: str) -> str:
    """Validate tagged instrument JSON and return canonical JSON.

    Args:
        json: JSON string for a tagged valuation instrument.

    Returns:
        Canonical pretty-printed instrument JSON after Rust serde validation.

    Raises:
        ValueError: If the JSON is malformed, has an unknown instrument tag, or
            fails instrument-specific validation.
    """
    ...

def price_instrument(
    instrument_json: str,
    market: MarketContext | str,
    as_of: str,
    model: str = "default",
) -> str:
    """Price one instrument and return a ``ValuationResult`` JSON string.

    Args:
        instrument_json: Tagged instrument JSON accepted by
            :func:`validate_instrument_json`.
        market: Typed ``MarketContext`` or serialized market-context JSON.
        as_of: ISO 8601 valuation date.
        model: Pricing model selector. Common values include ``"default"``,
            ``"discounting"``, ``"hazard_rate"``, and option-model keys such
            as ``"black76"`` where supported by the instrument.

    Returns:
        JSON-serialized valuation result containing value, currency, metrics,
        and covenant flags when applicable.

    Raises:
        ValueError: If any input JSON is malformed, required market data is
            missing, or the selected model is unsupported for the instrument.
    """
    ...

def price_instrument_with_metrics(
    instrument_json: str,
    market: MarketContext | str,
    as_of: str,
    model: str = "default",
    metrics: list[str] = [],
    pricing_options: str | None = None,
    market_history: str | None = None,
) -> str:
    """Price one instrument and compute explicit risk metric requests.

    Args:
        instrument_json: Tagged instrument JSON.
        market: Typed ``MarketContext`` or serialized market-context JSON.
        as_of: ISO 8601 valuation date.
        model: Pricing model selector.
        metrics: Metric IDs to compute, such as ``"ytm"``, ``"dv01"``,
            ``"modified_duration"``, ``"hvar"``, or ``"expected_shortfall"``
            when supported by the instrument.
        pricing_options: Optional JSON string for metric pricing overrides.
        market_history: Optional JSON market-history payload required by
            historical risk metrics.

    Returns:
        JSON-serialized valuation result including requested metric values.

    Raises:
        ValueError: If a metric is unknown, not applicable, or cannot be
            calculated from the supplied market and history inputs.
    """
    ...

def list_standard_metrics() -> list[str]:
    """Return all standard metric IDs registered by the Rust valuation engine."""
    ...

def list_standard_metrics_grouped() -> dict[str, list[str]]:
    """Return standard metric IDs grouped by human-readable category."""
    ...

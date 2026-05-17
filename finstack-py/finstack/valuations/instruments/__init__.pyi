from __future__ import annotations

from finstack.core.market_data import MarketContext

__all__: list[str]

def validate_instrument_json(json: str) -> str: ...
def price_instrument(
    instrument_json: str,
    market: MarketContext | str,
    as_of: str,
    model: str = "default",
) -> str: ...
def price_instrument_with_metrics(
    instrument_json: str,
    market: MarketContext | str,
    as_of: str,
    model: str = "default",
    metrics: list[str] = [],
    pricing_options: str | None = None,
    market_history: str | None = None,
) -> str: ...
def list_standard_metrics() -> list[str]: ...
def list_standard_metrics_grouped() -> dict[str, list[str]]: ...

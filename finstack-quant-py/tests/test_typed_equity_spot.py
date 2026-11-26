from datetime import date

import pytest

from finstack_quant.core.market_data import MarketContext
from finstack_quant.valuations.instruments import Equity, price_instrument


def test_equity_builder_prices_short_and_preserves_quote_choice() -> None:
    market = MarketContext()
    as_of = date(2025, 1, 2)
    asset = (
        Equity
        .builder()
        .id("EQ-POSITION")
        .ticker("UNDERLYING")
        .currency("USD")
        .quantity(-4.0)
        .quoted_spot(12.125)
        .spot_id("EXPLICIT-QUOTE")
        .discrete_dividends([(date(2025, 2, 3), 0.38)])
        .discount_curve_id("USD-OIS")
        .build()
    )
    assert price_instrument(asset, market, as_of).price == -48.5
    assert price_instrument(asset.to_json(), market, as_of).value == price_instrument(asset, market, as_of).value
    assert asset.discrete_dividends == [(date(2025, 2, 3), 0.38)]
    edited = asset.to_builder().quoted_spot(None).build()
    assert edited.spot_id == "EXPLICIT-QUOTE"
    assert asset.quoted_spot == 12.125
    with pytest.raises(KeyError, match="EXPLICIT-QUOTE"):
        price_instrument(edited, market, as_of)
    with pytest.raises(ValueError, match="quantity"):
        asset.to_builder().quantity(float("inf")).build()

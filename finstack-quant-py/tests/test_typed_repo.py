from datetime import date
from decimal import Decimal
import json

import pytest

from finstack_quant.core.market_data import DiscountCurve, MarketContext
from finstack_quant.valuations.instruments import Repo, price_instrument


def test_repo_exact_rate_survives_builder_copy_and_wire_pricing() -> None:
    source = Repo.example()
    rate = Decimal("0.052501234567890123")
    trade = source.to_builder().repo_rate(rate).build()
    assert trade.repo_rate == rate
    assert source.repo_rate != rate
    as_of = trade.start_date
    assert isinstance(as_of, date)
    curve = DiscountCurve(
        trade.discount_curve_id,
        as_of,
        [(0.0, 1.0), (1.0, 0.95)],
        interp="log_linear",
        extrapolation="extend",
        day_count="act_365f",
    )
    market = MarketContext().insert(curve)
    wire_value = price_instrument(json.loads(trade.to_json()), market, as_of).value
    assert price_instrument(trade, market, as_of).value == wire_value
    with pytest.raises(ValueError, match="maturity"):
        trade.to_builder().maturity(date(1990, 1, 1)).build()

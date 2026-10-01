"""Credit-index references stay canonical across Python market mutations."""

import math

import pytest

from finstack_quant.core.market_data import (
    BaseCorrelationCurve,
    CreditIndexData,
    DiscountCurve,
    HazardCurve,
    MarketContext,
)


def test_credit_index_resolves_canonical_curves_before_snapshot() -> None:
    canonical = HazardCurve.flat("HAZ", "2025-01-01", 0.02, 0.4)
    stale = HazardCurve.flat("HAZ", "2025-01-01", 0.10, 0.4)
    correlation = BaseCorrelationCurve("CORR", [(3.0, 0.2), (100.0, 0.3)])
    market = MarketContext().insert(canonical).insert(correlation)
    assert market.insert_credit_index("INDEX", CreditIndexData(125, 0.4, stale, correlation)) is market
    restored = MarketContext.from_json(market.to_json())
    market.insert(DiscountCurve("UNRELATED", "2025-01-01", [(0.0, 1.0), (5.0, 0.9)]))
    for context in (market, restored):
        assert context.get_credit_index("INDEX").index_credit_curve.sp(5.0) == pytest.approx(math.exp(-0.1))


def test_credit_index_missing_or_wrong_type_reference_is_atomic() -> None:
    hazard = HazardCurve.flat("HAZ", "2025-01-01", 0.02, 0.4)
    correlation = BaseCorrelationCurve("CORR", [(3.0, 0.2), (100.0, 0.3)])
    index = CreditIndexData(125, 0.4, hazard, correlation)
    market = MarketContext().insert(correlation)
    before = market.to_json()
    with pytest.raises(KeyError, match="HAZ"):
        market.insert_credit_index("INDEX", index)
    assert market.to_json() == before

    market.insert(DiscountCurve("HAZ", "2025-01-01", [(0.0, 1.0), (5.0, 0.9)]))
    before = market.to_json()
    with pytest.raises(ValueError, match="HAZ"):
        market.insert_credit_index("INDEX", index)
    assert market.to_json() == before

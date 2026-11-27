from datetime import date
import json

import pytest

from finstack_quant.core.market_data import DiscountCurve, FxMatrix, MarketContext
from finstack_quant.core.money import Money
from finstack_quant.valuations.instruments import (
    FxBarrierOption,
    FxDigitalOption,
    FxSwap,
    FxTouchOption,
    Ndf,
    QuantoOption,
    price_instrument,
)


@pytest.mark.parametrize(
    "instrument_type", [FxSwap, Ndf, QuantoOption, FxDigitalOption, FxBarrierOption, FxTouchOption]
)
def test_typed_fx_builder_preserves_all_contract_terms(
    instrument_type: type[FxSwap | Ndf | QuantoOption | FxDigitalOption | FxBarrierOption | FxTouchOption],
) -> None:
    instrument = instrument_type.example()
    original = instrument.to_dict()
    edited = instrument.to_builder().id("EDITED-FX").build()
    assert edited.id == "EDITED-FX"
    assert instrument.to_dict() == original
    assert edited.to_dict() == {**original, "id": "EDITED-FX"}


def test_fx_swap_reverses_base_cash_at_explicit_settlement_dates() -> None:
    as_of, near, far = date(2025, 1, 2), date(2025, 2, 3), date(2025, 5, 2)
    usd = DiscountCurve(
        "USD-OIS", as_of, [(0.0, 1.0), (1.0, 0.96)], interp="log_linear", extrapolation="extend", day_count="act_365f"
    )
    eur = DiscountCurve(
        "EUR-OIS", as_of, [(0.0, 1.0), (1.0, 0.97)], interp="log_linear", extrapolation="extend", day_count="act_365f"
    )
    fx = FxMatrix(None)
    fx.set_quotes([("EUR", "USD", 1.10)])
    market = MarketContext().insert(usd).insert(eur).insert_fx(fx)
    instrument = (
        FxSwap
        .builder()
        .id("SWAP")
        .base_currency("EUR")
        .quote_currency("USD")
        .near_date(near)
        .far_date(far)
        .near_rate(1.11)
        .far_rate(1.13)
        .notional(Money(1_000_000, "EUR"))
        .domestic_discount_curve_id("USD-OIS")
        .foreign_discount_curve_id("EUR-OIS")
        .build()
    )
    assert (
        price_instrument(instrument, market, as_of).value
        == price_instrument(json.loads(instrument.to_json()), market, as_of).value
    )
    assert instrument.near_date == near
    assert instrument.far_date == far
    assert instrument.far_rate - instrument.near_rate == pytest.approx(0.02)
    with pytest.raises(ValueError, match="far"):
        instrument.to_builder().far_date(as_of).build()

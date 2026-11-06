"""The bond workout ladder against the ``ytw`` metric, through the Python surface."""

from __future__ import annotations

import datetime
import json

import pytest

from finstack_quant.core.market_data import DiscountCurve, MarketContext
from finstack_quant.valuations.instruments import Bond, BondWorkoutLadder, bond_workout_ladder, price_instrument

AS_OF = datetime.date(2025, 1, 15)
MARKET = MarketContext().insert(DiscountCurve.flat("USD-OIS", AS_OF, 0.04))


def _callable(clean_price_pct: float | None) -> Bond:
    spec = json.loads(Bond.example_callable().to_json())
    if clean_price_pct is not None:
        spec["instrument"]["spec"]["instrument_pricing_overrides"] = {
            "market_quotes": {"quoted_clean_price_pct": clean_price_pct}
        }
    return Bond.from_json(json.dumps(spec))


@pytest.mark.parametrize(("clean", "worst_date"), [(108.0, "2027-01-15"), (92.0, "2034-01-16")])
def test_worst_row_is_the_ytw_metric(clean: float, worst_date: str) -> None:
    bond = _callable(clean)
    ladder = bond_workout_ladder(bond, MARKET, AS_OF)
    worst = [row for row in ladder.rows if row.is_worst]
    assert [row.date.isoformat() for row in worst] == [worst_date]
    result = price_instrument(bond, MARKET, AS_OF, metrics=["ytw", "i_spread"])
    assert worst[0].yield_rate == result.get_metric("ytw")
    assert worst[0].i_spread == result.get_metric("i_spread")


def test_rows_and_frame_carry_each_exit() -> None:
    ladder = bond_workout_ladder(_callable(108.0), MARKET, AS_OF)
    assert [(row.kind, row.redemption_price_pct) for row in ladder.rows] == [
        ("call", 103.0),
        ("call", 101.0),
        ("call", 100.0),
        ("maturity", 100.0),
    ]
    assert ladder.clean_price_pct == 108.0
    frame = ladder.to_dataframe()
    assert list(frame["kind"]) == ["call", "call", "call", "maturity"]
    assert frame["is_worst"].sum() == 1


def test_missing_quote_is_a_key_error_as_the_metric_needs_one() -> None:
    with pytest.raises(KeyError, match="quoted_clean_price_pct"):
        bond_workout_ladder(_callable(None), MARKET, AS_OF)


def test_ladder_round_trips() -> None:
    ladder = bond_workout_ladder(_callable(108.0), MARKET, AS_OF)
    assert BondWorkoutLadder.from_json(ladder.to_json()).to_dict() == ladder.to_dict()

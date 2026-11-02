"""The convertible spot ladder against the metrics of its fields, through the Python surface."""

from __future__ import annotations

import datetime
import json

import pytest

from finstack_quant.core.market_data import DiscountCurve, HazardCurve, MarketContext
from finstack_quant.valuations.instruments import (
    ConvertibleBond,
    ConvertibleSpotLadder,
    convertible_spot_ladder,
    price_instrument,
)

AS_OF = datetime.date(2025, 1, 15)


def _market(spot: float) -> MarketContext:
    return (
        MarketContext()
        .insert(DiscountCurve.flat("USD-IG", AS_OF, 0.04))
        .insert(HazardCurve("USD-CREDIT-BBB", AS_OF, [(1.0, 0.025)], recovery_rate=0.0))
        .insert_price("TECH", spot, "USD")
        .insert_price("TECH-VOL", 0.30)
    )


def _bond() -> ConvertibleBond:
    spec = json.loads(ConvertibleBond.example().to_json())
    spec["instrument"]["spec"]["conversion"]["ratio"] = 10_000.0
    return ConvertibleBond.from_json(json.dumps(spec))


@pytest.mark.parametrize("spot", [50.0, 100.0, 300.0])
def test_each_row_is_the_metrics_at_that_share_price(spot: float) -> None:
    bond = _bond()
    row = convertible_spot_ladder(bond, _market(100.0), AS_OF, [spot]).rows[0]
    result = price_instrument(
        bond,
        _market(spot),
        AS_OF,
        metrics=["delta", "parity", "conversion_value", "conversion_premium", "bond_floor"],
    )
    assert row.price == result.value.amount
    assert row.delta == result.get_metric("delta")
    assert row.parity == result.get_metric("parity")
    assert row.conversion_value == result.get_metric("conversion_value")
    assert row.conversion_premium == result.get_metric("conversion_premium")
    assert row.bond_floor == result.get_metric("bond_floor")
    assert row.hedge_ratio == row.delta / 10_000.0


def test_default_grid_runs_through_every_regime_and_marks_the_market() -> None:
    ladder = convertible_spot_ladder(_bond(), _market(100.0), AS_OF)
    assert ladder.market_spot == 100.0
    assert [row.spot for row in ladder.rows] == [25.0, 50.0, 75.0, 100.0, 125.0, 150.0, 200.0, 300.0]
    assert [row.spot for row in ladder.rows if row.is_market_spot] == [100.0]
    assert {row.regime for row in ladder.rows} == {"busted", "balanced", "equity_sensitive"}
    frame = ladder.to_dataframe()
    assert list(frame["spot"]) == [row.spot for row in ladder.rows]
    assert list(frame["regime"]) == [row.regime for row in ladder.rows]


@pytest.mark.parametrize("spots", [[], [0.0], [-1.0], [float("nan")], [float("inf")]])
def test_spots_the_tree_cannot_value_are_a_value_error(spots: list[float]) -> None:
    with pytest.raises(ValueError, match="share price"):
        convertible_spot_ladder(_bond(), _market(100.0), AS_OF, spots)


def test_missing_share_price_is_a_key_error() -> None:
    market = MarketContext().insert(DiscountCurve.flat("USD-IG", AS_OF, 0.04))
    with pytest.raises(KeyError, match="TECH"):
        convertible_spot_ladder(_bond(), market, AS_OF, [100.0])


def test_ladder_round_trips() -> None:
    ladder = convertible_spot_ladder(_bond(), _market(100.0), AS_OF, [100.0])
    assert ConvertibleSpotLadder.from_json(ladder.to_json()).to_dict() == ladder.to_dict()

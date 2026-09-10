"""Raw fixing windows and recovery-scaled spread shocks through public APIs."""

import json
from pathlib import Path

import pytest

from finstack_quant.cashflows.builder import CashFlowMeta, CashFlowSchedule, Notional
from finstack_quant.cashflows.fixings import ProjectedFixing, materialize_fixings
from finstack_quant.core.dates import DayCount, Tenor
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.scenarios import apply_scenario_to_market
from finstack_quant.valuations.instruments import Bond

FIXTURES = Path(__file__).resolve().parents[2] / "finstack-quant/valuations/tests/fixtures"


def test_materialized_fixings_roundtrip_and_failure_atomicity() -> None:
    market = MarketContext()
    observation = ProjectedFixing("FIXING:USD-SOFR", "2025-01-03", 0.03)
    metadata = CashFlowMeta(projected_fixings=[observation])
    assert metadata.projected_fixings[0].value == 0.03
    assert CashFlowMeta.from_json(metadata.to_json()).projected_fixings[0].value == 0.03
    schedule = CashFlowSchedule.from_flows([], Notional.par(1.0, "USD"), DayCount.ACT_360, metadata)
    rolled = materialize_fixings(market, [schedule], "2025-01-02", "2025-01-06")
    repeated = materialize_fixings(rolled, [schedule], "2025-01-02", "2025-01-06")
    data = json.loads(repeated.to_json())
    assert data["series"][0]["observations"] == [["2025-01-03", 0.03]]
    assert json.loads(market.to_json())["series"] == []
    missing = CashFlowSchedule.from_flows(
        [],
        Notional.par(1.0, "USD"),
        DayCount.ACT_360,
        CashFlowMeta(projected_fixings=[ProjectedFixing("FIXING:USD-SOFR", "2025-01-03")]),
    )
    with pytest.raises(ValueError, match="missing pre-roll projection"):
        materialize_fixings(market, [missing], "2025-01-02", "2025-01-03")
    assert json.loads(market.to_json())["series"] == []
    # The existing observation has priority over an unavailable projection.
    retained = materialize_fixings(rolled, [missing], "2025-01-02", "2025-01-03")
    assert json.loads(retained.to_json())["series"] == data["series"]


def test_first_order_spread_shock_uses_loss_given_default() -> None:
    fixture = json.loads((FIXTURES / "production_cds_option.json").read_text())
    hazard = next(curve for curve in fixture["market"]["curves"] if curve["type"] == "hazard")
    scenario = {
        "id": "units",
        "hazard_bump_mode": "first_order_shift",
        "operations": [{"kind": "curve_parallel_bp", "curve_kind": "par_cds", "curve_id": hazard["id"], "bp": 10}],
    }
    result = apply_scenario_to_market(json.dumps(scenario), json.dumps(fixture["market"]), fixture["as_of"])
    actual = next(curve for curve in json.loads(result.market.to_json())["curves"] if curve["id"] == hazard["id"])
    expected = hazard["knot_points"][0][1] + 0.001 / (1 - hazard["recovery_rate"])
    assert actual["knot_points"][0][1] == pytest.approx(expected, abs=1e-12)
    assert any("first_order" in warning["kind"] for warning in result.report.warnings)


def test_frn_time_roll_preserves_raw_index_excluding_coupon_spread() -> None:
    fixture = json.loads((FIXTURES / "production_quanto_range.json").read_text())
    market = fixture["market"]
    market["curves"] = [curve for curve in market["curves"] if curve["id"] == "USD-OIS"]
    market["curves"][0]["knot_points"] = [[0, 1], [1, 1], [10, 1]]
    market["curves"].append({
        "type": "forward",
        "id": "USD-SOFR-3M",
        "base": fixture["as_of"],
        "reset_lag": 0,
        "day_count": "act_360",
        "tenor": 0.25,
        "knot_points": [[0, 0.03], [1, 0.03], [2, 0.03]],
        "projection_grid": None,
        "interp_style": "linear",
        "extrapolation": "flat_forward",
        "rate_calibration": None,
        "fx_policy": None,
    })
    bond = Bond.floating(
        "ROLL-FRN",
        Money(1_000_000, "USD"),
        "USD-SOFR-3M",
        150,
        "2025-01-03",
        "2026-01-03",
        Tenor.quarterly(),
        DayCount.ACT_360,
        "USD-OIS",
    )
    instrument = json.loads(bond.to_json())
    instrument["instrument"]["spec"]["cashflow_spec"]["floating"]["rate_spec"]["reset_lag_days"] = 0
    scenario = {
        "id": "roll",
        "operations": [
            {"kind": "time_roll_forward", "period": "4D", "apply_shocks": False, "roll_mode": "calendar_days"}
        ],
    }
    result = apply_scenario_to_market(
        json.dumps(scenario), json.dumps(market), fixture["as_of"], [json.dumps(instrument)]
    )
    assert result.report.time_roll["failed_instruments"] == []
    series = next(
        series for series in json.loads(result.market.to_json())["series"] if series["id"] == "FIXING:USD-SOFR-3M"
    )
    assert series["observations"] == [["2025-01-03", pytest.approx(0.03, abs=1e-12)]]
    scenario["operations"][0]["period"] = "1D"
    repeated = apply_scenario_to_market(
        json.dumps(scenario), result.market, result.report.time_roll["new_date"], result.instruments
    )
    assert repeated.report.time_roll["failed_instruments"] == []
    assert json.loads(repeated.market.to_json())["series"] == json.loads(result.market.to_json())["series"]

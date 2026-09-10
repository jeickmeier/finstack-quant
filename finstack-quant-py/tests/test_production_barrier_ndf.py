"""Reciprocal NDF and total-trade rebate contracts through Python pricing."""

from __future__ import annotations

import json
import math
from pathlib import Path
from typing import Any

import pytest

from finstack_quant.valuations.instruments import price_instrument


def fixture(kind: str) -> tuple[dict[str, Any], dict[str, Any]]:
    root = Path(__file__).resolve().parents[2] / "finstack-quant/valuations/tests"
    instrument = json.loads((root / "instruments/json_examples" / f"{kind}.json").read_text())
    market = json.loads((root / "fixtures/production_convertible.json").read_text())["market"]
    market["curves"][0]["base"] = "2024-01-02"
    market["prices"]["SPX-SPOT"] = {"unitless": 100.0}
    market["surfaces"] = json.loads((root / "fixtures/production_equity.json").read_text())["market"]["surfaces"]
    market["surfaces"][0]["id"] = "SPX-VOL"
    return instrument, market


@pytest.mark.parametrize("reciprocal", [False, True])
def test_ndf_quote_orientation_preserves_long_base_payoff(reciprocal: bool) -> None:
    instrument, market = fixture("ndf")
    spec = instrument["instrument"]["spec"]
    spec["notional"]["amount"] = "7000000"
    spec["contract_rate"] = 1.0 / 7.0 if reciprocal else 7.0
    spec["fixing_rate"] = 1.0 / 8.0 if reciprocal else 8.0
    spec["quote_convention"] = "settlement_per_base" if reciprocal else "base_per_settlement"
    result = price_instrument(json.dumps(instrument), json.dumps(market), "2024-01-02", "discounting", [])
    assert result.value.amount == pytest.approx(-125_000.0, abs=1e-8)


@pytest.mark.parametrize("expired", [False, True])
def test_total_trade_rebate_and_paid_at_hit_state(expired: bool) -> None:
    instrument, market = fixture("barrier_option")
    spec = instrument["instrument"]["spec"]
    spec.update(div_yield_id=None, strike=100.0, observed_barrier_breached=True, monitoring={"type": "continuous"})
    spec["barrier"]["amount"] = "120"
    spec["notional"]["amount"] = "1000"
    spec["rebate"]["amount"] = "25"
    spec["rebate_timing"] = "at_hit" if expired else "at_expiry"
    spec["expiry_fixing"] = {"amount": "100", "currency": "USD"}
    result = price_instrument(
        json.dumps(instrument),
        json.dumps(market),
        spec["expiry"] if expired else "2024-01-02",
        "barrier_bs_continuous",
        [],
    )
    assert result.value.amount == pytest.approx(0.0 if expired else 25.0, abs=1e-10)


def quanto_fixture() -> dict[str, Any]:
    root = Path(__file__).resolve().parents[2] / "finstack-quant/valuations/tests/fixtures"
    return json.loads((root / "production_quanto_range.json").read_text())


def test_quanto_asset_financing_and_required_market_inputs() -> None:
    f = quanto_fixture()
    log_mean = 0.10 - 0.5 * 0.20 * 0.10 - 0.5 * 0.20**2

    def cdf(z: float) -> float:
        return 0.5 * (1.0 + math.erf(z / math.sqrt(2.0)))

    expected = 8000.0 * (cdf((math.log(1.2) - log_mean) / 0.2) - cdf((math.log(0.8) - log_mean) / 0.2))
    result = price_instrument(
        json.dumps(f["instrument"]), json.dumps(f["market"]), f["as_of"], "static_replication", []
    )
    assert result.value.amount == pytest.approx(expected, abs=0.02)
    f["instrument"]["instrument"]["spec"]["instrument_pricing_overrides"] = {
        "market_quotes": {"implied_volatility": 0.4}
    }
    expected_override = 8000.0 * (cdf(math.log(1.2) / 0.4) - cdf(math.log(0.8) / 0.4))
    overridden = price_instrument(
        json.dumps(f["instrument"]), json.dumps(f["market"]), f["as_of"], "static_replication", []
    )
    assert overridden.value.amount == pytest.approx(expected_override, abs=0.02)
    del f["market"]["prices"]["EURUSD"]
    with pytest.raises(RuntimeError, match="EURUSD"):
        price_instrument(json.dumps(f["instrument"]), json.dumps(f["market"]), f["as_of"], "static_replication", [])


def test_quanto_rejects_asset_spot_currency_mismatch() -> None:
    f = quanto_fixture()
    f["market"]["prices"]["SPX-SPOT"]["price"]["currency"] = "USD"
    with pytest.raises(RuntimeError, match=r"(?i)currency"):
        price_instrument(json.dumps(f["instrument"]), json.dumps(f["market"]), f["as_of"], "static_replication", [])


def test_discrete_barrier_contract_cannot_use_continuous_analytical_engine() -> None:
    instrument, market = fixture("barrier_option")
    spec = instrument["instrument"]["spec"]
    spec["div_yield_id"] = None
    spec["monitoring"] = {"type": "discrete", "observation_dates": [spec["expiry"]]}
    with pytest.raises(RuntimeError, match="continuous monitoring"):
        price_instrument(json.dumps(instrument), json.dumps(market), "2024-01-02", "barrier_bs_continuous", [])

"""Behavioral contracts shared with the WASM valuation facade."""

from __future__ import annotations

import copy
import json
from pathlib import Path

import pytest

from finstack_quant.valuations.instruments import price_instrument

_FIXTURES = json.loads(
    (
        Path(__file__).parents[2] / "finstack-quant/valuations/tests/fixtures/valuation_binding_regressions.json"
    ).read_text()
)


@pytest.mark.parametrize("model", ["black76", "static_replication"])
@pytest.mark.parametrize("missing", ["curves", "surfaces"])
def test_cms_missing_market_data_preserves_key_error(model: str, missing: str) -> None:
    market = copy.deepcopy(_FIXTURES["market"])
    market[missing] = []
    with pytest.raises(KeyError, match="USD-"):
        price_instrument(json.dumps(_FIXTURES["cms"]), json.dumps(market), "2025-01-02", model)


@pytest.mark.parametrize("model", ["black76", "static_replication"])
def test_cms_invalid_contract_preserves_value_error(model: str) -> None:
    instrument = copy.deepcopy(_FIXTURES["cms"])
    instrument["instrument"]["spec"]["accrual_fractions"] = []
    with pytest.raises(ValueError, match="vectors must have equal length"):
        price_instrument(json.dumps(instrument), json.dumps(_FIXTURES["market"]), "2025-01-02", model)


def test_metric_fixing_requirement_preserves_value_error() -> None:
    instrument = copy.deepcopy(_FIXTURES["cms"])
    instrument["instrument"]["spec"].update(
        fixing_dates=["2025-01-03"],
        payment_dates=["2025-04-03"],
        metric_pricing_overrides={"theta_period": "2D"},
    )
    with pytest.raises(ValueError, match=r"(?i)fixing"):
        price_instrument(
            json.dumps(instrument), json.dumps(_FIXTURES["market"]), "2025-01-02", "static_replication", ["theta"]
        )


def test_commodity_mc_details_survive_binding_and_scale_with_position() -> None:
    instrument = copy.deepcopy(_FIXTURES["commodity"])

    def price() -> dict:
        result = price_instrument(
            json.dumps(instrument), json.dumps(_FIXTURES["market"]), "2025-01-02", "monte_carlo_schwartz_smith"
        )
        return json.loads(result.to_json())

    base = price()
    replay = price()
    assert base["value"] == replay["value"]
    assert base["details"] == replay["details"], "same seed and contract must replay exactly"
    assert base["details"]["type"] == "monte_carlo"
    details = base["details"]["data"]
    assert details["standard_error"] > 0
    assert details["estimator_paths"] == details["simulated_paths"] == 2000
    assert details["seed"] == 42
    assert len(details["time_grid"]) == 253
    assert details["time_grid"][0] == 0
    assert not details["antithetic"]
    assert not details["sobol"]
    assert not details["brownian_bridge"]
    instrument["instrument"]["spec"]["quantity"] *= 10
    scaled = price()
    assert float(scaled["value"]["amount"]) == pytest.approx(10 * float(base["value"]["amount"]))
    assert scaled["details"]["data"]["standard_error"] == pytest.approx(10 * details["standard_error"])


def test_metric_missing_volatility_preserves_key_error() -> None:
    # SS pricing needs only the forward and discount curve; analytic vega
    # additionally needs the missing WTI-VOL surface, inside metric enrichment.
    with pytest.raises(KeyError, match="WTI-VOL"):
        price_instrument(
            json.dumps(_FIXTURES["commodity"]),
            json.dumps(_FIXTURES["market"]),
            "2025-01-02",
            "monte_carlo_schwartz_smith",
            ["vega"],
        )


@pytest.mark.parametrize("strike", ["0.00001", "0.02", "0.03", "0.04"])
def test_cms_replication_zero_volatility_matches_discounted_intrinsic(strike: str) -> None:
    instrument = copy.deepcopy(_FIXTURES["cms"])
    instrument["instrument"]["spec"]["strike"] = strike
    market = copy.deepcopy(_FIXTURES["market"])
    surface = market["surfaces"][0]
    surface["vols_row_major"] = [0.0] * len(surface["vols_row_major"])
    prices = [
        price_instrument(json.dumps(instrument), json.dumps(market), "2025-01-02", model).price
        for model in ["black76", "static_replication"]
    ]
    assert prices[1] == pytest.approx(prices[0], abs=1e-7)

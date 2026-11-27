from __future__ import annotations

import json

import pytest

from finstack_quant.portfolio import StrategyAllocationInput, WeightAllocationSpec, allocate_weights


def test_allocate_weights_inverse_volatility_from_dict_spec() -> None:
    spec = {
        "scheme": "inverse_volatility",
        "total_capital": 1000.0,
        "strategies": [
            {"id": "low_vol", "returns": [0.01, 0.02, 0.01, 0.02]},
            {"id": "high_vol", "returns": [0.05, -0.05, 0.05, -0.05]},
        ],
        "money_decimal_places": 2,
    }

    wire_result = allocate_weights(spec).to_json()
    with pytest.raises(TypeError, match="spec must be a list, a dict or a DataFrame, not a string"):
        allocate_weights(json.dumps(spec))
    request = WeightAllocationSpec(
        "inverse_volatility",
        1000.0,
        [
            StrategyAllocationInput("low_vol", returns=[0.01, 0.02, 0.01, 0.02]),
            StrategyAllocationInput("high_vol", returns=[0.05, -0.05, 0.05, -0.05]),
        ],
        money_decimal_places=2,
    )
    assert allocate_weights(request).to_json() == wire_result
    result = json.loads(wire_result)

    assert result["scheme"] == "inverse_volatility"
    low, high = result["allocations"]
    assert low["id"] == "low_vol"
    assert low["weight"] > high["weight"]
    assert low["weight"] + high["weight"] == pytest.approx(1.0)

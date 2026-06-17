from __future__ import annotations

import json

import pytest

from finstack_quant.portfolio import allocate_weights, validate_allocation_json


def test_allocate_weights_json_entrypoint_inverse_volatility() -> None:
    spec = {
        "scheme": "inverse_volatility",
        "total_capital": 1000.0,
        "strategies": [
            {"id": "low_vol", "returns": [0.01, 0.02, 0.01, 0.02]},
            {"id": "high_vol", "returns": [0.05, -0.05, 0.05, -0.05]},
        ],
        "money_decimal_places": 2,
    }

    validate_allocation_json(json.dumps(spec))
    result = json.loads(allocate_weights(json.dumps(spec)))

    assert result["scheme"] == "inverse_volatility"
    low, high = result["allocations"]
    assert low["id"] == "low_vol"
    assert low["weight"] > high["weight"]
    assert low["weight"] + high["weight"] == pytest.approx(1.0)

from __future__ import annotations

import json

import pytest

from finstack_quant.portfolio import allocate_weights


def test_allocate_weights_inverse_volatility_from_json_spec() -> None:
    spec = {
        "scheme": "inverse_volatility",
        "total_capital": 1000.0,
        "strategies": [
            {"id": "low_vol", "returns": [0.01, 0.02, 0.01, 0.02]},
            {"id": "high_vol", "returns": [0.05, -0.05, 0.05, -0.05]},
        ],
        "money_decimal_places": 2,
    }

    result = json.loads(allocate_weights(json.dumps(spec)).to_json())

    assert result["scheme"] == "inverse_volatility"
    low, high = result["allocations"]
    assert low["id"] == "low_vol"
    assert low["weight"] > high["weight"]
    assert low["weight"] + high["weight"] == pytest.approx(1.0)

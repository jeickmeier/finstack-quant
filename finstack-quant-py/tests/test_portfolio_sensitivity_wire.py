"""The factor-sensitivity matrix has one Rust-owned wire form in both hosts.

``SensitivityMatrix.to_json`` / ``from_json`` use the Rust
``SensitivityMatrixJson`` shape (``{base_currency, position_ids, factor_ids,
data}`` with nested rows), which is also what WASM ``computeFactorSensitivities``
returns and ``decomposeFactorRisk`` accepts. The golden below is asserted
identically by ``finstack-quant-wasm/tests/facade/portfolio.test.mjs``.
"""

from __future__ import annotations

import json

import pytest

from finstack_quant.core.market_data import MarketContext
from finstack_quant.portfolio import SensitivityMatrix, compute_factor_sensitivities, decompose_factor_risk

SENSITIVITY_WIRE = {
    "base_currency": "USD",
    "position_ids": ["A", "B"],
    "factor_ids": ["F1", "F2"],
    "data": [[1.0, 2.0], [3.0, -1.0]],
}
SENSITIVITY_COVARIANCE = json.dumps({"factor_ids": ["F1", "F2"], "n": 2, "data": [0.04, 0.01, 0.01, 0.09]})


def test_wire_round_trips_exactly_and_decomposes_like_wasm() -> None:
    matrix = SensitivityMatrix.from_json(json.dumps(SENSITIVITY_WIRE))
    assert json.loads(matrix.to_json()) == SENSITIVITY_WIRE
    assert matrix.position_deltas(1) == [3.0, -1.0]

    variance = decompose_factor_risk(matrix, SENSITIVITY_COVARIANCE)
    # exposures e = [4, 1]; e' S e = 16*0.04 + 2*4*0.01 + 0.09 = 0.81
    assert variance.total_risk == pytest.approx(0.81, abs=1e-12)
    assert variance.measure == "variance"
    assert len(variance.position_factor_contributions()) == 4
    assert variance.position_residual_contributions() == []


@pytest.mark.parametrize(
    "measure",
    [{"var": {"confidence": 0.99}}, {"expected_shortfall": {"confidence": 0.975}}, "volatility"],
)
def test_measure_keeps_its_serde_form(measure: object) -> None:
    matrix = SensitivityMatrix.from_json(json.dumps(SENSITIVITY_WIRE))
    decomposition = decompose_factor_risk(matrix, SENSITIVITY_COVARIANCE, measure)
    assert decomposition.measure == measure
    assert json.loads(decomposition.to_json())["measure"] == measure


@pytest.mark.parametrize(
    ("wire", "message"),
    [
        ({**SENSITIVITY_WIRE, "data": [[1.0, 2.0]]}, r"1 row\(s\) but position_ids declares 2"),
        ({**SENSITIVITY_WIRE, "data": [[1.0, 2.0], [3.0]]}, r"row 1 has 1 element\(s\)"),
        ({**SENSITIVITY_WIRE, "n_factors": 2}, r"unknown field `n_factors`"),
        ({**SENSITIVITY_WIRE, "base_currency": "NOT_A_CCY"}, r"NOT_A_CCY"),
        ({**SENSITIVITY_WIRE, "data": [1.0, 2.0, 3.0, -1.0]}, r"invalid type"),
    ],
)
def test_malformed_wire_raises_value_error(wire: dict[str, object], message: str) -> None:
    with pytest.raises(ValueError, match=message):
        SensitivityMatrix.from_json(json.dumps(wire))


def test_engine_output_uses_the_wire_form() -> None:
    matrix = compute_factor_sensitivities("[]", "[]", MarketContext(), "2025-01-15", "EUR")
    assert json.loads(matrix.to_json()) == {
        "base_currency": "EUR",
        "position_ids": [],
        "factor_ids": [],
        "data": [],
    }

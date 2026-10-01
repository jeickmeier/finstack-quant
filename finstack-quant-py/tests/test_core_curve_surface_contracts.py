"""Boundary and persistence contracts for core correlation curves and SABR data."""

import json

import pytest

from finstack_quant.core.market_data import BaseCorrelationCurve, SabrParameterData, VolCube
from finstack_quant.models.volatility import get_cube_vol


@pytest.mark.parametrize(
    ("detachments", "correlations"),
    [([3.0, 7.0, 10.0], [0.25, 0.45]), ([3.0, 7.0], [0.25, 0.45, 0.9])],
)
def test_base_correlation_wire_rejects_mismatched_nodes(detachments: list[float], correlations: list[float]) -> None:
    wire = {"id": "CDX", "detachment_points": detachments, "correlations": correlations}
    message = (
        f"Base correlation curve 'CDX': detachment_points length {len(detachments)} "
        f"does not match correlations length {len(correlations)}"
    )
    with pytest.raises(ValueError, match=message):
        BaseCorrelationCurve.from_json(json.dumps(wire))


def test_base_correlation_stress_and_zero_survive_wire_roundtrip() -> None:
    curve = BaseCorrelationCurve("STRESS", [(0.0, 0.0), (7.0, 1.0), (100.0, 0.6)])
    restored = BaseCorrelationCurve.from_json(curve.to_json())
    assert restored.correlations == [0.0, 1.0, 0.6]
    for detachment in [0.0, 3.0, 7.0, 50.0, 100.0]:
        assert restored.correlation(detachment) == curve.correlation(detachment)


def test_sabr_zero_vol_of_vol_preserves_exact_black_cube() -> None:
    node = SabrParameterData(0.2, 1.0, -0.5, 0.0)
    cube = VolCube("BLACK", [1.0, 5.0], [2.0, 10.0], [node] * 4, [0.03] * 4)
    restored = VolCube.from_json(cube.to_json())
    assert restored.params_at(0, 0).nu == 0.0
    for strike in [0.02, 0.03, 0.04]:
        assert get_cube_vol(restored, 3.0, 6.0, strike) == pytest.approx(0.2, abs=1e-14, rel=0.0)

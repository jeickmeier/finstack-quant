"""Model metadata and validation remain intact across the Python boundary."""

import json
import math

import pytest

from finstack_quant.core.market_data import VolCube, VolCubeExpirySlice, VolSurface
from finstack_quant.models.correlation import CopulaSpec
from finstack_quant.models.credit import DynamicRecoverySpec, EndogenousHazardSpec
from finstack_quant.models.factor.risk import evaluate_risk_budget
from finstack_quant.models.volatility import (
    get_cube_expiry_slice_vol,
    get_cube_expiry_slice_vol_clamped,
    get_cube_normal_vol,
    get_cube_vol,
    materialize_cube_expiry_slice,
    materialize_cube_expiry_slice_normal,
    materialize_cube_tenor_slice,
)


def test_cube_materialization_retains_expiry_tenor_and_displacement() -> None:
    node = {"alpha": 0.03, "beta": 0.5, "rho": -0.2, "nu": 0.4, "shift": 0.03}
    cube = VolCube("SHIFT", [1.0, 2.0], [5.0, 10.0], [node] * 4, [0.01] * 4)
    strikes = [-0.01, 0.0, 0.01]
    grid = materialize_cube_expiry_slice(cube, 1.0, strikes)
    assert isinstance(grid, VolCubeExpirySlice)
    assert grid.get_expiry() == 1.0
    assert grid.get_tenors() == [5.0, 10.0]
    assert grid.get_grid_shape() == (2, 3)
    assert grid.get_quote_type() == "shifted_black_lognormal"
    assert grid.get_displacements() == [0.03, 0.03]
    for tenor in grid.get_tenors():
        assert get_cube_expiry_slice_vol(grid, tenor, -0.01) == pytest.approx(get_cube_vol(cube, 1.0, tenor, -0.01))
    assert get_cube_expiry_slice_vol_clamped(grid, 20.0, -0.01) == pytest.approx(get_cube_vol(cube, 1.0, 10.0, -0.01))
    with pytest.raises(ValueError, match="out of bounds"):
        get_cube_expiry_slice_vol(grid, 20.0, -0.01)
    restored = VolCubeExpirySlice.from_json(grid.to_json())
    assert restored.get_expiry() == grid.get_expiry()
    assert restored.get_displacements() == grid.get_displacements()
    frame = grid.to_dataframe()
    assert frame.columns.tolist() == ["expiry", "tenor", "strike", "vol"]
    assert frame["expiry"].tolist() == [1.0] * 6

    normal = materialize_cube_expiry_slice_normal(cube, 1.0, strikes)
    assert normal.get_quote_type() == "normal"
    assert normal.get_displacements() is None
    assert get_cube_expiry_slice_vol(normal, 5.0, -0.01) == pytest.approx(get_cube_normal_vol(cube, 1.0, 5.0, -0.01))
    surface = materialize_cube_tenor_slice(cube, 5.0, strikes)
    assert surface.quote_type == "shifted_black_lognormal"
    assert surface.get_displacements() == [0.03, 0.03]
    assert VolSurface.from_json(surface.to_json()).get_displacements() == [0.03, 0.03]


def test_shifted_surface_constructor_keeps_metadata_and_validates_shape() -> None:
    for vols in [[0.2, 0.21], [[0.2, 0.21]]]:
        surface = VolSurface(
            "SHIFT",
            [1.0],
            [-0.01, 0.01],
            vols,
            quote_type="shifted_black_lognormal",
            displacements=[0.03],
        )
        assert surface.get_displacements() == [0.03]
        assert surface.quote_type == "shifted_black_lognormal"
    with pytest.raises(ValueError, match="displacement"):
        VolSurface("SHIFT", [1.0], [-0.01, 0.01], [0.2, 0.21], quote_type="shifted_black_lognormal")
    with pytest.raises(ValueError, match="displacement"):
        VolCubeExpirySlice(
            "SHIFT", 1.0, [5.0, 10.0], [0.01], [0.2, 0.2], quote_type="shifted_black_lognormal", displacements=[0.03]
        )


@pytest.mark.parametrize("recovery", [-0.1, 1.5])
def test_dynamic_recovery_json_cannot_bypass_validation(recovery: float) -> None:
    payload = json.loads(DynamicRecoverySpec.constant(0.4).to_json())
    payload["base_recovery"] = recovery
    with pytest.raises(ValueError, match="Invalid input data"):
        DynamicRecoverySpec.from_json(json.dumps(payload))


def test_endogenous_hazard_rejects_empty_tabular_snapshot() -> None:
    payload = json.loads(EndogenousHazardSpec.power_law(0.1, 1.5, 2.0).to_json())
    payload["leverage_hazard_map"] = {"tabular": {"leverage_points": [], "hazard_points": []}}
    with pytest.raises(ValueError, match="Input dimensions do not match"):
        EndogenousHazardSpec.from_json(json.dumps(payload))


@pytest.mark.parametrize("invalid", [math.nan, math.inf, -math.inf])
def test_risk_budget_rejects_non_finite_risk_inputs(invalid: float) -> None:
    with pytest.raises(ValueError, match="component VaR"):
        evaluate_risk_budget(["A"], [invalid], [1.0], 1.0)
    with pytest.raises(ValueError, match="portfolio_var"):
        evaluate_risk_budget(["A"], [1.0], [1.0], invalid)
    with pytest.raises(ValueError, match="utilization_threshold"):
        evaluate_risk_budget(["A"], [1.0], [1.0], 1.0, invalid)


def test_risk_budget_rejects_negative_targets_even_if_they_sum_to_one() -> None:
    with pytest.raises(ValueError, match="risk budget target"):
        evaluate_risk_budget(["A", "B"], [1.0, 1.0], [-0.5, 1.5], 2.0)


def test_rfl_tail_dependence_is_a_probability_with_exact_endpoints() -> None:
    copula = CopulaSpec.random_factor_loading(0.2).build()
    assert copula.tail_dependence(0.0) == 0.0
    assert copula.tail_dependence(1.0) == 1.0
    assert 0.0 < copula.tail_dependence(0.3) < 1.0

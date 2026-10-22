import json

import pytest

from finstack_quant.calibration import CalibrationConfig, RateBounds, SolverConfig, ValidationConfig


def test_solver_config_wire_shape() -> None:
    config = SolverConfig.from_json("{}")
    assert json.loads(config.to_json()) == {"tolerance": 1e-12, "max_iterations": 100}
    configured = SolverConfig(tolerance=1e-10, max_iterations=200)
    assert SolverConfig.from_json(configured.to_json()) == configured


@pytest.mark.parametrize("field", ["bracket_expansion", "initial_bracket_size", "bracket_min", "bracket_max"])
def test_solver_config_rejects_unused_bracket_controls(field: str) -> None:
    with pytest.raises(ValueError, match=field):
        SolverConfig.from_json(json.dumps({field: 1.0}))


def test_calibration_config_merges_nested_wire_fields() -> None:
    defaults = CalibrationConfig().to_dict()
    config = CalibrationConfig(solver={"max_iterations": 64}, use_parallel=False)
    assert config.solver.max_iterations == 64
    assert config.solver.tolerance == defaults["solver"]["tolerance"]
    assert config.to_dict()["validation"] == defaults["validation"]
    assert not config.use_parallel
    assert not hasattr(config, "tolerance")
    assert not hasattr(config, "max_iterations")


def test_calibration_config_accepts_typed_nested_values() -> None:
    solver = SolverConfig(tolerance=1e-10, max_iterations=200)
    bounds = RateBounds(-0.02, 0.25)
    validation = ValidationConfig()
    typed = CalibrationConfig(solver=solver, rate_bounds=bounds, rate_bounds_policy="explicit", validation=validation)
    wire = CalibrationConfig(
        solver=json.loads(solver.to_json()),
        rate_bounds=json.loads(bounds.to_json()),
        rate_bounds_policy="explicit",
        validation=validation.to_dict(),
    )
    assert typed.to_dict() == wire.to_dict()
    assert typed.to_dict()["rate_bounds_policy"] == "explicit"
    assert CalibrationConfig(rate_bounds=bounds).to_dict()["rate_bounds_policy"] == "auto_currency"


@pytest.mark.parametrize("field", ["tolerance", "max_iterations", "unknown_field"])
def test_calibration_config_rejects_flat_or_unknown_fields(field: str) -> None:
    with pytest.raises(ValueError, match=field):
        CalibrationConfig(**{field: 1})


def test_calibration_config_rejects_unknown_nested_fields() -> None:
    with pytest.raises(ValueError, match="unknown_field"):
        CalibrationConfig(solver={"unknown_field": 1})
    with pytest.raises(ValueError, match="null"):
        CalibrationConfig(solver=None)

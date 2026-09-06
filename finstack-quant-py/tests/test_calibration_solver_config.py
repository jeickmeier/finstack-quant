import json

import pytest

from finstack_quant.calibration import SolverConfig


def test_solver_config_wire_shape() -> None:
    config = SolverConfig.from_json("{}")
    assert json.loads(config.to_json()) == {"tolerance": 1e-12, "max_iterations": 100}
    configured = SolverConfig(tolerance=1e-10, max_iterations=200)
    assert SolverConfig.from_json(configured.to_json()) == configured


@pytest.mark.parametrize("field", ["bracket_expansion", "initial_bracket_size", "bracket_min", "bracket_max"])
def test_solver_config_rejects_unused_bracket_controls(field: str) -> None:
    with pytest.raises(ValueError, match=field):
        SolverConfig.from_json(json.dumps({field: 1.0}))

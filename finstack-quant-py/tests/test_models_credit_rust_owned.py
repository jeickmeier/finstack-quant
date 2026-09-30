"""Credit-model behaviour owned by Rust and shared with the WASM facade.

``finstack-quant-wasm/tests/facade/models_credit_merton.test.mjs`` asserts the
same cases: the seeded path simulation, the FromStr label lists, the
constructors' finiteness checks and the validating spec deserialization all
live in ``finstack_quant_models::credit``.
"""

from __future__ import annotations

import datetime
import math

import pytest

from finstack_quant.models.credit import (
    CreditState,
    DynamicRecoverySpec,
    EndogenousHazardSpec,
    MertonModel,
    ToggleExerciseModel,
    liability_management,
)

# First path of MertonModel(100, 0.25, 80, 0.05).simulate_paths(2, 4, 1.0, 7, True).
SEEDED_FIRST_PATH = [100.0, 106.3619193858413, 173.31551865077458, 179.24766644780442, 171.48303098644598]


def test_simulate_paths_is_seeded_in_rust() -> None:
    model = MertonModel(100.0, 0.25, 80.0, 0.05)
    paths = model.simulate_paths(2, 4, 1.0, 7, True)
    assert paths.path(0) == pytest.approx(SEEDED_FIRST_PATH, rel=1e-12)
    assert paths.asset_values == model.simulate_paths(2, 4, 1.0, 7, True).asset_values
    with pytest.raises(TypeError):
        model.simulate_paths(2, 4, 1.0, 7)  # type: ignore[call-arg]


def test_defaults_rust_leaves_to_the_caller_are_required() -> None:
    model = MertonModel(100.0, 0.25, 80.0, 0.05)
    with pytest.raises(TypeError):
        model.to_hazard_curve("X", datetime.date(2025, 1, 15), [1.0, 5.0], 0.4)  # type: ignore[call-arg]
    with pytest.raises(TypeError):
        liability_management.analyze_lme("tender_offer", 100.0, 0.9)  # type: ignore[call-arg]
    with pytest.raises(TypeError):
        liability_management.analyze_exchange_offer(80.0, 90.0)  # type: ignore[call-arg]
    lme = liability_management.analyze_lme("tender_offer", 100.0, 0.9, 1.0)
    assert lme.cost == pytest.approx(90.0)


def test_default_probabilities_follow_the_rust_horizon_rule() -> None:
    model = MertonModel(100.0, 0.25, 80.0, 0.05)
    series = model.default_probabilities([1.0, 0.0, -1.0])
    assert list(series) == [model.default_probability(1.0), 0.0, 0.0]


def test_toggle_label_errors_come_from_rust() -> None:
    with pytest.raises(ValueError, match="unknown credit state variable") as variable:
        ToggleExerciseModel.threshold("bogus", 1.0, "above")
    assert str(variable.value) == (
        "unknown credit state variable: bogus (expected one of hazard_rate, distance_to_default, leverage)"
    )
    with pytest.raises(ValueError, match="unknown threshold direction") as direction:
        ToggleExerciseModel.threshold("leverage", 1.0, "sideways")
    assert str(direction.value) == "unknown threshold direction: sideways (expected one of above, below)"


def test_constructors_reject_values_json_cannot_carry() -> None:
    with pytest.raises(ValueError, match="must be finite"):
        CreditState(hazard_rate=math.nan)
    with pytest.raises(ValueError, match="must be finite"):
        ToggleExerciseModel.threshold("leverage", math.nan, "above")
    with pytest.raises(ValueError, match="must be finite"):
        ToggleExerciseModel.stochastic("leverage", 1.0, math.inf)
    with pytest.raises(ValueError, match="nested_paths"):
        ToggleExerciseModel.optimal(0, 0.05, 0.3, 0.03, 1.0)
    with pytest.raises(ValueError, match="must be finite"):
        ToggleExerciseModel.optimal(100, math.nan, 0.3, 0.03, 1.0)
    with pytest.raises(ValueError, match="must be finite"):
        EndogenousHazardSpec.power_law(0.05, 1.5, math.nan)


def test_spec_json_goes_through_the_rust_constructors() -> None:
    for payload in (
        '{"base_recovery":1.5,"base_notional":-10.0,"model":"constant"}',
        '{"base_recovery":-0.5,"base_notional":100.0,"model":"inverse_linear"}',
        '{"base_recovery":0.4,"base_notional":7.0,"model":"constant"}',
    ):
        with pytest.raises(ValueError, match="invalid DynamicRecoverySpec JSON"):
            DynamicRecoverySpec.from_json(payload)
    with pytest.raises(ValueError, match="invalid EndogenousHazardSpec JSON"):
        EndogenousHazardSpec.from_json(
            '{"base_hazard_rate":0.05,"base_leverage":1.5,'
            '"leverage_hazard_map":{"tabular":{"leverage_points":[],"hazard_points":[]}}}'
        )
    spec = DynamicRecoverySpec.inverse_linear(0.4, 100.0)
    assert DynamicRecoverySpec.from_json(spec.to_json()).to_json() == spec.to_json()

"""Behaviour that moved from the Python binding into Rust (parity slice P5).

Each case pins a result or an exception type that Python now takes from the
Rust crate, so the WASM binding (which calls the same Rust entry points)
reports the same thing. The WASM twins are in
``finstack-quant-wasm/tests/facade/models_handles.test.mjs``.
"""

from __future__ import annotations

from pathlib import Path

import pytest

from finstack_quant.models import correlation, monte_carlo
from finstack_quant.models.credit import MertonModel, migration, pd as pd_models
from finstack_quant.models.factor import credit as factor_credit, risk as factor_risk
from finstack_quant.models.rates import dtsm

ROOT = Path(__file__).parents[2]
CANONICAL_MODEL = ROOT / "finstack-quant/models/tests/data/canonical/credit_factor_model.json"


def _generator() -> migration.GeneratorMatrix:
    scale = migration.RatingScale.custom(["A", "B", "D"])
    return migration.GeneratorMatrix(scale, [[-0.10, 0.08, 0.02], [0.10, -0.20, 0.10], [0.0, 0.0, 0.0]])


def test_default_probabilities_match_the_scalar_method() -> None:
    model = MertonModel(100.0, 0.25, 80.0, 0.05)
    grid = model.default_probabilities([1.0, 5.0])
    assert list(grid) == [model.default_probability(1.0), model.default_probability(5.0)]


def test_transition_probability_by_index_rejects_an_index_outside_the_scale() -> None:
    matrix = migration.project(_generator(), 1.0)
    assert matrix.probability_by_index(0, 2) == matrix.probability("A", "D")
    with pytest.raises(ValueError, match="out of range"):
        matrix.probability_by_index(0, 3)


def test_seeded_migration_simulation_is_reproducible() -> None:
    simulator = migration.MigrationSimulator(_generator(), 5.0)
    first = simulator.simulate(0, 50, 7)
    assert first.to_json() == simulator.simulate(0, 50, 7).to_json()
    assert 0.0 < first.default_rate < 1.0
    assert simulator.empirical_matrix(200, 7).to_matrix() == simulator.empirical_matrix(200, 7).to_matrix()


def test_master_scale_batch_mapping_reports_the_first_bad_pd() -> None:
    scale = pd_models.MasterScale.sp_assumptions()
    frame = scale.map_pds([0.001, 0.2])
    assert list(frame["input_pd"]) == [0.001, 0.2]
    with pytest.raises(ValueError, match="outside"):
        scale.map_pds([0.01, 5.0])


def test_latent_multi_factor_rejects_the_wrong_number_of_draws() -> None:
    model = correlation.LatentMultiFactor.uncorrelated(2, [0.2, 0.3])
    assert model.generate_correlated_factors([1.0, -1.0]) == [0.2, -0.3]
    with pytest.raises(ValueError, match="expected 2"):
        model.generate_correlated_factors([1.0])


def test_credit_factor_model_policy_names_the_variant() -> None:
    model = factor_credit.CreditFactorModel.from_json(CANONICAL_MODEL.read_text())
    assert model.policy == "dynamic"
    assert len(model.level_names()) == model.n_levels
    assert len(model.issuer_ids()) == model.n_issuers
    assert len(model.factor_ids()) == model.n_factors


def test_vol_horizon_validation_and_labels_come_from_rust() -> None:
    assert factor_credit.VolHorizon.years(0.25).kind == "years"
    assert factor_credit.VolHorizon.n_steps(3).kind == "n_steps"
    with pytest.raises(ValueError, match="finite and non-negative"):
        factor_credit.VolHorizon.years(-1.0)


def test_position_component_var_raises_key_error_for_an_unknown_position() -> None:
    decomposition = factor_risk.parametric_var_decomposition(["A", "B"], [1.0, 2.0], [[0.04, 0.01], [0.01, 0.09]])
    assert factor_risk.position_component_var(decomposition, "A") == decomposition.var_contributions[0].component_var
    with pytest.raises(KeyError, match="missing"):
        factor_risk.position_component_var(decomposition, "missing")


def test_monte_carlo_pricers_resolve_defaults_in_rust() -> None:
    default = monte_carlo.EuropeanPricer()
    assert default.num_paths > 0
    explicit = monte_carlo.EuropeanPricer(500, 7, False)
    assert (explicit.num_paths, explicit.seed, explicit.use_parallel) == (500, 7, False)
    call = explicit.price_call(100.0, 100.0, 0.05, 0.0, 0.2, 1.0)
    assert call.num_paths == 500
    assert explicit.price_call(100.0, 100.0, 0.05, 0.0, 0.2, 1.0, currency="EUR").mean.currency.code == "EUR"

    lsmc = monte_carlo.LsmcPricer(500, 7, False, 10, "polynomial", 3, False)
    assert (lsmc.basis, lsmc.basis_degree, lsmc.antithetic) == ("polynomial", 3, False)
    with pytest.raises(ValueError, match="unknown basis"):
        monte_carlo.LsmcPricer(500, 7, False, 10, "nope")
    with pytest.raises(ValueError, match="unknown basis"):
        lsmc.price_american_put(100.0, 100.0, 0.05, 0.0, 0.2, 1.0, basis="nope")

    asian = monte_carlo.PathDependentPricer(500, 7, False, True)
    assert (asian.num_paths, asian.seed, asian.antithetic) == (500, 7, True)
    with pytest.raises(ValueError, match="num_paths must be positive"):
        monte_carlo.EuropeanPricer(0)


def test_pca_convenience_functions_default_to_three_components() -> None:
    changes = [
        [0.001, 0.0012, 0.0011, 0.0009],
        [-0.0005, -0.0004, -0.0006, -0.0002],
        [0.0008, 0.0005, 0.0002, 0.0001],
        [-0.001, -0.0007, -0.0003, -0.0004],
        [0.0002, 0.0004, 0.0007, 0.0003],
    ]
    assert len(dtsm.yield_pca_fit(changes).eigenvalues) == 3
    assert dtsm.yield_pca_scenario(changes, 0, 1.0) == dtsm.yield_pca_scenario(changes, 0, 1.0, 3)

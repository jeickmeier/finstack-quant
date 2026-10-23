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
CANONICAL_MODEL = ROOT / "finstack-quant/factor/tests/data/canonical/credit_factor_model.json"


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
    matrix, rerun_matrix = simulator.empirical_matrix(200, 7), simulator.empirical_matrix(200, 7)
    assert matrix.to_rows() == rerun_matrix.to_rows()


def test_master_scale_batch_mapping_reports_the_first_bad_pd() -> None:
    scale = pd_models.MasterScale.default_calibration()
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
    call = explicit.price_gbm(100.0, 100.0, 0.05, 0.0, 0.2, 1.0, True)
    assert call.num_paths == 500
    assert str(explicit.price_gbm(100.0, 100.0, 0.05, 0.0, 0.2, 1.0, True, currency="EUR").mean.currency) == "EUR"

    lsmc = monte_carlo.LsmcPricer(500, 7, False, 10, "polynomial", 3, False)
    assert (lsmc.basis, lsmc.basis_degree, lsmc.antithetic) == ("polynomial", 3, False)
    with pytest.raises(ValueError, match="unknown basis"):
        monte_carlo.LsmcPricer(500, 7, False, 10, "nope")
    with pytest.raises(ValueError, match="unknown basis"):
        lsmc.price_american(100.0, 100.0, 0.05, 0.0, 0.2, 1.0, False, basis="nope")

    asian = monte_carlo.PathDependentPricer(500, 7, False, True)
    assert (asian.num_paths, asian.seed, asian.antithetic) == (500, 7, True)
    # A standard error needs two independent estimators: Rust rejects 0 and 1 paths.
    for num_paths in (0, 1):
        with pytest.raises(ValueError, match="EuropeanPricer requires at least two independent path estimators"):
            monte_carlo.EuropeanPricer(num_paths)


def test_pca_has_no_host_only_free_twins_or_invented_component_count() -> None:
    """MODA-008/009: PCA is reached through the bound ``YieldPca`` methods only."""
    changes = [
        [0.001, 0.0012, 0.0011, 0.0009],
        [-0.0005, -0.0004, -0.0006, -0.0002],
        [0.0008, 0.0005, 0.0002, 0.0001],
        [-0.001, -0.0007, -0.0003, -0.0004],
        [0.0002, 0.0004, 0.0007, 0.0003],
    ]
    assert not hasattr(dtsm, "yield_pca_fit")
    assert not hasattr(dtsm, "yield_pca_scenario")
    pca = dtsm.YieldPca.fit_yield_changes(changes)
    assert pca.num_components == 4
    with pytest.raises(TypeError):
        pca.truncated()  # type: ignore[call-arg]
    assert pca.truncated(4).num_components == 4
    two_tenor = [row[:2] for row in changes]
    assert dtsm.YieldPca.fit_yield_changes(two_tenor).truncated(2).num_components == 2


def test_factor_construction_preserves_valid_inputs_and_rejects_invalid_values() -> None:
    for vol in [0.0, 0.001, 3.0]:
        model = correlation.LatentSingleFactor(vol, 20.0)
        assert (model.volatility, model.mean_reversion) == (vol, 20.0)
        spec = correlation.LatentFactorSpec.single_factor(vol, 20.0)
        assert list(spec.build().volatilities) == [vol]
        multi = correlation.LatentMultiFactor.uncorrelated(1, [vol])
        assert list(multi.volatilities) == [vol]
    for rho in [-1.0, 1.0]:
        model = correlation.LatentTwoFactor(0.0, 3.0, rho)
        assert (model.prepay_vol, model.credit_vol, model.correlation) == (0.0, 3.0, rho)
        assert model.cholesky_l11 == 0.0
    for bad in [-0.1, float("nan"), float("inf"), -float("inf")]:
        for construct in [correlation.LatentSingleFactor, correlation.LatentFactorSpec.single_factor]:
            with pytest.raises(
                ValueError, match=r"finite|volatility|correlation|factor|Volatility|Correlation|Invalid"
            ):
                construct(bad, 0.0)
            with pytest.raises(
                ValueError, match=r"finite|volatility|correlation|factor|Volatility|Correlation|Invalid"
            ):
                construct(0.0, bad)
    for bad in [-1.1, 1.1, float("nan"), float("inf")]:
        for construct in [correlation.LatentTwoFactor, correlation.LatentFactorSpec.two_factor]:
            with pytest.raises(
                ValueError, match=r"finite|volatility|correlation|factor|Volatility|Correlation|Invalid"
            ):
                construct(0.2, 0.3, bad)


def test_recovery_construction_and_json_share_strict_bounds() -> None:
    for mean, vol, rho in [
        (-0.1, 0.2, 0.4),
        (1.1, 0.2, 0.4),
        (0.4, -0.1, 0.4),
        (0.4, 0.6, 0.4),
        (0.4, 0.2, -1.1),
        (0.4, 0.2, 1.1),
    ]:
        with pytest.raises(ValueError, match=r"recovery|Recovery"):
            correlation.RecoverySpec.market_correlated(mean, vol, rho)
    for rho in [-1.0, 1.0]:
        spec = correlation.RecoverySpec.market_correlated(0.4, 0.5, rho)
        assert correlation.RecoverySpec.from_json(spec.to_json()) == spec
        assert 0.0 <= spec.build().expected_recovery <= 1.0
    with pytest.raises(ValueError, match=r"recovery|Recovery"):
        correlation.RecoverySpec.from_json(
            '{"type":"market_correlated","mean_recovery":0.4,"recovery_volatility":0.6,"factor_correlation":0.4}'
        )


def test_exposure_json_cannot_bypass_checked_construction() -> None:
    from finstack_quant.models.credit import lgd

    for wire in [
        '{"drawn":-1,"undrawn":2,"ccf":{"ccf":0.75}}',
        '{"drawn":1,"undrawn":-2,"ccf":{"ccf":0.75}}',
        '{"drawn":1,"undrawn":2,"ccf":{"ccf":1.1}}',
    ]:
        with pytest.raises(ValueError, match=r"negative|non-negative|conversion factor"):
            lgd.EadCalculator.from_json(wire)
    wire = '{"drawn":100,"undrawn":20,"ccf":{"ccf":0.75}}'
    assert lgd.EadCalculator.from_json(wire).ead == 115.0

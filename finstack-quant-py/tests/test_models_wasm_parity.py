"""Cross-host goldens for the ``finstack_quant.models`` namespaces.

Each case computes one result through the Python binding and compares it with
``tests/data/models_parity_golden.json``. The WASM facade test
``finstack-quant-wasm/tests/facade/models_parity.test.mjs`` computes the same
cases through the JavaScript binding and compares with the same file, so both
hosts are pinned to one set of Rust results.

Regenerate the file after an intended numerical change with::

    UPDATE_MODELS_PARITY_GOLDEN=1 uv run --no-sync pytest finstack-quant-py/tests/test_models_wasm_parity.py
"""

from __future__ import annotations

from collections.abc import Callable
from datetime import date
from itertools import pairwise
import json
import math
import os
from pathlib import Path
from typing import Any

import pytest

from finstack_quant import models
from finstack_quant.core.market_data import DiscountCurve, FxDeltaVolSurface, VolCube
from finstack_quant.models import correlation, liquidity, monte_carlo, volatility
from finstack_quant.models.credit import (
    AssetDynamics,
    CreditState,
    DynamicRecoverySpec,
    EndogenousHazardSpec,
    MertonBarrierType,
    MertonModel,
    RatingFactorTable,
    ToggleExerciseModel,
    lgd,
    liability_management,
    migration,
    moodys_warf_factor,
    pd as pd_models,
    recovery_waterfall,
    scoring,
)
from finstack_quant.models.factor import credit as factor_credit, risk as factor_risk
from finstack_quant.models.rates import dtsm, hull_white

ROOT = Path(__file__).parents[2]
GOLDEN = Path(__file__).parent / "data" / "models_parity_golden.json"
CANONICAL_MODEL = ROOT / "finstack-quant/models/tests/data/canonical/credit_factor_model.json"

# Monte Carlo and iterative results go through libm on each target; everything
# else is closed form. See INVARIANTS.md section 2.1.
REL_TOL = 1e-9
ABS_TOL = 1e-12

MERTON = (100.0, 0.25, 80.0, 0.05)
GBM = (100.0, 100.0, 0.05, 0.0, 0.2, 1.0)
STATE = {
    "hazard_rate": 0.05,
    "distance_to_default": None,
    "leverage": 0.8,
    "accreted_notional": 100.0,
    "coupon_due": 2.0,
    "asset_value": None,
}
GRADES = [("A", 0.01, 0.005), ("B", 0.10, 0.04), ("C", 1.0, 0.30)]
TRANSITIONS = [[0.90, 0.08, 0.02], [0.10, 0.80, 0.10], [0.0, 0.0, 1.0]]
GENERATOR = [[-0.10, 0.08, 0.02], [0.10, -0.20, 0.10], [0.0, 0.0, 0.0]]
CLAIMS = [
    ("secured", "senior_secured", 1, 100.0, 5.0, 0.0, 60.0, 0.25),
    ("unsecured", "senior_unsecured", 2, 80.0, 0.0, 0.0, None, 0.0),
    ("junior", "subordinated", 3, 50.0, 0.0, 1.0, None, 0.0),
]
EXPOSURES = [
    ("A", 100.0, 0.02, 0.6, [0.4]),
    ("B", 150.0, 0.05, 0.5, [0.5]),
    ("C", 80.0, 0.10, 0.7, [0.3]),
]
PNLS = [
    [1.0, -2.0, 0.5, -4.0, 2.0, -1.0, 0.2, -3.0, 1.5, -0.5],
    [-0.5, -1.0, 1.0, -2.5, 0.3, 0.8, -0.7, -1.5, 0.4, 0.1],
]
PROFILE = {
    "instrument_id": "ACME",
    "mid": 100.0,
    "bid": 99.95,
    "ask": 100.05,
    "avg_daily_volume": 1_000_000.0,
    "avg_trade_size": 500.0,
    "spread_volatility": 0.0002,
    "spread_volatility_kind": "relative",
    "observation_days": 20,
}
TENORS = [1.0, 2.0, 5.0, 10.0]
YIELDS = [
    [0.030, 0.032, 0.036, 0.040],
    [0.031, 0.033, 0.036, 0.041],
    [0.029, 0.031, 0.035, 0.039],
    [0.032, 0.034, 0.038, 0.041],
    [0.033, 0.034, 0.037, 0.042],
    [0.031, 0.032, 0.036, 0.040],
    [0.030, 0.033, 0.037, 0.041],
    [0.034, 0.035, 0.039, 0.043],
]
STRIKES = [80.0, 90.0, 100.0, 110.0, 120.0]
EXPIRIES = [0.5, 1.0]
VOLS = [[0.28, 0.24, 0.20, 0.21, 0.23], [0.27, 0.235, 0.21, 0.215, 0.23]]
SVI = (0.04, 0.4, -0.4, 0.0, 0.2)
CAP_PERIODS = [(0.25, 0.5, 0.25), (0.5, 0.75, 0.25), (0.75, 1.0, 0.25)]


def _merton() -> MertonModel:
    return MertonModel(*MERTON)


def _state() -> CreditState:
    return CreditState(**STATE)


def _scale() -> migration.RatingScale:
    return migration.RatingScale.custom(["A", "B", "D"])


def _generator() -> migration.GeneratorMatrix:
    return migration.GeneratorMatrix(_scale(), GENERATOR)


def _master_scale() -> pd_models.MasterScale:
    return pd_models.MasterScale([pd_models.MasterScaleGrade(*grade) for grade in GRADES])


def _profile() -> liquidity.LiquidityProfile:
    return liquidity.LiquidityProfile(
        PROFILE["instrument_id"],
        PROFILE["mid"],
        PROFILE["bid"],
        PROFILE["ask"],
        PROFILE["avg_daily_volume"],
        PROFILE["avg_trade_size"],
        PROFILE["spread_volatility"],
        PROFILE["spread_volatility_kind"],
        PROFILE["observation_days"],
    )


def _trade() -> liquidity.TradeParams:
    return liquidity.TradeParams(50_000.0, 5.0, 0.02, _profile(), risk_aversion=1e-6)


def _panel() -> dtsm.YieldPanel:
    return dtsm.YieldPanel(TENORS, YIELDS)


def _changes() -> list[list[float]]:
    return [[b - a for a, b in zip(prev, row, strict=True)] for prev, row in pairwise(YIELDS)]


def _money(estimate: Any) -> list[float]:
    return [estimate.mean.amount, estimate.stderr]


def _cube() -> VolCube:
    return VolCube(
        "CUBE",
        [1.0, 2.0],
        [5.0],
        [
            {"alpha": 0.02, "beta": 0.5, "rho": -0.2, "nu": 0.4},
            {"alpha": 0.03, "beta": 0.5, "rho": -0.1, "nu": 0.3},
        ],
        [0.03, 0.035],
    )


def _surface_vols(surface: Any) -> list[float]:
    return json.loads(surface.to_json())["vols_row_major"]


def _json(value: Any) -> Any:
    return json.loads(value.to_json())


def _merton_cases() -> dict[str, Callable[[], Any]]:
    model = _merton
    return {
        "merton.default_probability": lambda: model().default_probability(1.0),
        "merton.default_probabilities": lambda: list(model().default_probabilities([1.0, 3.0, 5.0])),
        "merton.distance_to_default": lambda: [
            model().distance_to_default(1.0),
            model().distance_to_default_with_drift(0.08, 1.0),
            model().default_probability_with_drift(0.08, 1.0),
        ],
        "merton.spreads": lambda: [
            model().implied_spread(5.0, 0.4),
            model().debt_spread(5.0),
            model().cds_par_spread(5.0, 0.4),
        ],
        "merton.implied_equity": lambda: list(model().try_implied_equity(1.0)),
        "merton.kmv_default_point": lambda: MertonModel.kmv_default_point(40.0, 60.0),
        "merton.hazard_curve_sp": lambda: (
            model().to_hazard_curve("ACME", date(2025, 1, 15), [1.0, 3.0, 5.0], 0.4, "act_365f").sp(5.0)
        ),
        "merton.simulate_paths": lambda: list(model().simulate_paths(2, 4, 1.0, 7, True).asset_values),
        "merton.from_equity": lambda: [
            MertonModel.from_equity(30.0, 0.6, 80.0, 0.05, 0.0, 1.0).asset_value,
            MertonModel.from_equity(30.0, 0.6, 80.0, 0.05, 0.0, 1.0).asset_vol,
        ],
        "merton.from_cds_spread": lambda: (
            MertonModel.from_cds_spread(250.0, 0.4, 80.0, 0.05, 5.0, 100.0, 0.0).asset_vol
        ),
        "merton.from_target_pd": lambda: MertonModel.from_target_pd(100.0, 0.25, 0.05, 0.03, 0.05, 1.0).debt_barrier,
        "merton.credit_grades": lambda: MertonModel.credit_grades(40.0, 0.4, 60.0, 0.03, 0.3, 0.5).default_probability(
            5.0
        ),
        "merton.new_with_dynamics": lambda: MertonModel.new_with_dynamics(
            100.0,
            0.25,
            80.0,
            0.05,
            0.01,
            MertonBarrierType.first_passage(0.02),
            AssetDynamics.geometric_brownian(),
        ).default_probability(1.0),
        "merton.jump_diffusion": lambda: MertonModel.new_with_dynamics(
            100.0,
            0.25,
            80.0,
            0.05,
            0.0,
            MertonBarrierType.terminal(),
            AssetDynamics.jump_diffusion(0.5, -0.05, 0.1),
        ).default_probability(1.0),
        "dynamic_recovery.recovery_at_notional": lambda: [
            DynamicRecoverySpec.constant(0.4).recovery_at_notional(250.0),
            DynamicRecoverySpec.inverse_linear(0.4, 100.0).recovery_at_notional(250.0),
            DynamicRecoverySpec.inverse_power(0.4, 100.0, 0.5).recovery_at_notional(250.0),
            DynamicRecoverySpec.floored_inverse(0.4, 100.0, 0.25).recovery_at_notional(250.0),
            DynamicRecoverySpec.linear_decline(0.4, 100.0, 0.1, 0.05).recovery_at_notional(250.0),
        ],
        "endogenous_hazard.hazard": lambda: [
            EndogenousHazardSpec.power_law(0.10, 1.5, 2.5).hazard_at_leverage(1.8),
            EndogenousHazardSpec.exponential(0.10, 1.5, 2.0).hazard_at_leverage(1.8),
            EndogenousHazardSpec.tabular([1.0, 2.0], [0.05, 0.15]).hazard_at_leverage(1.8),
            EndogenousHazardSpec.power_law(0.10, 1.5, 2.5).hazard_after_pik_accrual(120.0, 80.0),
        ],
        "toggle.should_pik": lambda: [
            ToggleExerciseModel.threshold("leverage", 0.7, "above").should_pik_with_uniform(_state(), 0.5),
            ToggleExerciseModel.stochastic("leverage", -2.0, 4.0).should_pik_with_uniform(_state(), 0.5),
            ToggleExerciseModel.stochastic("leverage", -2.0, 4.0).should_pik_with_uniform(_state(), 0.9),
            ToggleExerciseModel.optimal(100, 0.1, 0.2, 0.03, 1.0).should_pik_with_uniform(_state(), 0.5),
        ],
        "rating_factors": lambda: [
            moodys_warf_factor("B2"),
            RatingFactorTable.moodys_standard().get_factor("Baa3"),
            RatingFactorTable.moodys_standard().default_factor,
        ],
        "liability_management.hurdle": lambda: liability_management.TENDER_RECOMMENDATION_HURDLE,
    }


def _credit_cases() -> dict[str, Callable[[], Any]]:
    beta = lambda: lgd.BetaRecovery(0.4, 0.2)  # noqa: E731
    workout = lambda: (  # noqa: E731
        lgd.WorkoutLgd
        .builder()
        .collateral(lgd.CollateralPiece("real_estate", 800_000.0, 0.3))
        .collateral_pieces([lgd.CollateralPiece("cash", 50_000.0, 0.0)])
        .workout_years(2.0)
        .discount_rate(0.05)
        .costs(lgd.WorkoutCosts(0.05, 0.03))
        .build()
    )
    matrix = lambda: migration.TransitionMatrix(_scale(), TRANSITIONS, 1.0)  # noqa: E731
    simulator = lambda: migration.MigrationSimulator(_generator(), 5.0)  # noqa: E731
    return {
        "lgd.beta_recovery": lambda: [
            beta().alpha,
            beta().beta_param,
            beta().variance,
            beta().mean_lgd,
            beta().quantile(0.05),
            lgd.beta_recovery_quantile(0.4, 0.2, 0.95),
        ],
        "lgd.beta_recovery_samples": lambda: [
            list(beta().sample_seeded(4, 42)),
            list(lgd.beta_recovery_sample(0.4, 0.2, 4, 42)),
        ],
        "lgd.seniority_recovery_stats": lambda: [
            lgd.seniority_recovery_stats("senior_unsecured").mean,
            lgd.seniority_recovery_stats("senior_unsecured").std_dev,
            lgd.seniority_recovery_stats("senior_secured", "sp").mean,
        ],
        "lgd.workout": lambda: [
            _json(workout().evaluate(1_000_000.0)),
            workout().lgd(1_000_000.0),
            workout().net_recovery(1_000_000.0),
            workout().recovery_rate(1_000_000.0),
            _json(lgd.workout_lgd(1_000_000.0, [("real_estate", 800_000.0, 0.3)], 0.05, 0.03, 2.0, 0.05)),
            lgd.WorkoutCosts.standard().total_rate,
        ],
        "lgd.downturn": lambda: [
            lgd.DownturnLgd.stressed(0.15, 0.3, 0.999).adjust(0.35),
            lgd.DownturnLgd.regulatory_floor(0.08, 0.1).adjust(0.35),
            lgd.downturn_lgd_stressed(0.35, 0.15, 0.3, 0.999),
            lgd.downturn_lgd_regulatory_floor(0.35, 0.08, 0.1),
            lgd.DownturnLgd.basel_secured().adjust(0.2),
            lgd.DownturnLgd.basel_unsecured().adjust(0.2),
        ],
        "lgd.ead": lambda: [
            lgd.EadCalculator(600.0, 400.0, 0.75).ead,
            lgd.EadCalculator(600.0, 400.0, 0.75).utilization,
            lgd.EadCalculator(600.0, 400.0, 0.75).total_commitment,
            lgd.EadCalculator(600.0, 400.0, 0.75).leq_from_observed_ead(800.0),
            lgd.EadCalculator.revolver(600.0, 400.0).ead,
            lgd.EadCalculator.term_loan(500.0).ead,
            lgd.ead_term_loan(500.0),
            lgd.ead_revolver(600.0, 400.0, 0.75),
        ],
        "pd.cycle": lambda: [
            pd_models.pit_to_ttc(0.02, 0.15, -1.0),
            pd_models.ttc_to_pit(0.02, 0.15, -1.0),
            pd_models.central_tendency([0.01, 0.02, 0.015]),
            pd_models.apply_basel_irb_pd_floor(0.0001),
            pd_models.BASEL_IRB_PD_FLOOR,
        ],
        "pd.master_scale": lambda: [
            _json(_master_scale().map_pd(0.02)),
            list(_master_scale().map_pds([0.5, 0.001, 0.05])["grade"]),
            _master_scale().map_score(scoring.zmijewski_score(0.05, 0.6, 1.5)).grade,
            pd_models.MasterScale.sp_assumptions().n_grades,
            pd_models.MasterScale.moodys_assumptions().n_grades,
        ],
        "scoring": lambda: [
            _json(scoring.altman_z_score(0.1, 0.2, 0.15, 1.5, 1.1)),
            _json(scoring.altman_z_prime(0.1, 0.2, 0.15, 0.9, 1.1)),
            _json(scoring.altman_z_double_prime(0.1, 0.2, 0.15, 0.9)),
            _json(scoring.altman_em_score(0.1, 0.2, 0.15, 0.9)),
            _json(scoring.ohlson_o_score(5.0, 0.6, 0.1, 0.8, 0.0, 0.05, 0.2, 0.0, 0.1)),
            _json(scoring.zmijewski_score(0.05, 0.6, 1.5)),
        ],
        "recovery_waterfall": lambda: _json(
            recovery_waterfall.allocate_recovery(120.0, [recovery_waterfall.RecoveryClaim(*claim) for claim in CLAIMS])
        ),
        "migration.scale": lambda: [
            migration.RatingScale.standard().n_states,
            migration.RatingScale.notched().n_states,
            migration.RatingScale.standard_with_nr().labels(),
            migration.RatingScale.standard().index_of("BBB"),
            migration.RatingScale.standard().warf("B"),
            migration.RatingScale.standard().rating_from_warf(610.0),
            _scale().default_state(),
        ],
        "migration.transition_matrix": lambda: [
            matrix().probability("A", "D"),
            matrix().probability_by_index(1, 2),
            list(matrix().row("B")),
            matrix().compose(matrix()).to_matrix(),
            list(matrix().default_probabilities()),
        ],
        "migration.generator": lambda: [
            _generator().exit_rate("A"),
            _generator().intensity("A", "B"),
            migration.project(_generator(), 2.0).to_matrix(),
            migration.GeneratorMatrix.from_transition_matrix(matrix()).to_matrix(),
        ],
        "migration.simulate": lambda: [
            simulator().simulate(0, 50, 7).default_rate,
            len(simulator().simulate(0, 50, 7)),
            [list(event) for event in simulator().simulate(0, 50, 7).paths[0].transitions()],
            simulator().simulate(0, 50, 7).paths[3].label_at(5.0),
            simulator().empirical_matrix(200, 7).to_matrix(),
        ],
    }


def _correlation_cases() -> dict[str, Callable[[], Any]]:
    exposures = lambda: [correlation.CreditExposure(*row) for row in EXPOSURES]  # noqa: E731
    config = lambda: correlation.PortfolioLossConfig(2000, 42, 0.99, correlation.CopulaSpec.gaussian())  # noqa: E731
    loss = lambda result: [result.expected_loss, result.var, result.expected_shortfall]  # noqa: E731
    pair = lambda: correlation.CorrelatedBernoulli(0.1, 0.2, 0.3)  # noqa: E731
    return {
        "correlation.bernoulli": lambda: [
            list(pair().joint_probabilities()),
            pair().conditional_p2_given_x1(),
            pair().conditional_p1_given_x2(),
            list(pair().sample_from_uniform(0.5)),
            pair().correlation,
        ],
        "correlation.latent": lambda: [
            list(correlation.LatentFactorSpec.two_factor(0.2, 0.25, -0.3).build().correlation_matrix),
            list(correlation.LatentFactorSpec.two_factor(0.2, 0.25, -0.3).build().volatilities),
            correlation.LatentFactorSpec.two_factor(0.2, 0.25, -0.3).build().diagonal_factor_contribution(1, 0.5),
            correlation.LatentFactorSpec.single_factor(0.25, 0.1).build().model_name,
            correlation.LatentTwoFactor.rmbs_standard().cholesky_l11,
            correlation.LatentTwoFactor.clo_standard().correlation,
            correlation.LatentTwoFactor(0.2, 0.25, -0.3).cholesky_l10,
            correlation.LatentSingleFactor(0.25, 0.1).volatility,
            list(
                correlation.LatentMultiFactor(2, [0.2, 0.3], [1.0, 0.5, 0.5, 1.0]).generate_correlated_factors([
                    1.0,
                    -1.0,
                ])
            ),
            list(correlation.LatentMultiFactor.uncorrelated(2, [0.2, 0.3]).correlation_matrix),
        ],
        "correlation.cholesky_decompose": lambda: list(correlation.cholesky_decompose([1.0, 0.5, 0.5, 1.0], 2)),
        "correlation.simulate_portfolio_loss": lambda: [
            loss(correlation.simulate_portfolio_loss(exposures(), config())),
            loss(
                correlation.simulate_portfolio_loss(
                    exposures(), config(), correlation.RecoverySpec.market_correlated(0.4, 0.25, 0.4)
                )
            ),
            correlation.MAX_PORTFOLIO_LOSS_PATHS,
        ],
        "correlation.spec_json": lambda: [
            _json(correlation.CopulaSpec.student_t(5.0)),
            _json(correlation.RecoverySpec.market_correlated(0.4, 0.25, 0.4)),
        ],
    }


def _factor_cases() -> dict[str, Callable[[], Any]]:
    matrix = lambda: factor_credit.FactorCovarianceMatrix(["rates", "credit"], [0.04, 0.01, 0.01, 0.09])  # noqa: E731
    model = lambda: factor_credit.CreditFactorModel.from_json(CANONICAL_MODEL.read_text())  # noqa: E731
    decomposition = lambda: factor_risk.parametric_var_decomposition(  # noqa: E731
        ["A", "B"], [1.0, 2.0], [[0.04, 0.01], [0.01, 0.09]]
    )
    return {
        "factor.vol_horizon": lambda: [
            factor_credit.VolHorizon.years(0.25).kind,
            factor_credit.VolHorizon.years(0.25).years_value,
            factor_credit.VolHorizon.n_steps(5).n,
            factor_credit.VolHorizon.parse('{"n_steps": 3}').kind,
            factor_credit.VolHorizon.one_step().kind,
            factor_credit.VolHorizon.unconditional().kind,
        ],
        "factor.covariance_matrix": lambda: [
            matrix().variance("rates"),
            matrix().covariance("rates", "credit"),
            matrix().correlation("rates", "credit"),
            matrix().to_numpy().tolist(),
        ],
        "factor.credit_factor_model": lambda: [
            model().as_of,
            [d.isoformat() for d in model().calibration_window],
            model().policy,
            model().panel_frequency,
            model().bucket_weighting,
            model().n_levels,
            model().n_issuers,
            model().n_factors,
            model().level_names(),
            model().issuer_ids(),
            model().factor_ids(),
            model().covariance.n_factors,
            model().config.n_factors,
        ],
        "factor.decomposition_config": lambda: [
            _json(factor_risk.DecompositionConfig.parametric(0.975).with_incremental()),
            _json(factor_risk.DecompositionConfig.parametric_95()),
            _json(factor_risk.DecompositionConfig.parametric_99()),
            _json(factor_risk.DecompositionConfig.historical(0.9)),
            _json(factor_risk.DecompositionConfig.historical_95()),
            factor_risk.DecompositionConfig.historical_95().method,
        ],
        "factor.stress_attribution": lambda: _json(factor_risk.build_stress_attribution(["A", "B"], PNLS, 0.8)),
        "factor.position_component_var": lambda: [
            factor_risk.position_component_var(decomposition(), "A"),
            factor_risk.DEFAULT_UTILIZATION_THRESHOLD,
        ],
    }


def _liquidity_cases() -> dict[str, Callable[[], Any]]:
    almgren = lambda: liquidity.AlmgrenChrissModel(1e-7, 1e-6, 1.0)  # noqa: E731
    kyle = lambda: liquidity.KyleLambdaModel.from_amihud(1e-9, 100.0)  # noqa: E731
    calibrated = lambda: liquidity.AlmgrenChrissModel.from_profile(_profile(), 0.02)  # noqa: E731
    return {
        "liquidity.almgren_chriss": lambda: [
            _json(almgren().estimate_cost(_trade())),
            _json(almgren().optimal_trajectory(_trade(), 5)),
            [calibrated().gamma, calibrated().eta, calibrated().delta],
            almgren().model_name,
        ],
        "liquidity.kyle": lambda: [
            kyle().lambda_,
            _json(kyle().estimate_cost(_trade())),
            _json(kyle().optimal_trajectory(_trade(), 4)),
            liquidity.KyleLambdaModel(2e-7).model_name,
        ],
    }


def _monte_carlo_cases() -> dict[str, Callable[[], Any]]:
    european = lambda: monte_carlo.EuropeanPricer(2000, 42, False)  # noqa: E731
    asian = lambda: monte_carlo.PathDependentPricer(2000, 42, False)  # noqa: E731
    lsmc = lambda: monte_carlo.LsmcPricer(2000, 42, False, 20)  # noqa: E731
    fd = {"num_paths": 2000, "seed": 42}
    return {
        "monte_carlo.european": lambda: [
            _money(european().price_call(*GBM)),
            _money(european().price_put(*GBM, 16, "EUR")),
        ],
        "monte_carlo.asian": lambda: [
            _money(asian().price_asian_call(*GBM, 12)),
            _money(asian().price_asian_put(*GBM, 12)),
        ],
        "monte_carlo.lsmc": lambda: [
            _money(lsmc().price_american_put(*GBM)),
            _money(lsmc().price_american_call(*GBM, num_steps=10, basis="polynomial", basis_degree=2)),
            _money(lsmc().price_american_put_unbiased(*GBM, 99)),
            _money(lsmc().price_american_call_unbiased(*GBM, 99)),
            lsmc().basis,
            lsmc().basis_degree,
        ],
        "monte_carlo.finite_diff": lambda: [
            monte_carlo.finite_diff_delta(*GBM, True, **fd).mean,
            monte_carlo.finite_diff_delta_crn(*GBM, True, **fd).mean,
            monte_carlo.finite_diff_gamma(*GBM, False, **fd).mean,
            monte_carlo.finite_diff_gamma_crn(*GBM, False, **fd).mean,
        ],
        "monte_carlo.simulate_gbm_paths": lambda: (
            monte_carlo.simulate_gbm_paths(100.0, 0.05, 0.0, 0.2, 1.0, 4, 3, 42, False).paths
        ),
        "monte_carlo.heston_satisfies_feller": lambda: [
            monte_carlo.heston_satisfies_feller(2.0, 0.04, 0.3),
            monte_carlo.heston_satisfies_feller(0.5, 0.04, 0.5),
        ],
        "monte_carlo.relative_stderr": lambda: [
            european().price_call(*GBM).relative_stderr(),
            european().price_call(*GBM).stderr / abs(european().price_call(*GBM).mean.amount),
        ],
    }


def _rates_cases() -> dict[str, Callable[[], Any]]:
    params = lambda: hull_white.HullWhiteParams.piecewise(0.05, [0.0, 2.5], [0.01, 0.012])  # noqa: E731
    curve = lambda rate: DiscountCurve(  # noqa: E731
        "USD-OIS",
        date(2025, 1, 15),
        [(t, math.exp(-rate * t)) for t in (0.0, 0.5, 1.0, 2.0)],
        day_count="act_365f",
    )
    model = lambda: dtsm.DieboldLi().fit(_panel())  # noqa: E731
    pca = lambda: dtsm.YieldPca.fit(_panel())  # noqa: E731
    return {
        "hull_white.params": lambda: [
            params().sigma(3.0),
            params().state_variance(2.0),
            params().state_covariance(1.0, 2.0),
            params().bond_vol(0.0, 1.0, 5.0),
            hull_white.HullWhiteParams(0.05, 0.01).state_variance(2.0),
        ],
        "hull_white.functions": lambda: [
            hull_white.hw1f_convexity_adjustment(0.05, 0.01, 1.0, 1.25),
            hull_white.hw_bond_vol(0.05, 0.01, 0.0, 1.0, 5.0),
            hull_white.hw1f_zcb_option_price(0.95, 0.80, 0.84, 0.02, True),
            hull_white.hw1f_caplet_forward_rate_normal_vol(0.05, 0.01, 1.0, 0.25),
            hull_white.hw1f_cap_floor_price(0.05, 0.01, CAP_PERIODS, 0.03, True, curve(0.03)),
            hull_white.hw1f_cap_floor_price(0.05, 0.01, CAP_PERIODS, 0.03, False, curve(0.03), curve(0.035)),
        ],
        "dtsm.yield_panel": lambda: [
            _panel().num_dates,
            _panel().num_tenors,
            _panel().yield_changes(),
        ],
        "dtsm.diebold_li": lambda: [
            list(model().forecast(1).yields),
            model().loading_matrix(),
            model().phi,
            list(model().mu),
            model().factors.r_squared_avg,
            dtsm.diebold_li_fit_factors(TENORS, YIELDS).r_squared_avg,
            list(dtsm.diebold_li_forecast(TENORS, YIELDS, 2).yields),
        ],
        "dtsm.yield_pca": lambda: [
            list(pca().variance_explained),
            list(pca().eigenvalues),
            pca().components_for_threshold(0.9),
            list(pca().scenario([1.0])),
            list(pca().apply_scenario([0.03, 0.032, 0.036, 0.04], [1.0, -0.5])),
            list(dtsm.yield_pca_fit(_changes(), 2).eigenvalues),
            list(dtsm.yield_pca_scenario(_changes(), 0, 1.0, 2)),
            list(dtsm.YieldPca.fit_yield_changes(_changes()).cumulative_variance),
        ],
    }


def _volatility_cases() -> dict[str, Callable[[], Any]]:
    svi = lambda: volatility.SviParams(*SVI)  # noqa: E731
    tenor_slice = lambda: volatility.materialize_cube_tenor_slice(_cube(), 5.0, [0.02, 0.03, 0.04])  # noqa: E731
    fx = lambda: FxDeltaVolSurface("EURUSD", [0.5, 1.0], [0.10, 0.11], [0.01, 0.012], [0.002, 0.003])  # noqa: E731
    greeks = lambda: models.bs_greeks(100.0, 100.0, 0.05, 0.0, 0.2, 1.0, True)  # noqa: E731
    smile = lambda: volatility.SabrSmile(volatility.SabrParameters(0.2, 1.0, 0.3, -0.2), 100.0, 1.5)  # noqa: E731
    return {
        "volatility.svi": lambda: [
            svi().total_variance(0.1),
            svi().durrleman_g(0.1),
            svi().implied_vol(0.1, 1.0),
        ],
        "volatility.arbitrage": lambda: [
            _json(volatility.check_surface_grid(STRIKES, EXPIRIES, VOLS, [100.0, 100.0])),
            len(volatility.check_butterfly_grid(STRIKES, EXPIRIES, VOLS, [100.0, 100.0])),
            len(volatility.check_calendar_spread_grid(STRIKES, EXPIRIES, VOLS, [100.0, 100.0])),
            len(volatility.check_local_vol_density_grid(STRIKES, EXPIRIES, VOLS, [100.0, 100.0])),
        ],
        "volatility.cube_slices": lambda: [
            _surface_vols(tenor_slice()),
            _surface_vols(volatility.materialize_cube_tenor_slice_normal(_cube(), 5.0, [0.02, 0.03, 0.04])),
            _surface_vols(volatility.materialize_cube_expiry_slice(_cube(), 1.0, [0.02, 0.03, 0.04])),
            _surface_vols(volatility.materialize_cube_expiry_slice_normal(_cube(), 1.0, [0.02, 0.03, 0.04])),
            volatility.get_surface_vol(tenor_slice(), 1.5, 0.025),
            volatility.get_surface_vol_clamped(tenor_slice(), 9.0, 0.5),
        ],
        "volatility.fx_delta_surface": lambda: _surface_vols(
            volatility.materialize_fx_delta_surface(fx(), 1.10, 0.03, 0.02)
        ),
        "volatility.sabr_smile": lambda: [smile().forward, smile().t],
        "models.bs_greeks_is_valid": lambda: greeks().is_valid(),
    }


def _analytic_cases() -> dict[str, Callable[[], Any]]:
    """Closed-form, COS and SVI kernels (audit finding F128)."""
    smile = [volatility.SviParams(*SVI).implied_vol(math.log(strike / 100.0), 1.0) for strike in STRIKES]
    return {
        "closed_form.forward_greeks": lambda: [
            _json(models.black76_greeks(0.03, 0.035, 2.0, 0.25, True)),
            _json(models.black76_greeks(0.03, 0.035, 2.0, 0.25, False)),
            _json(models.bachelier_greeks(0.03, 0.035, 0.0075, 2.0, True)),
            _json(models.bachelier_greeks(-0.002, 0.001, 0.0075, 2.0, False)),
            models.black_shifted_vega(-0.002, 0.001, 0.3, 2.0, 0.03),
        ],
        "closed_form.barrier_call": lambda: [
            models.barrier_call(100.0, 100.0, barrier, 0.05, 0.01, 0.2, 1.0, direction, knock)
            for barrier, direction in ((120.0, "up"), (85.0, "down"))
            for knock in ("in", "out")
        ],
        "closed_form.asian": lambda: [
            models.asian_option_price(*GBM, 12),
            models.asian_option_price(*GBM, 12, "geometric", False),
            models.asian_option_price(*GBM, 1, "arithmetic", True),
        ],
        "closed_form.quanto": lambda: [
            models.quanto_option_price(100.0, 105.0, 1.5, 0.04, 0.01, 0.02, 0.25, 0.1, -0.3),
            models.quanto_option_price(100.0, 105.0, 1.5, 0.04, 0.01, 0.02, 0.25, 0.1, -0.3, False),
        ],
        "fourier.heston": lambda: [
            models.heston_price(100.0, 100.0, 1.0, 0.05, 0.0, 2.0, 0.04, 0.3, -0.7, 0.04),
            models.heston_price(100.0, 110.0, 1.0, 0.05, 0.0, 2.0, 0.04, 0.3, -0.7, 0.04, False),
        ],
        "fourier.cos": lambda: [
            models.vg_cos_price(100.0, 100.0, 0.05, 0.0, 0.2, -0.14, 0.2, 1.0, True),
            models.vg_cos_price(100.0, 95.0, 0.05, 0.0, 0.2, -0.14, 0.2, 1.0, False, 512),
            models.merton_jump_cos_price(100.0, 100.0, 0.05, 0.0, 0.2, -0.1, 0.15, 0.5, 1.0, True),
            models.merton_jump_cos_price(100.0, 95.0, 0.05, 0.0, 0.2, -0.1, 0.15, 0.5, 1.0, False, 512),
        ],
        "volatility.convert_atm": lambda: [
            volatility.convert_atm_volatility(0.2, "lognormal", "normal", 0.03, 2.0),
            volatility.convert_atm_volatility(0.006, "normal", "lognormal", 0.03, 2.0),
            volatility.convert_atm_volatility(0.2, {"shifted_lognormal": {"shift": 0.02}}, "normal", 0.03, 2.0),
        ],
        "volatility.calibrate_svi": lambda: _json(volatility.calibrate_svi(STRIKES, smile, 100.0, 1.0)),
    }


def cases() -> dict[str, Callable[[], Any]]:
    out: dict[str, Callable[[], Any]] = {}
    for group in (
        _merton_cases,
        _credit_cases,
        _correlation_cases,
        _factor_cases,
        _liquidity_cases,
        _monte_carlo_cases,
        _rates_cases,
        _volatility_cases,
        _analytic_cases,
    ):
        out.update(group())
    return out


CASES = cases()


def _plain(value: Any) -> Any:
    """Reduce a result to JSON-compatible data (tuples become lists)."""
    return json.loads(json.dumps(value))


def _assert_close(actual: Any, expected: Any, where: str) -> None:
    if isinstance(expected, bool) or expected is None or isinstance(expected, str):
        assert actual == expected, where
    elif isinstance(expected, (int, float)):
        assert isinstance(actual, (int, float)), where
        assert not isinstance(actual, bool), where
        assert math.isclose(actual, expected, rel_tol=REL_TOL, abs_tol=ABS_TOL), f"{where}: {actual} vs {expected}"
    elif isinstance(expected, list):
        assert isinstance(actual, list), where
        assert len(actual) == len(expected), where
        for index, (a, e) in enumerate(zip(actual, expected, strict=True)):
            _assert_close(a, e, f"{where}[{index}]")
    else:
        assert isinstance(actual, dict), where
        assert sorted(actual) == sorted(expected), where
        for key in expected:
            _assert_close(actual[key], expected[key], f"{where}.{key}")


def test_golden_file_lists_exactly_these_cases() -> None:
    if os.environ.get("UPDATE_MODELS_PARITY_GOLDEN") == "1":
        results = {name: _plain(compute()) for name, compute in sorted(CASES.items())}
        GOLDEN.write_text(json.dumps(results, indent=2, sort_keys=True) + "\n")
    assert sorted(json.loads(GOLDEN.read_text())) == sorted(CASES)


@pytest.mark.parametrize("name", sorted(CASES))
def test_python_matches_the_cross_host_golden(name: str) -> None:
    expected = json.loads(GOLDEN.read_text())[name]
    _assert_close(_plain(CASES[name]()), expected, name)

/**
 * Cross-host goldens for the `models` namespaces (parity slice P5).
 *
 * Each case computes one result through the WASM facade and compares it with
 * `finstack-quant-py/tests/data/models_parity_golden.json`. The pytest
 * `finstack-quant-py/tests/test_models_wasm_parity.py` computes the same cases
 * through the Python binding against the same file, so both hosts are pinned to
 * one set of Rust results. The file is regenerated from Python (see the pytest).
 *
 * Requires the wasm-pack web build: npm run build (mise run wasm-build).
 */

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { core, models } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

const repoFile = (path) => readFileSync(new URL(`../../../${path}`, import.meta.url), 'utf8');
const GOLDEN = JSON.parse(repoFile('finstack-quant-py/tests/data/models_parity_golden.json'));
const CANONICAL_MODEL = repoFile(
  'finstack-quant/models/tests/data/canonical/credit_factor_model.json'
);

// Monte Carlo and iterative results go through libm on each target; everything
// else is closed form. See INVARIANTS.md section 2.1.
const REL_TOL = 1e-9;
const ABS_TOL = 1e-12;

const { correlation, credit, liquidity, monteCarlo, volatility } = models;
const factorCredit = models.factor.credit;
const factorRisk = models.factor.risk;
const { dtsm, hullWhite } = models.rates;

const MERTON = [100.0, 0.25, 80.0, 0.05];
const GBM = [100.0, 100.0, 0.05, 0.0, 0.2, 1.0];
const STATE = {
  hazard_rate: 0.05,
  distance_to_default: null,
  leverage: 0.8,
  accreted_notional: 100.0,
  coupon_due: 2.0,
  asset_value: null,
};
const GRADES = [
  { label: 'A', upper_pd: 0.01, central_pd: 0.005 },
  { label: 'B', upper_pd: 0.1, central_pd: 0.04 },
  { label: 'C', upper_pd: 1.0, central_pd: 0.3 },
];
const TRANSITIONS = [
  [0.9, 0.08, 0.02],
  [0.1, 0.8, 0.1],
  [0.0, 0.0, 1.0],
];
const GENERATOR = [
  [-0.1, 0.08, 0.02],
  [0.1, -0.2, 0.1],
  [0.0, 0.0, 0.0],
];
const claim = (
  id,
  seniority,
  priority,
  principal,
  accrued,
  penalties,
  collateral_value,
  collateral_haircut
) => ({
  id,
  seniority,
  priority,
  principal,
  accrued,
  penalties,
  collateral_value,
  collateral_haircut,
});
const CLAIMS = [
  claim('secured', 'senior_secured', 1, 100.0, 5.0, 0.0, 60.0, 0.25),
  claim('unsecured', 'senior_unsecured', 2, 80.0, 0.0, 0.0, null, 0.0),
  claim('junior', 'subordinated', 3, 50.0, 0.0, 1.0, null, 0.0),
];
const EXPOSURES = [
  { id: 'A', notional: 100.0, default_probability: 0.02, lgd: 0.6, factor_loadings: [0.4] },
  { id: 'B', notional: 150.0, default_probability: 0.05, lgd: 0.5, factor_loadings: [0.5] },
  { id: 'C', notional: 80.0, default_probability: 0.1, lgd: 0.7, factor_loadings: [0.3] },
];
const PNLS = [
  [1.0, -2.0, 0.5, -4.0, 2.0, -1.0, 0.2, -3.0, 1.5, -0.5],
  [-0.5, -1.0, 1.0, -2.5, 0.3, 0.8, -0.7, -1.5, 0.4, 0.1],
];
const PROFILE = {
  instrument_id: 'ACME',
  mid: 100.0,
  bid: 99.95,
  ask: 100.05,
  avg_daily_volume: 1_000_000.0,
  avg_trade_size: 500.0,
  spread_volatility: 0.0002,
  spread_volatility_kind: 'relative',
  observation_days: 20,
};
const TRADE = {
  quantity: 50_000.0,
  horizon_days: 5.0,
  daily_volatility: 0.02,
  profile: PROFILE,
  risk_aversion: 1e-6,
  reference_price: null,
};
const TENORS = [1.0, 2.0, 5.0, 10.0];
const YIELDS = [
  [0.03, 0.032, 0.036, 0.04],
  [0.031, 0.033, 0.036, 0.041],
  [0.029, 0.031, 0.035, 0.039],
  [0.032, 0.034, 0.038, 0.041],
  [0.033, 0.034, 0.037, 0.042],
  [0.031, 0.032, 0.036, 0.04],
  [0.03, 0.033, 0.037, 0.041],
  [0.034, 0.035, 0.039, 0.043],
];
const CHANGES = YIELDS.slice(1).map((row, i) => row.map((value, j) => value - YIELDS[i][j]));
const STRIKES = [80.0, 90.0, 100.0, 110.0, 120.0];
const EXPIRIES = [0.5, 1.0];
const VOLS = [
  [0.28, 0.24, 0.2, 0.21, 0.23],
  [0.27, 0.235, 0.21, 0.215, 0.23],
];
const SVI = { a: 0.04, b: 0.4, rho: -0.4, m: 0.0, sigma: 0.2 };
const CAP_PERIODS = [
  [0.25, 0.5, 0.25],
  [0.5, 0.75, 0.25],
  [0.75, 1.0, 0.25],
];

const list = (values) => Array.from(values);
const merton = () => new credit.MertonModel(...MERTON);
const scale = () => credit.RatingScale.custom(['A', 'B', 'D']);
const generator = () => new credit.GeneratorMatrix(scale(), GENERATOR);
const masterScale = () => new credit.MasterScale(GRADES);
const panel = () => new dtsm.YieldPanel(TENORS, YIELDS);
const money = (estimate) => [Number(estimate.mean.amount), estimate.stderr];
const json = (handle) => JSON.parse(handle.toJson());
const cube = () =>
  new core.VolCube(
    'CUBE',
    [1, 2],
    [5],
    [0.02, 0.5, -0.2, 0.4, Number.NaN, 0.03, 0.5, -0.1, 0.3, Number.NaN],
    [0.03, 0.035]
  );
const surfaceVols = (surface) => surface.vols_row_major;
const discountCurve = (rate) =>
  new core.DiscountCurve({
    id: 'USD-OIS',
    baseDate: '2025-01-15',
    knots: [0.0, 0.5, 1.0, 2.0].flatMap((t) => [t, Math.exp(-rate * t)]),
    dayCount: 'act_365f',
  });

const CASES = {
  'merton.default_probability': () => merton().defaultProbability(1.0),
  'merton.default_probabilities': () => list(merton().defaultProbabilities([1.0, 3.0, 5.0])),
  'merton.distance_to_default': () => [
    merton().distanceToDefault(1.0),
    merton().distanceToDefaultWithDrift(0.08, 1.0),
    merton().defaultProbabilityWithDrift(0.08, 1.0),
  ],
  'merton.spreads': () => [
    merton().impliedSpread(5.0, 0.4),
    merton().debtSpread(5.0),
    merton().cdsParSpread(5.0, 0.4),
  ],
  'merton.implied_equity': () => list(merton().tryImpliedEquity(1.0)),
  'merton.kmv_default_point': () => credit.MertonModel.kmvDefaultPoint(40.0, 60.0),
  'merton.hazard_curve_sp': () =>
    merton().toHazardCurve('ACME', '2025-01-15', [1.0, 3.0, 5.0], 0.4, 'act_365f').sp(5.0),
  'merton.simulate_paths': () => list(merton().simulatePaths(2, 4, 1.0, 7, true).assetValues),
  'merton.from_equity': () => {
    const m = credit.MertonModel.fromEquity(30.0, 0.6, 80.0, 0.05, 0.0, 1.0);
    return [m.assetValue, m.assetVol];
  },
  'merton.from_cds_spread': () =>
    credit.MertonModel.fromCdsSpread(250.0, 0.4, 80.0, 0.05, 5.0, 100.0, 0.0).assetVol,
  'merton.from_target_pd': () =>
    credit.MertonModel.fromTargetPd(100.0, 0.25, 0.05, 0.03, 0.05, 1.0).debtBarrier,
  'merton.credit_grades': () =>
    credit.MertonModel.creditGrades(40.0, 0.4, 60.0, 0.03, 0.3, 0.5).defaultProbability(5.0),
  'merton.new_with_dynamics': () =>
    credit.MertonModel.newWithDynamics(
      100.0,
      0.25,
      80.0,
      0.05,
      0.01,
      credit.MertonBarrierType.firstPassage(0.02),
      credit.AssetDynamics.geometricBrownian()
    ).defaultProbability(1.0),
  'merton.jump_diffusion': () =>
    credit.MertonModel.newWithDynamics(
      100.0,
      0.25,
      80.0,
      0.05,
      0.0,
      credit.MertonBarrierType.terminal(),
      credit.AssetDynamics.jumpDiffusion(0.5, -0.05, 0.1)
    ).defaultProbability(1.0),
  'dynamic_recovery.recovery_at_notional': () => [
    credit.DynamicRecoverySpec.constant(0.4).recoveryAtNotional(250.0),
    credit.DynamicRecoverySpec.inverseLinear(0.4, 100.0).recoveryAtNotional(250.0),
    credit.DynamicRecoverySpec.inversePower(0.4, 100.0, 0.5).recoveryAtNotional(250.0),
    credit.DynamicRecoverySpec.flooredInverse(0.4, 100.0, 0.25).recoveryAtNotional(250.0),
    credit.DynamicRecoverySpec.linearDecline(0.4, 100.0, 0.1, 0.05).recoveryAtNotional(250.0),
  ],
  'endogenous_hazard.hazard': () => [
    credit.EndogenousHazardSpec.powerLaw(0.1, 1.5, 2.5).hazardAtLeverage(1.8),
    credit.EndogenousHazardSpec.exponential(0.1, 1.5, 2.0).hazardAtLeverage(1.8),
    credit.EndogenousHazardSpec.tabular([1.0, 2.0], [0.05, 0.15]).hazardAtLeverage(1.8),
    credit.EndogenousHazardSpec.powerLaw(0.1, 1.5, 2.5).hazardAfterPikAccrual(120.0, 80.0),
  ],
  'toggle.should_pik': () => [
    credit.ToggleExerciseModel.threshold('leverage', 0.7, 'above').shouldPikWithUniform(STATE, 0.5),
    credit.ToggleExerciseModel.stochastic('leverage', -2.0, 4.0).shouldPikWithUniform(STATE, 0.5),
    credit.ToggleExerciseModel.stochastic('leverage', -2.0, 4.0).shouldPikWithUniform(STATE, 0.9),
    credit.ToggleExerciseModel.optimal(100, 0.1, 0.2, 0.03, 1.0).shouldPikWithUniform(STATE, 0.5),
  ],
  rating_factors: () => [
    credit.moodysWarfFactor('B2'),
    credit.RatingFactorTable.moodysStandard().getFactor('Baa3'),
    credit.RatingFactorTable.moodysStandard().defaultFactor,
  ],
  'liability_management.hurdle': () => credit.tenderRecommendationHurdle(),

  'lgd.beta_recovery': () => {
    const beta = new credit.BetaRecovery(0.4, 0.2);
    return [
      beta.alpha,
      beta.betaParam,
      beta.variance,
      beta.meanLgd,
      beta.quantile(0.05),
      credit.betaRecoveryQuantile(0.4, 0.2, 0.95),
    ];
  },
  'lgd.beta_recovery_samples': () => [
    list(new credit.BetaRecovery(0.4, 0.2).sampleSeeded(4, 42)),
    list(credit.betaRecoverySample(0.4, 0.2, 4, 42)),
  ],
  'lgd.seniority_recovery_stats': () => [
    credit.seniorityRecoveryStats('senior_unsecured').mean,
    credit.seniorityRecoveryStats('senior_unsecured').stdDev,
    credit.seniorityRecoveryStats('senior_secured', 'sp').mean,
  ],
  'lgd.workout': () => {
    const workout = credit.WorkoutLgd.builder()
      .collateral({ collateral_type: 'real_estate', book_value: 800_000.0, haircut: 0.3 })
      .collateralPieces([{ collateral_type: 'cash', book_value: 50_000.0, haircut: 0.0 }])
      .workoutYears(2.0)
      .discountRate(0.05)
      .costs(new credit.WorkoutCosts(0.05, 0.03))
      .build();
    return [
      workout.evaluate(1_000_000.0),
      workout.lgd(1_000_000.0),
      workout.netRecovery(1_000_000.0),
      workout.recoveryRate(1_000_000.0),
      credit.workoutLgd(1_000_000.0, [['real_estate', 800_000.0, 0.3]], 0.05, 0.03, 2.0, 0.05),
      credit.WorkoutCosts.standard().totalRate,
    ];
  },
  'lgd.downturn': () => [
    credit.DownturnLgd.stressed(0.15, 0.3, 0.999).adjust(0.35),
    credit.DownturnLgd.regulatoryFloor(0.08, 0.1).adjust(0.35),
    credit.downturnLgdStressed(0.35, 0.15, 0.3, 0.999),
    credit.downturnLgdRegulatoryFloor(0.35, 0.08, 0.1),
    credit.DownturnLgd.baselSecured().adjust(0.2),
    credit.DownturnLgd.baselUnsecured().adjust(0.2),
  ],
  'lgd.ead': () => {
    const ead = new credit.EadCalculator(600.0, 400.0, 0.75);
    return [
      ead.ead,
      ead.utilization,
      ead.totalCommitment,
      ead.leqFromObservedEad(800.0),
      credit.EadCalculator.revolver(600.0, 400.0).ead,
      credit.EadCalculator.termLoan(500.0).ead,
      credit.eadTermLoan(500.0),
      credit.eadRevolver(600.0, 400.0, 0.75),
    ];
  },
  'pd.cycle': () => [
    credit.pitToTtc(0.02, 0.15, -1.0),
    credit.ttcToPit(0.02, 0.15, -1.0),
    credit.centralTendency([0.01, 0.02, 0.015]),
    credit.applyBaselIrbPdFloor(0.0001),
    credit.baselIrbPdFloor(),
  ],
  'pd.master_scale': () => [
    masterScale().mapPd(0.02),
    masterScale()
      .mapPds([0.5, 0.001, 0.05])
      .map((row) => row.grade),
    masterScale().mapScore(credit.zmijewskiScore(0.05, 0.6, 1.5)).grade,
    credit.MasterScale.spAssumptions().nGrades,
    credit.MasterScale.moodysAssumptions().nGrades,
  ],
  scoring: () => [
    credit.altmanZScore(0.1, 0.2, 0.15, 1.5, 1.1),
    credit.altmanZPrime(0.1, 0.2, 0.15, 0.9, 1.1),
    credit.altmanZDoublePrime(0.1, 0.2, 0.15, 0.9),
    credit.altmanEmScore(0.1, 0.2, 0.15, 0.9),
    credit.ohlsonOScore(5.0, 0.6, 0.1, 0.8, 0.0, 0.05, 0.2, 0.0, 0.1),
    credit.zmijewskiScore(0.05, 0.6, 1.5),
  ],
  recovery_waterfall: () => credit.allocateRecovery(120.0, CLAIMS),
  'migration.scale': () => [
    credit.RatingScale.standard().nStates,
    credit.RatingScale.notched().nStates,
    credit.RatingScale.standardWithNr().labels(),
    credit.RatingScale.standard().indexOf('BBB'),
    credit.RatingScale.standard().warf('B'),
    credit.RatingScale.standard().ratingFromWarf(610.0),
    scale().defaultState(),
  ],
  'migration.transition_matrix': () => {
    const matrix = () => new credit.TransitionMatrix(scale(), TRANSITIONS, 1.0);
    return [
      matrix().probability('A', 'D'),
      matrix().probabilityByIndex(1, 2),
      list(matrix().row('B')),
      matrix().compose(matrix()).toMatrix(),
      list(matrix().defaultProbabilities()),
    ];
  },
  'migration.generator': () => [
    generator().exitRate('A'),
    generator().intensity('A', 'B'),
    credit.project(generator(), 2.0).toMatrix(),
    credit.GeneratorMatrix.fromTransitionMatrix(
      new credit.TransitionMatrix(scale(), TRANSITIONS, 1.0)
    ).toMatrix(),
  ],
  'migration.simulate': () => {
    const simulator = new credit.MigrationSimulator(generator(), 5.0);
    const batch = simulator.simulate(0, 50, 7);
    return [
      batch.defaultRate,
      batch.length,
      batch.paths[0].transitions(),
      batch.paths[3].labelAt(5.0),
      simulator.empiricalMatrix(200, 7).toMatrix(),
    ];
  },

  'correlation.bernoulli': () => {
    const pair = new correlation.CorrelatedBernoulli(0.1, 0.2, 0.3);
    return [
      list(pair.jointProbabilities()),
      pair.conditionalP2GivenX1(),
      pair.conditionalP1GivenX2(),
      list(pair.sampleFromUniform(0.5)),
      pair.correlation,
    ];
  },
  'correlation.latent': () => {
    const kind = () => correlation.LatentFactorSpec.twoFactor(0.2, 0.25, -0.3).build();
    return [
      list(kind().correlationMatrix),
      list(kind().volatilities),
      kind().diagonalFactorContribution(1, 0.5),
      correlation.LatentFactorSpec.singleFactor(0.25, 0.1).build().modelName,
      correlation.LatentTwoFactor.rmbsStandard().choleskyL11,
      correlation.LatentTwoFactor.cloStandard().correlation,
      new correlation.LatentTwoFactor(0.2, 0.25, -0.3).choleskyL10,
      new correlation.LatentSingleFactor(0.25, 0.1).volatility,
      list(
        new correlation.LatentMultiFactor(
          2,
          [0.2, 0.3],
          [1.0, 0.5, 0.5, 1.0]
        ).generateCorrelatedFactors([1.0, -1.0])
      ),
      list(correlation.LatentMultiFactor.uncorrelated(2, [0.2, 0.3]).correlationMatrix),
    ];
  },
  'correlation.cholesky_decompose': () =>
    list(correlation.choleskyDecompose([1.0, 0.5, 0.5, 1.0], 2)),
  'correlation.simulate_portfolio_loss': () => {
    const config = {
      num_paths: 2000,
      seed: 42,
      confidence: 0.99,
      copula: json(correlation.CopulaSpec.gaussian()),
    };
    const loss = (result) => [result.expectedLoss, result.var, result.expectedShortfall];
    return [
      loss(correlation.simulatePortfolioLoss(EXPOSURES, config)),
      loss(
        correlation.simulatePortfolioLoss(
          EXPOSURES,
          config,
          correlation.RecoverySpec.marketCorrelated(0.4, 0.25, 0.4).toJson()
        )
      ),
      correlation.maxPortfolioLossPaths(),
    ];
  },
  'correlation.spec_json': () => [
    json(correlation.CopulaSpec.studentT(5.0)),
    json(correlation.RecoverySpec.marketCorrelated(0.4, 0.25, 0.4)),
  ],

  'factor.vol_horizon': () => [
    factorCredit.VolHorizon.years(0.25).kind,
    factorCredit.VolHorizon.years(0.25).yearsValue,
    factorCredit.VolHorizon.nSteps(5).n,
    factorCredit.VolHorizon.parse('{"n_steps": 3}').kind,
    factorCredit.VolHorizon.oneStep().kind,
    factorCredit.VolHorizon.unconditional().kind,
  ],
  'factor.covariance_matrix': () => {
    const matrix = { factor_ids: ['rates', 'credit'], n: 2, data: [0.04, 0.01, 0.01, 0.09] };
    return [
      factorCredit.factorVariance(matrix, 'rates'),
      factorCredit.factorCovariance(matrix, 'rates', 'credit'),
      factorCredit.factorCorrelation(matrix, 'rates', 'credit'),
      factorCredit.factorCovarianceRows(matrix),
    ];
  },
  'factor.credit_factor_model': () => {
    const model = factorCredit.CreditFactorModel.fromJson(CANONICAL_MODEL);
    return [
      model.asOf,
      model.calibrationWindow,
      model.policy,
      model.panelFrequency,
      model.bucketWeighting,
      model.nLevels,
      model.nIssuers,
      model.nFactors,
      model.levelNames(),
      model.issuerIds(),
      model.factorIds(),
      model.covariance.factor_ids.length,
      model.config.factors.length,
    ];
  },
  'factor.decomposition_config': () => [
    json(factorRisk.DecompositionConfig.parametric(0.975).withIncremental()),
    json(factorRisk.DecompositionConfig.parametric95()),
    json(factorRisk.DecompositionConfig.parametric99()),
    json(factorRisk.DecompositionConfig.historical(0.9)),
    json(factorRisk.DecompositionConfig.historical95()),
    factorRisk.DecompositionConfig.historical95().method,
  ],
  'factor.stress_attribution': () => factorRisk.buildStressAttribution(['A', 'B'], PNLS, 0.8),
  'factor.position_component_var': () => [
    factorRisk.positionComponentVar(
      factorRisk.parametricVarDecomposition(
        ['A', 'B'],
        [1.0, 2.0],
        [
          [0.04, 0.01],
          [0.01, 0.09],
        ]
      ),
      'A'
    ),
    factorRisk.defaultUtilizationThreshold(),
  ],

  'liquidity.almgren_chriss': () => {
    const almgren = new liquidity.AlmgrenChrissModel(1e-7, 1e-6, 1.0);
    const calibrated = liquidity.AlmgrenChrissModel.fromProfile(PROFILE, 0.02);
    return [
      almgren.estimateCost(TRADE),
      almgren.optimalTrajectory(TRADE, 5),
      [calibrated.gamma, calibrated.eta, calibrated.delta],
      almgren.modelName,
    ];
  },
  'liquidity.kyle': () => {
    const kyle = liquidity.KyleLambdaModel.fromAmihud(1e-9, 100.0);
    return [
      kyle.lambda,
      kyle.estimateCost(TRADE),
      kyle.optimalTrajectory(TRADE, 4),
      new liquidity.KyleLambdaModel(2e-7).modelName,
    ];
  },

  'monte_carlo.european': () => {
    const european = new monteCarlo.EuropeanPricer(2000, 42, false);
    return [money(european.priceCall(...GBM)), money(european.pricePut(...GBM, 16, 'EUR'))];
  },
  'monte_carlo.asian': () => {
    const asian = new monteCarlo.PathDependentPricer(2000, 42, false);
    return [money(asian.priceAsianCall(...GBM, 12)), money(asian.priceAsianPut(...GBM, 12))];
  },
  'monte_carlo.lsmc': () => {
    const lsmc = new monteCarlo.LsmcPricer(2000, 42, false, 20);
    return [
      money(lsmc.priceAmericanPut(...GBM)),
      money(lsmc.priceAmericanCall(...GBM, undefined, 10, 'polynomial', 2)),
      money(lsmc.priceAmericanPutUnbiased(...GBM, 99)),
      money(lsmc.priceAmericanCallUnbiased(...GBM, 99)),
      lsmc.basis,
      lsmc.basisDegree,
    ];
  },
  'monte_carlo.finite_diff': () => [
    monteCarlo.finiteDiffDelta(...GBM, true, 2000, 42).mean,
    monteCarlo.finiteDiffDeltaCrn(...GBM, true, 2000, 42).mean,
    monteCarlo.finiteDiffGamma(...GBM, false, 2000, 42).mean,
    monteCarlo.finiteDiffGammaCrn(...GBM, false, 2000, 42).mean,
  ],
  'monte_carlo.simulate_gbm_paths': () =>
    monteCarlo.simulateGbmPaths(100.0, 0.05, 0.0, 0.2, 1.0, 4, 3, 42, false).paths,
  'monte_carlo.heston_satisfies_feller': () => [
    monteCarlo.hestonSatisfiesFeller(2.0, 0.04, 0.3),
    monteCarlo.hestonSatisfiesFeller(0.5, 0.04, 0.5),
  ],
  'monte_carlo.relative_stderr': () => {
    const estimate = new monteCarlo.EuropeanPricer(2000, 42, false).priceCall(...GBM);
    return [
      monteCarlo.relativeStderr(estimate),
      estimate.stderr / Math.abs(Number(estimate.mean.amount)),
    ];
  },

  'hull_white.params': () => {
    const params = hullWhite.HullWhiteParams.piecewise(0.05, [0.0, 2.5], [0.01, 0.012]);
    return [
      params.sigma(3.0),
      params.stateVariance(2.0),
      params.stateCovariance(1.0, 2.0),
      params.bondVol(0.0, 1.0, 5.0),
      new hullWhite.HullWhiteParams(0.05, 0.01).stateVariance(2.0),
    ];
  },
  'hull_white.functions': () => [
    hullWhite.hw1fConvexityAdjustment(0.05, 0.01, 1.0, 1.25),
    hullWhite.hwBondVol(0.05, 0.01, 0.0, 1.0, 5.0),
    hullWhite.hw1fZcbOptionPrice(0.95, 0.8, 0.84, 0.02, true),
    hullWhite.hw1fCapletForwardRateNormalVol(0.05, 0.01, 1.0, 0.25),
    hullWhite.hw1fCapFloorPrice(0.05, 0.01, CAP_PERIODS, 0.03, true, discountCurve(0.03)),
    hullWhite.hw1fCapFloorPrice(
      0.05,
      0.01,
      CAP_PERIODS,
      0.03,
      false,
      discountCurve(0.03),
      discountCurve(0.035)
    ),
  ],
  'dtsm.yield_panel': () => [panel().numDates, panel().numTenors, panel().yieldChanges()],
  'dtsm.diebold_li': () => {
    const model = new dtsm.DieboldLi().fit(panel());
    return [
      model.forecast(1).yields,
      model.loadingMatrix(),
      model.phi,
      list(model.mu),
      model.factors.r_squared_avg,
      dtsm.dieboldLiFitFactors(TENORS, YIELDS).r_squared_avg,
      dtsm.dieboldLiForecast(TENORS, YIELDS, 2).yields,
    ];
  },
  'dtsm.yield_pca': () => {
    const pca = dtsm.YieldPca.fit(panel());
    return [
      list(pca.varianceExplained),
      list(pca.eigenvalues),
      pca.componentsForThreshold(0.9),
      list(pca.scenario([1.0])),
      list(pca.applyScenario([0.03, 0.032, 0.036, 0.04], [1.0, -0.5])),
      dtsm.yieldPcaFit(CHANGES, 2).eigenvalues,
      list(dtsm.yieldPcaScenario(CHANGES, 0, 1.0, 2)),
      list(dtsm.YieldPca.fitYieldChanges(CHANGES).cumulativeVariance),
    ];
  },

  'volatility.svi': () => [
    volatility.sviTotalVariance(SVI, 0.1),
    volatility.sviDurrlemanG(SVI, 0.1),
    volatility.sviImpliedVol(SVI, 0.1, 1.0),
  ],
  'volatility.arbitrage': () => [
    volatility.checkSurfaceGrid(STRIKES, EXPIRIES, VOLS, [100.0, 100.0]),
    volatility.checkButterflyGrid(STRIKES, EXPIRIES, VOLS, [100.0, 100.0]).length,
    volatility.checkCalendarSpreadGrid(STRIKES, EXPIRIES, VOLS, [100.0, 100.0]).length,
    volatility.checkLocalVolDensityGrid(STRIKES, EXPIRIES, VOLS, [100.0, 100.0]).length,
  ],
  'volatility.cube_slices': () => {
    const strikes = [0.02, 0.03, 0.04];
    const tenorSlice = volatility.materializeCubeTenorSlice(cube(), 5.0, strikes);
    return [
      surfaceVols(tenorSlice),
      surfaceVols(volatility.materializeCubeTenorSliceNormal(cube(), 5.0, strikes)),
      surfaceVols(volatility.materializeCubeExpirySlice(cube(), 1.0, strikes)),
      surfaceVols(volatility.materializeCubeExpirySliceNormal(cube(), 1.0, strikes)),
      volatility.getSurfaceVol(tenorSlice, 1.5, 0.025),
      volatility.getSurfaceVolClamped(JSON.stringify(tenorSlice), 9.0, 0.5),
    ];
  },
  'volatility.fx_delta_surface': () =>
    surfaceVols(
      volatility.materializeFxDeltaSurface(
        new core.FxDeltaVolSurface(
          'EURUSD',
          [0.5, 1.0],
          [0.1, 0.11],
          [0.01, 0.012],
          [0.002, 0.003]
        ),
        1.1,
        0.03,
        0.02
      )
    ),
  'volatility.sabr_smile': () => {
    const smile = new volatility.SabrSmile(
      new volatility.SabrParameters(0.2, 1.0, 0.3, -0.2),
      100.0,
      1.5
    );
    return [smile.forward, smile.t];
  },
  'models.bs_greeks_is_valid': () =>
    models.bsGreeksIsValid(models.bsGreeks(100.0, 100.0, 0.05, 0.0, 0.2, 1.0, true)),

  // Closed-form, COS and SVI kernels (audit finding F128).
  'closed_form.forward_greeks': () => [
    models.black76Greeks(0.03, 0.035, 2.0, 0.25, true),
    models.black76Greeks(0.03, 0.035, 2.0, 0.25, false),
    models.bachelierGreeks(0.03, 0.035, 0.0075, 2.0, true),
    models.bachelierGreeks(-0.002, 0.001, 0.0075, 2.0, false),
    models.blackShiftedVega(-0.002, 0.001, 0.3, 2.0, 0.03),
  ],
  'closed_form.barrier_call': () =>
    [
      [120.0, 'up'],
      [85.0, 'down'],
    ].flatMap(([barrier, direction]) =>
      ['in', 'out'].map((knock) =>
        models.barrierCall(100.0, 100.0, barrier, 0.05, 0.01, 0.2, 1.0, direction, knock)
      )
    ),
  'closed_form.asian': () => [
    models.asianOptionPrice(...GBM, 12),
    models.asianOptionPrice(...GBM, 12, 'geometric', false),
    models.asianOptionPrice(...GBM, 1, 'arithmetic', true),
  ],
  'closed_form.quanto': () => [
    models.quantoOptionPrice(100.0, 105.0, 1.5, 0.04, 0.01, 0.02, 0.25, 0.1, -0.3),
    models.quantoOptionPrice(100.0, 105.0, 1.5, 0.04, 0.01, 0.02, 0.25, 0.1, -0.3, false),
  ],
  'fourier.heston': () => [
    models.hestonPrice(100.0, 100.0, 1.0, 0.05, 0.0, 2.0, 0.04, 0.3, -0.7, 0.04),
    models.hestonPrice(100.0, 110.0, 1.0, 0.05, 0.0, 2.0, 0.04, 0.3, -0.7, 0.04, false),
  ],
  'fourier.cos': () => [
    models.vgCosPrice(100.0, 100.0, 0.05, 0.0, 0.2, -0.14, 0.2, 1.0, true),
    models.vgCosPrice(100.0, 95.0, 0.05, 0.0, 0.2, -0.14, 0.2, 1.0, false, 512),
    models.mertonJumpCosPrice(100.0, 100.0, 0.05, 0.0, 0.2, -0.1, 0.15, 0.5, 1.0, true),
    models.mertonJumpCosPrice(100.0, 95.0, 0.05, 0.0, 0.2, -0.1, 0.15, 0.5, 1.0, false, 512),
  ],
  'volatility.convert_atm': () => [
    volatility.convertAtmVolatility(0.2, 'lognormal', 'normal', 0.03, 2.0),
    volatility.convertAtmVolatility(0.006, 'normal', 'lognormal', 0.03, 2.0),
    volatility.convertAtmVolatility(
      0.2,
      { shifted_lognormal: { shift: 0.02 } },
      'normal',
      0.03,
      2.0
    ),
  ],
  'volatility.calibrate_svi': () =>
    volatility.calibrateSvi(
      STRIKES,
      STRIKES.map((strike) => volatility.sviImpliedVol(SVI, Math.log(strike / 100.0), 1.0)),
      100.0,
      1.0
    ),
};

/** Reduce a result to JSON data (typed arrays become arrays). */
const plain = (value) =>
  JSON.parse(
    JSON.stringify(value, (_key, item) => (ArrayBuffer.isView(item) ? Array.from(item) : item))
  );

function assertClose(actual, expected, where) {
  if (typeof expected === 'number') {
    assert.equal(typeof actual, 'number', where);
    const tolerance = Math.max(ABS_TOL, REL_TOL * Math.max(Math.abs(actual), Math.abs(expected)));
    assert.ok(Math.abs(actual - expected) <= tolerance, `${where}: ${actual} vs ${expected}`);
  } else if (Array.isArray(expected)) {
    assert.ok(Array.isArray(actual), where);
    assert.equal(actual.length, expected.length, where);
    expected.forEach((item, index) => assertClose(actual[index], item, `${where}[${index}]`));
  } else if (expected !== null && typeof expected === 'object') {
    assert.deepEqual(Object.keys(actual).sort(), Object.keys(expected).sort(), where);
    for (const key of Object.keys(expected))
      assertClose(actual[key], expected[key], `${where}.${key}`);
  } else {
    assert.equal(actual, expected, where);
  }
}

test('the golden file lists exactly the facade cases', () => {
  assert.deepEqual(Object.keys(CASES).sort(), Object.keys(GOLDEN).sort());
});

for (const [name, compute] of Object.entries(CASES)) {
  test(`WASM matches the cross-host golden: ${name}`, () => {
    assertClose(plain(compute()), GOLDEN[name], name);
  });
}

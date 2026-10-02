/**
 * Behaviour of the `models` handles and functions bound in parity slice P5
 * that a numeric golden does not show: JSON round trips, error kinds, strict
 * input conversion, builder ownership and optional-argument defaults.
 *
 * The numeric results are pinned across hosts by `models_parity.test.mjs`.
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
const { correlation, credit, liquidity, monteCarlo, volatility } = models;
const factorCredit = models.factor.credit;
const factorRisk = models.factor.risk;
const { dtsm, hullWhite } = models.rates;

const kind = (expected) => (e) => e instanceof Error && e.kind === expected;
const invalidType = (e) => e instanceof TypeError && e.kind === 'invalid_type';
const scale = () => credit.RatingScale.custom(['A', 'B', 'D']);
const generator = () =>
  new credit.GeneratorMatrix(scale(), [-0.1, 0.08, 0.02, 0.1, -0.2, 0.1, 0, 0, 0]);

test('every serde handle round-trips its canonical JSON', () => {
  const handles = [
    [credit.MertonModel, new credit.MertonModel(100, 0.25, 80, 0.05)],
    [credit.MertonBarrierType, credit.MertonBarrierType.firstPassage(0.02)],
    [credit.AssetDynamics, credit.AssetDynamics.jumpDiffusion(0.5, -0.05, 0.1)],
    [credit.DynamicRecoverySpec, credit.DynamicRecoverySpec.linearDecline(0.4, 100, 0.1, 0.05)],
    [credit.EndogenousHazardSpec, credit.EndogenousHazardSpec.exponential(0.1, 1.5, 2)],
    [credit.ToggleExerciseModel, credit.ToggleExerciseModel.stochastic('leverage', -2, 4)],
    [credit.RatingFactorTable, credit.RatingFactorTable.moodysStandard()],
    [credit.BetaRecovery, new credit.BetaRecovery(0.4, 0.2)],
    [credit.WorkoutCosts, new credit.WorkoutCosts(0.05, 0.03)],
    [credit.DownturnLgd, credit.DownturnLgd.stressed(0.15, 0.3, 0.999)],
    [credit.EadCalculator, new credit.EadCalculator(600, 400, 0.75)],
    [credit.MasterScale, credit.MasterScale.spAssumptions()],
    [credit.RatingScale, scale()],
    [credit.TransitionMatrix, credit.project(generator(), 1)],
    [credit.GeneratorMatrix, generator()],
    [credit.MigrationSimulator, new credit.MigrationSimulator(generator(), 5)],
    [correlation.CopulaSpec, correlation.CopulaSpec.studentT(5)],
    [correlation.RecoverySpec, correlation.RecoverySpec.constant(0.4)],
    [factorRisk.DecompositionConfig, factorRisk.DecompositionConfig.historical(0.9)],
    [liquidity.AlmgrenChrissModel, new liquidity.AlmgrenChrissModel(1e-7, 1e-6, 1)],
    [liquidity.KyleLambdaModel, new liquidity.KyleLambdaModel(2e-7)],
    [
      dtsm.YieldPanel,
      new dtsm.YieldPanel(
        [1, 5],
        [
          [0.03, 0.04],
          [0.031, 0.041],
        ]
      ),
    ],
    [dtsm.DieboldLi, new dtsm.DieboldLi(0.6)],
    [hullWhite.HullWhiteParams, hullWhite.HullWhiteParams.piecewise(0.05, [0, 1], [0.01, 0.012])],
  ];
  for (const [cls, handle] of handles) {
    const text = handle.toJson();
    assert.equal(typeof text, 'string', cls.name);
    assert.equal(cls.fromJson(text).toJson(), text, `${cls.name} from text`);
    assert.equal(cls.fromJson(JSON.parse(text)).toJson(), text, `${cls.name} from object`);
    assert.throws(() => cls.fromJson(7), invalidType, cls.name);
  }
});

test('simulation batches round-trip and expose their paths', () => {
  const simulator = new credit.MigrationSimulator(generator(), 5);
  const batch = simulator.simulate(0, 20, 7n);
  assert.equal(batch.length, 20);
  assert.equal(batch.paths.length, 20);
  const copy = credit.RatingPaths.fromJson(batch.toJson());
  assert.equal(copy.defaultRate, batch.defaultRate);
  const path = credit.RatingPath.fromJson(batch.paths[0].toJson());
  assert.equal(path.stateAt(0), 0);
  assert.equal(path.labelAt(0), 'A');
  assert.equal(path.horizon, 5);
  assert.equal(path.scale.nStates, 3);
  assert.equal(path.nTransitions(), path.transitions().length - 1);
  assert.equal(path.defaulted(), path.defaultTime() !== undefined);
  assert.equal(simulator.horizon, 5);
  assert.equal(simulator.generator.nStates, 3);
  // Equal seeds give equal paths; the seed may be a number or a bigint.
  assert.equal(simulator.simulate(0, 20, 7).toJson(), batch.toJson());
});

test('migration lookups report the Rust error kind', () => {
  const matrix = credit.project(generator(), 1);
  assert.throws(() => matrix.probability('A', 'ZZ'), kind('not_found'));
  assert.throws(() => matrix.probabilityByIndex(0, 3), kind('validation'));
  assert.throws(() => matrix.probabilityByIndex(-1, 0), invalidType);
  assert.throws(() => generator().exitRate('ZZ'), kind('not_found'));
  assert.throws(() => scale().indexOfRequired('ZZ'), kind('not_found'));
  assert.equal(scale().indexOf('ZZ'), undefined);
  assert.equal(scale().labelOf(9), undefined);
  assert.equal(scale().labelOf(1), 'B');
  assert.throws(() => credit.RatingScale.custom(['A']), kind('validation'));
  assert.throws(() => credit.RatingScale.custom('A,B'), invalidType);
  assert.throws(() => new credit.MigrationSimulator(generator(), 0), kind('validation'));
  assert.throws(
    () => new credit.MigrationSimulator(generator(), 5).empiricalMatrix(0, 7),
    kind('validation')
  );
  // A transition matrix accepts flat row-major data or an array of rows.
  const flat = new credit.TransitionMatrix(scale(), [0.9, 0.08, 0.02, 0.1, 0.8, 0.1, 0, 0, 1], 1);
  const rows = new credit.TransitionMatrix(scale(), flat.toMatrix(), 1);
  assert.equal(rows.toJson(), flat.toJson());
  assert.throws(
    () =>
      new credit.TransitionMatrix(
        scale(),
        [
          [0.9, 0.1],
          [0.1, 0.8, 0.1],
          [0, 0, 1],
        ],
        1
      ),
    kind('validation')
  );
  assert.equal(matrix.horizon, 1);
  assert.equal(matrix.nStates, 3);
  assert.equal(matrix.scale.labels().join(), 'A,B,D');
  const fitted = credit.GeneratorMatrix.fromTransitionMatrixWithTol(matrix, 1e-6);
  assert.ok(fitted.roundTripError < 1e-6);
  assert.ok(fitted.regularizationL1 >= 0);
  assert.equal(fitted.scale.nStates, 3);
});

test('the workout builder is consumed by each call and validates collateral', () => {
  const builder = credit.WorkoutLgd.builder();
  const next = builder.workoutYears(2);
  assert.throws(() => builder.discountRate(0.05), /null pointer|moved|freed|destroyed/i);
  const workout = next
    .discountRate(0.05)
    .collateral({ collateral_type: 'cash', book_value: 10, haircut: 0 })
    .build();
  assert.equal(workout.workoutYears, 2);
  assert.equal(workout.discountRate, 0.05);
  assert.deepEqual(workout.collateral, [{ collateral_type: 'cash', book_value: 10, haircut: 0 }]);
  assert.ok(workout.costs.totalRate >= 0);
  assert.throws(
    () =>
      credit.WorkoutLgd.builder().collateral({
        collateral_type: 'cash',
        book_value: 10,
        haircut: 1.5,
      }),
    kind('validation')
  );
  assert.throws(
    () =>
      credit.WorkoutLgd.builder().collateral({
        collateral_type: 'gold',
        book_value: 10,
        haircut: 0,
      }),
    kind('validation')
  );
  assert.throws(() => credit.WorkoutLgd.builder().collateralPieces('not json'), kind('validation'));
  assert.throws(() => workout.lgd(-1), kind('validation'));
});

test('LGD, EAD and PD handles expose their accessors and reject bad input', () => {
  const beta = new credit.BetaRecovery(0.4, 0.2);
  assert.equal(beta.mean, 0.4);
  assert.equal(beta.stdDev, 0.2);
  assert.ok(beta.mode === undefined || typeof beta.mode === 'number');
  assert.throws(() => new credit.BetaRecovery(0.4, 2), kind('validation'));
  assert.throws(() => beta.quantile(2), kind('validation'));
  assert.throws(() => beta.sampleSeeded(1.5, 7), invalidType);
  assert.throws(() => credit.seniorityRecoveryStats('nope'), kind('validation'));
  assert.throws(() => credit.seniorityRecoveryStats('senior_secured', 'nope'), kind('validation'));

  const costs = new credit.WorkoutCosts(0.05, 0.03);
  assert.equal(costs.directCostRate, 0.05);
  assert.equal(costs.indirectCostRate, 0.03);
  assert.ok(Math.abs(costs.totalRate - 0.08) < 1e-15);
  assert.equal(credit.WorkoutCosts.zero().totalRate, 0);
  assert.throws(() => new credit.WorkoutCosts(-1, 0), kind('validation'));

  const downturn = credit.DownturnLgd.regulatoryFloor(0.08, 0.1);
  assert.equal(downturn.method, 'regulatory_floor');
  assert.deepEqual(downturn.params, { regulatory_floor: { add_on: 0.08, floor: 0.1 } });
  assert.equal(credit.DownturnLgd.stressed(0.15, 0.3, 0.999).method, 'stressed_approximation');
  assert.throws(() => credit.DownturnLgd.fromRegistryId('nope'), kind('validation'));
  assert.throws(
    () => credit.DownturnLgd.fromJson({ method: { regulatory_floor: { add_on: -1, floor: 0.1 } } }),
    kind('validation')
  );

  assert.equal(new credit.EadCalculator(600, 0, 0.75).leqFromObservedEad(600), undefined);
  assert.throws(() => new credit.EadCalculator(600, 400, 1.5), kind('validation'));
  assert.throws(() => credit.eadRevolver(-1, 0, 0.5), kind('validation'));

  const scaleHandle = credit.MasterScale.moodysAssumptions();
  assert.equal(scaleHandle.grades.length, scaleHandle.nGrades);
  assert.equal(typeof scaleHandle.grades[0].label, 'string');
  assert.throws(() => scaleHandle.mapPd(5), kind('validation'));
  assert.throws(() => scaleHandle.mapPds([0.01, -1]), kind('validation'));
  assert.throws(() => new credit.MasterScale([]), kind('validation'));
  assert.throws(() => credit.MasterScale.fromRegistryId('nope'), kind('validation'));
  // Altman scores carry no implied PD, so they cannot be mapped to a grade.
  assert.throws(
    () => scaleHandle.mapScore(credit.altmanZScore(0.1, 0.2, 0.15, 1.5, 1.1)),
    kind('validation')
  );
  assert.throws(() => credit.pitToTtc(1.5, 0.15, 0), kind('validation'));
  assert.throws(() => credit.centralTendency([]), kind('validation'));
  assert.throws(() => credit.altmanZScore(NaN, 0.2, 0.15, 1.5, 1.1), kind('validation'));
  assert.throws(
    () => credit.ohlsonOScore(5, 0.6, 0.1, 0.8, 0.5, 0.05, 0.2, 0, 0.1),
    kind('validation')
  );
  assert.throws(() => credit.allocateRecovery(-1, []), kind('validation'));
  assert.throws(() => credit.allocateRecovery(100, [{ id: 'x' }]), kind('validation'));
});

test('correlation handles expose their accessors and reject bad input', () => {
  const pair = new correlation.CorrelatedBernoulli(0.1, 0.2, 0.3);
  assert.equal(pair.p1, 0.1);
  assert.equal(pair.p2, 0.2);
  assert.equal(pair.requestedCorrelation, 0.3);
  assert.ok(Math.abs(pair.jointP11 + pair.jointP10 + pair.jointP01 + pair.jointP00 - 1) < 1e-15);
  assert.ok(pair.sampleFromUniform(0.5) instanceof Uint8Array);
  assert.throws(() => pair.sampleFromUniform(2), kind('validation'));
  assert.throws(() => new correlation.CorrelatedBernoulli(2, 0.2, 0.3), kind('validation'));

  const spec = correlation.LatentFactorSpec.singleFactor(0.25, 0.1);
  assert.equal(spec.numFactors, 1);
  assert.deepEqual(spec.build().factorNames.length, 1);
  const two = new correlation.LatentTwoFactor(0.2, 0.25, -0.3);
  assert.equal(two.numFactors, 2);
  assert.equal(two.prepayVol, 0.2);
  assert.equal(two.creditVol, 0.25);
  const single = new correlation.LatentSingleFactor(0.25, 0.1);
  assert.equal(single.meanReversion, 0.1);
  assert.equal(single.numFactors, 1);
  const multi = new correlation.LatentMultiFactor(2, [0.2, 0.3], [1, 0.5, 0.5, 1]);
  assert.equal(multi.numFactors, 2);
  assert.deepEqual(Array.from(multi.volatilities), [0.2, 0.3]);
  assert.throws(() => multi.generateCorrelatedFactors([1]), kind('validation'));
  assert.throws(
    () => new correlation.LatentMultiFactor(2, [0.2], [1, 0.5, 0.5, 1]),
    kind('validation')
  );
  assert.throws(() => correlation.choleskyDecompose([1, 2, 2, 1], 2), kind('validation'));

  const exposures = [
    { id: 'A', notional: 100, default_probability: 0.02, lgd: 0.6, factor_loadings: [0.4] },
  ];
  const config = { num_paths: 100, seed: 1, confidence: 0.95, copula: { type: 'gaussian' } };
  const result = correlation.simulatePortfolioLoss(exposures, config);
  assert.equal(result.losses.length, 100);
  // `null` and an omitted recovery are the same call.
  assert.equal(
    correlation.simulatePortfolioLoss(exposures, config, null).toJson(),
    result.toJson()
  );
  assert.throws(
    () =>
      correlation.simulatePortfolioLoss(exposures, {
        ...config,
        num_paths: correlation.maxPortfolioLossPaths() + 1,
      }),
    kind('validation')
  );
  assert.throws(() => correlation.simulatePortfolioLoss(exposures, 'nope'), kind('validation'));
});

test('factor handles and helpers follow the Rust accessors', () => {
  const horizon = factorCredit.VolHorizon.nSteps(3);
  assert.equal(horizon.toString(), '{"n_steps": 3}');
  assert.equal(horizon.yearsValue, undefined);
  assert.equal(factorCredit.VolHorizon.years(0.5).n, undefined);
  assert.equal(factorCredit.VolHorizon.parse(horizon.toString()).n, 3);
  assert.equal(factorCredit.VolHorizon.oneStep().toString(), 'one_step');
  assert.throws(() => factorCredit.VolHorizon.years(-1), kind('validation'));
  assert.throws(() => factorCredit.VolHorizon.parse('soon'), kind('validation'));
  assert.throws(() => factorCredit.VolHorizon.nSteps(1.5), invalidType);

  const model = factorCredit.CreditFactorModel.fromJson(
    repoFile('finstack-quant/models/tests/data/canonical/credit_factor_model.json')
  );
  assert.equal(model.factorIds().length, model.nFactors);
  assert.equal(model.issuerIds().length, model.nIssuers);
  assert.equal(model.levelNames().length, model.nLevels);
  assert.equal(model.calibrationWindow.length, 2);
  assert.deepEqual(model.covariance, model.config.covariance);
  assert.equal(typeof model.diagnostics, 'object');
  assert.equal(typeof model.staticCorrelation, 'object');
  // The horizon descriptor feeds the forecast methods, and a forecast
  // configuration validates through the free function.
  const forecast = new factorCredit.FactorCovarianceForecast(model);
  const covariance = forecast.covarianceAt(factorCredit.VolHorizon.oneStep().toString());
  assert.deepEqual(covariance.factor_ids, model.factorIds());
  // A unit `RiskMeasure` variant is its bare wire label; the quoted JSON text
  // of the same label is equivalent.
  const oneStep = factorCredit.VolHorizon.oneStep().toString();
  const atVolatility = forecast.factorModelAt(oneStep, 'volatility');
  assert.equal(atVolatility.risk_measure, 'volatility');
  assert.deepEqual(atVolatility, forecast.factorModelAt(oneStep, '"volatility"'));
  assert.throws(() => forecast.factorModelAt(oneStep, 'nope'), kind('validation'));
  const firstFactor = model.factorIds()[0];
  assert.equal(
    factorCredit.factorVariance(covariance, firstFactor),
    factorCredit.factorCovariance(covariance, firstFactor, firstFactor)
  );
  assert.equal(factorCredit.factorVariance(covariance, 'unknown'), 0);
  assert.equal(factorCredit.factorCovarianceRows(covariance).length, model.nFactors);
  assert.equal(factorCredit.validateFactorModelConfig(model.config), undefined);
  assert.throws(() => factorCredit.validateFactorModelConfig({}), kind('validation'));
  assert.equal(typeof new factorCredit.CreditCalibrator().config.policy, 'string');

  const decomposition = factorRisk.parametricVarDecomposition(
    ['A', 'B'],
    [1, 2],
    [
      [0.04, 0.01],
      [0.01, 0.09],
    ]
  );
  assert.throws(() => factorRisk.positionComponentVar(decomposition, 'missing'), kind('not_found'));
  assert.equal(factorRisk.DecompositionConfig.parametric99().confidence, 0.99);
  assert.equal(factorRisk.DecompositionConfig.parametric95().computeIncremental, false);
  assert.throws(
    () => factorRisk.buildStressAttribution(['A', 'B'], [[1, 2, 3]], 0.9),
    kind('validation')
  );
  // An omitted confidence is the Rust historical 95% preset.
  const pnls = [Array.from({ length: 40 }, (_, i) => Math.sin(i))];
  assert.deepEqual(
    factorRisk.buildStressAttribution(['A'], pnls),
    factorRisk.buildStressAttribution(['A'], pnls, 0.95)
  );
});

test('liquidity models validate their inputs', () => {
  const almgren = new liquidity.AlmgrenChrissModel(1e-7, 1e-6, 1);
  assert.equal(almgren.gamma, 1e-7);
  assert.equal(almgren.eta, 1e-6);
  assert.equal(almgren.delta, 1);
  assert.throws(() => new liquidity.AlmgrenChrissModel(-1, 1e-6, 1), kind('validation'));
  assert.throws(() => almgren.estimateCost({ quantity: 1 }), kind('validation'));
  assert.throws(() => almgren.estimateCost(7), invalidType);
  assert.throws(() => new liquidity.KyleLambdaModel(-1), kind('validation'));
  assert.equal(new liquidity.KyleLambdaModel(2e-7).lambda, 2e-7);
});

test('Monte Carlo pricers take their defaults from the Rust registry', () => {
  const european = new monteCarlo.EuropeanPricer();
  assert.ok(european.numPaths > 0);
  assert.equal(typeof european.seed, 'bigint');
  assert.equal(typeof european.useParallel, 'boolean');
  const explicit = new monteCarlo.EuropeanPricer(500, 7n, false);
  assert.equal(explicit.numPaths, 500);
  assert.equal(explicit.seed, 7n);
  // The default currency and step count come from the registry too.
  const call = explicit.priceCall(100, 100, 0.05, 0, 0.2, 1);
  assert.equal(typeof call.mean.currency, 'string');
  assert.equal(call.num_paths, 500);
  assert.equal(
    explicit.priceCall(100, 100, 0.05, 0, 0.2, 1, undefined, 'EUR').mean.currency,
    'EUR'
  );
  assert.throws(() => new monteCarlo.EuropeanPricer(0), kind('validation'));
  assert.throws(() => new monteCarlo.EuropeanPricer('100'), invalidType);
  assert.throws(() => explicit.priceCall(100, 100, 0.05, 0, 0.2, 1, 4, 'ZZZZ'), kind('validation'));

  const asian = new monteCarlo.PathDependentPricer(500, 7, false, true);
  assert.equal(asian.antithetic, true);
  assert.equal(asian.numPaths, 500);
  assert.equal(asian.seed, 7n);
  assert.equal(typeof asian.useSobol, 'boolean');
  assert.equal(typeof asian.useBrownianBridge, 'boolean');
  assert.equal(asian.useParallel, false);

  const lsmc = new monteCarlo.LsmcPricer(500, 7, false, 10, 'polynomial', 3, false);
  assert.equal(lsmc.basis, 'polynomial');
  assert.equal(lsmc.basisDegree, 3);
  assert.equal(lsmc.numPaths, 500);
  assert.equal(lsmc.seed, 7n);
  assert.equal(lsmc.antithetic, false);
  assert.equal(lsmc.useParallel, false);
  assert.throws(() => new monteCarlo.LsmcPricer(500, 7, false, 10, 'nope'), kind('validation'));
  assert.throws(
    () => lsmc.priceAmericanPut(100, 100, 0.05, 0, 0.2, 1, undefined, undefined, 'nope'),
    kind('validation')
  );
  const paths = monteCarlo.simulateGbmPaths(100, 0.05, 0, 0.2, 1, 4, 3);
  assert.equal(paths.paths.length, 3);
  assert.equal(paths.times.length, 5);
  assert.throws(() => monteCarlo.finiteDiffDelta(100, 100, 0.05, 0, 0.2, 1, 'call'), invalidType);
  assert.equal(
    monteCarlo.relativeStderr({ ...call, mean: { ...call.mean, amount: '0' } }),
    Infinity
  );
});

test('rates handles expose their accessors and reject bad input', () => {
  const params = new hullWhite.HullWhiteParams(0.05, 0.01);
  assert.equal(params.kappa, 0.05);
  assert.deepEqual(Array.from(params.values), [0.01]);
  assert.equal(params.times.length, 1);
  assert.equal(params.sigma(10), 0.01);
  assert.throws(() => new hullWhite.HullWhiteParams(-1, 0.01), kind('validation'));
  assert.throws(
    () => hullWhite.HullWhiteParams.piecewise(0.05, [0], [0.01, 0.02]),
    kind('validation')
  );
  // The forward curve defaults to the discount curve, as in Python.
  const curve = new core.DiscountCurve({
    id: 'USD-OIS',
    baseDate: '2025-01-15',
    knots: [0, 1, 1, 0.97, 2, 0.94],
    dayCount: 'act_365f',
  });
  const periods = [
    [0.25, 0.5, 0.25],
    [0.5, 0.75, 0.25],
  ];
  assert.equal(
    hullWhite.hw1fCapFloorPrice(0.05, 0.01, periods, 0.03, true, curve),
    hullWhite.hw1fCapFloorPrice(0.05, 0.01, periods, 0.03, true, curve, curve)
  );
  assert.throws(
    () => hullWhite.hw1fCapFloorPrice(0.05, 0.01, [[0.25, 0.5]], 0.03, true, curve),
    kind('validation')
  );
  assert.throws(() => hullWhite.hw1fZcbOptionPrice(0.95, 0.8, 0.84, 0.02, 1), invalidType);

  const panel = new dtsm.YieldPanel(
    [1, 5],
    [
      [0.03, 0.04],
      [0.031, 0.041],
    ],
    ['2025-01-31', '2025-02-28']
  );
  assert.deepEqual(panel.dates, ['2025-01-31', '2025-02-28']);
  assert.deepEqual(Array.from(panel.tenors), [1, 5]);
  assert.deepEqual(panel.yields, [
    [0.03, 0.04],
    [0.031, 0.041],
  ]);
  const undated = [
    [0.03, 0.04],
    [0.031, 0.041],
  ];
  assert.equal(new dtsm.YieldPanel([1, 5], undated).dates, undefined);
  assert.throws(() => new dtsm.YieldPanel([1, 5], [[0.03, 0.04]]), kind('validation'));
  assert.throws(() => new dtsm.YieldPanel([1, 5], [[0.03], [0.04]]), kind('validation'));
  assert.throws(() => new dtsm.YieldPanel([1, 5], undated, ['soon', 'later']), kind('validation'));
  const unfitted = new dtsm.DieboldLi();
  assert.equal(unfitted.factors, undefined);
  assert.equal(unfitted.phi, undefined);
  assert.equal(unfitted.mu, undefined);
  assert.equal(unfitted.qCov, undefined);
  assert.equal(unfitted.tenors.length, 0);
  assert.throws(() => unfitted.forecast(1), kind('validation'));
  assert.throws(() => unfitted.fitVar(), kind('validation'));
  assert.throws(() => new dtsm.DieboldLi(-1), kind('validation'));
  assert.throws(() => dtsm.YieldPca.fitYieldChanges([[0.001, 0.002]]), kind('validation'));
});

test('volatility helpers take a VolSurface in its canonical wire form', () => {
  const cube = new core.VolCube(
    'CUBE',
    [1, 2],
    [5],
    [0.02, 0.5, -0.2, 0.4, Number.NaN, 0.03, 0.5, -0.1, 0.3, Number.NaN],
    [0.03, 0.035]
  );
  const surface = volatility.materializeCubeTenorSlice(cube, 5, [0.02, 0.03, 0.04]);
  assert.deepEqual(surface.expiries, [1, 2]);
  assert.deepEqual(surface.strikes, [0.02, 0.03, 0.04]);
  assert.equal(surface.vols_row_major.length, 6);
  assert.equal(
    volatility.getSurfaceVol(surface, 1, 0.03),
    volatility.getSurfaceVol(JSON.stringify(surface), 1, 0.03)
  );
  assert.equal(volatility.getSurfaceVol(surface, 1, 0.03), surface.vols_row_major[1]);
  assert.throws(() => volatility.getSurfaceVol(surface, 9, 0.03), kind('validation'));
  assert.equal(
    volatility.getSurfaceVolClamped(surface, 9, 0.03),
    volatility.getSurfaceVol(surface, 2, 0.03)
  );
  assert.throws(() => volatility.getSurfaceVol({ id: 'x' }, 1, 0.03), kind('validation'));
  assert.throws(
    () => volatility.materializeCubeTenorSlice(cube, 5, [0.04, 0.03]),
    kind('validation')
  );

  const strikes = [80, 90, 100, 110, 120];
  const vols = [
    [0.28, 0.24, 0.2, 0.21, 0.23],
    [0.27, 0.235, 0.21, 0.215, 0.23],
  ];
  // An omitted tolerance is the Rust DEFAULT_GRID_TOLERANCE (1e-6).
  assert.deepEqual(
    volatility.checkSurfaceGrid(strikes, [0.5, 1], vols, [100, 100]),
    volatility.checkSurfaceGrid(strikes, [0.5, 1], vols, [100, 100], 1e-6)
  );
  assert.throws(
    () => volatility.checkButterflyGrid(strikes, [0.5, 1], vols, [100, 100], -1),
    kind('validation')
  );
  assert.throws(() => volatility.checkSurfaceGrid(strikes, [0.5], vols, [100]), kind('validation'));
  assert.throws(
    () => volatility.sviTotalVariance({ a: 0.04, b: -1, rho: 0, m: 0, sigma: 0.2 }, 0),
    kind('validation')
  );
  assert.equal(
    models.bsGreeksIsValid({ ...models.bsGreeks(100, 100, 0.05, 0, 0.2, 1, true), gamma: -1 }),
    false
  );
});

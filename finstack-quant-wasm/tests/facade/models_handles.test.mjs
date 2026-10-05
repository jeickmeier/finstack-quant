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
  assert.throws(() => credit.DownturnLgd.fromRegistryId('nope'), kind('not_found'));
  assert.throws(
    () => credit.DownturnLgd.fromJson({ method: { regulatory_floor: { add_on: -1, floor: 0.1 } } }),
    kind('validation')
  );

  assert.equal(new credit.EadCalculator(600, 0, 0.75).leqFromObservedEad(600), undefined);
  assert.throws(() => new credit.EadCalculator(600, 400, 1.5), kind('validation'));
  assert.throws(() => new credit.EadCalculator(-1, 0, 0.5), kind('validation'));

  const scaleHandle = credit.MasterScale.moodysAssumptions();
  assert.equal(scaleHandle.grades.length, scaleHandle.nGrades);
  assert.equal(typeof scaleHandle.grades[0].label, 'string');
  assert.throws(() => scaleHandle.mapPd(5), kind('validation'));
  assert.throws(() => scaleHandle.mapPds([0.01, -1]), kind('validation'));
  assert.throws(() => new credit.MasterScale([]), kind('validation'));
  assert.throws(() => credit.MasterScale.fromRegistryId('nope'), kind('not_found'));
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

  const lrmPricer = new monteCarlo.PathDependentPricer(20000, 7, false);
  const gbm = [100, 100, 0.04, 0.01, 0.25, 1];
  const greeks = lrmPricer.priceWithLrmGreeks(...gbm, true, 12);
  assert.deepEqual(Object.keys(greeks).sort(), ['delta', 'price', 'vega']);
  assert.equal(greeks.price.mean.amount, lrmPricer.priceAsianCall(...gbm, 12).mean.amount);
  assert.equal(greeks.delta.num_paths, 20000);
  const bumpDelta =
    (lrmPricer.priceAsianCall(100.5, ...gbm.slice(1), 12).mean.amount -
      lrmPricer.priceAsianCall(99.5, ...gbm.slice(1), 12).mean.amount) /
    1.0;
  assert.ok(Math.abs(greeks.delta.mean - bumpDelta) < 5 * greeks.delta.stderr);
  assert.ok(greeks.vega.stderr > 0);
  assert.deepEqual(JSON.parse(JSON.stringify(greeks)), greeks);
  assert.equal(lrmPricer.priceWithLrmGreeks(...gbm, false, 12, 'EUR').price.mean.currency, 'EUR');
  // The default step count keeps a default pricer under the path-capture cap.
  assert.equal(
    new monteCarlo.PathDependentPricer().priceWithLrmGreeks(...gbm, true).price.num_paths,
    100000
  );
  assert.throws(
    () => new monteCarlo.PathDependentPricer().priceWithLrmGreeks(...gbm, true, 252),
    (e) => kind('validation')(e) && /must not exceed 4000000/.test(e.message)
  );
  assert.throws(() => lrmPricer.priceWithLrmGreeks(...gbm, 'call', 12), invalidType);
  assert.throws(() => lrmPricer.priceWithLrmGreeks(...gbm, true, 1.5), invalidType);
  assert.throws(() => lrmPricer.priceWithLrmGreeks(0, ...gbm.slice(1), true, 12), kind('validation'));
  assert.throws(() => asian.priceWithLrmGreeks(...gbm, true, 12), kind('validation'));

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
  const spec = {
    process: { type: 'gbm', r: 0.05, q: 0, sigma: 0.2 },
    initial_state: [100],
    time_grid: { type: 'uniform', expiry: 1, num_steps: 4 },
    num_paths: 3,
    seed: 42,
  };
  const paths = monteCarlo.simulatePaths(spec);
  assert.equal(paths.num_simulated_paths, 3);
  assert.equal(paths.times.length, 5);
  assert.deepEqual(paths.factor_names, ['spot']);
  assert.equal(paths.values.length, 15);
  assert.deepEqual(monteCarlo.simulatePaths(JSON.stringify(spec)), paths);
  assert.throws(() => monteCarlo.simulatePaths(42), invalidType);
  assert.throws(() => monteCarlo.simulatePaths({ ...spec, paths: 3 }), kind('validation'));
  assert.throws(
    () =>
      monteCarlo.simulatePaths({
        ...spec,
        scheme: 'milstein',
        process: { type: 'cir', kappa: 0.5, theta: 0.04, sigma: 0.1 },
        initial_state: [0.03],
      }),
    kind('validation')
  );
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
  // An omitted tolerance is the Rust DEFAULT_ARBITRAGE_TOLERANCE (1e-10), the
  // same default as every Rust arbitrage check.
  assert.deepEqual(
    volatility.checkSurfaceGrid(strikes, [0.5, 1], vols, [100, 100]),
    volatility.checkSurfaceGrid(strikes, [0.5, 1], vols, [100, 100], 1e-10)
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

test('LGD replay and uncorrelated factors use checked Rust construction', () => {
  assert.throws(() =>
    credit.WorkoutCosts.fromJson({ direct_cost_rate: -1, indirect_cost_rate: 0 })
  );
  const model = credit.WorkoutLgd.builder().build();
  const wire = JSON.parse(model.toJson());
  wire.costs.direct_cost_rate = -1;
  assert.throws(() => credit.WorkoutLgd.fromJson(wire));
  for (const vols of [[0.2], [0.2, -0.1], [0.2, NaN]]) {
    assert.throws(() => correlation.LatentMultiFactor.uncorrelated(2, vols));
  }
});

test('local volatility is a plain object with free-function twins', () => {
  const smile = [0.24, 0.22, 0.2, 0.19, 0.185];
  const implied = (rows) => ({
    id: 'IMPLIED',
    expiries: [0.25, 0.5, 1.0, 2.0],
    strikes: [80, 90, 100, 110, 120],
    vols_row_major: rows.flat(),
    secondary_axis: 'strike',
    interpolation_mode: 'vol',
    quote_type: 'black_lognormal',
  });
  const forwards = [100.5, 101.0, 102.0, 104.0];

  // A flat implied volatility is its own local volatility, under any forwards.
  const flat = volatility.localVolFromImpliedVol(implied(Array(4).fill(Array(5).fill(0.2))), forwards);
  assert.deepEqual(Object.keys(flat).sort(), ['expiries', 'local_vols', 'strikes']);
  assert.deepEqual(flat.expiries, [0.25, 0.5, 1.0, 2.0]);
  assert.equal(flat.local_vols.length, 20);
  assert.ok(flat.local_vols.every((vol) => Math.abs(vol - 0.2) < 1e-12));
  assert.ok(Math.abs(volatility.localVolValue(flat, 0.7, 93) - 0.2) < 1e-12);

  const skew = implied(Array(4).fill(smile));
  const local = volatility.localVolFromImpliedVol(skew, forwards);
  const value = (surface, strike) => volatility.localVolValue(surface, 1.0, strike);
  assert.ok(value(local, 90) - value(local, 110) > 0.22 - 0.19);
  assert.equal(value(local, 100), local.local_vols[2 * 5 + 2]);
  assert.deepEqual(volatility.localVolFromImpliedVol(JSON.stringify(skew), forwards), local);
  assert.equal(volatility.localVolValue(JSON.stringify(local), 1.0, 100), value(local, 100));
  const smoothed = volatility.localVolFromImpliedVolSmoothed(skew, forwards, 10.0);
  assert.ok(value(smoothed, 80) - value(smoothed, 120) < value(local, 80) - value(local, 120));
  assert.deepEqual(volatility.localVolFromImpliedVolSmoothed(skew, forwards, 0), local);

  // Bilinear inside the grid, flat outside it.
  const grid = { expiries: [1, 2], strikes: [100, 200], local_vols: [0.1, 0.2, 0.3, 0.4] };
  assert.ok(Math.abs(volatility.localVolValue(grid, 1.5, 150) - 0.25) < 1e-15);
  assert.equal(volatility.localVolValue(grid, 9, 50), 0.3);

  const message = (pattern) => (e) => kind('validation')(e) && pattern.test(e.message);
  assert.throws(() => volatility.localVolFromImpliedVol(skew, [100]), message(/one forward per expiry/));
  assert.throws(
    () => volatility.localVolFromImpliedVol(skew, [100, 100, -1, 100]),
    message(/finite and positive/)
  );
  assert.throws(
    () => volatility.localVolFromImpliedVolSmoothed(skew, forwards, -1),
    message(/sigma_strikes/)
  );
  const calendar = {
    ...implied([Array(5).fill(0.25), Array(5).fill(0.25), Array(5).fill(0.15)]),
    expiries: [0.5, 1.0, 2.0],
  };
  assert.throws(
    () => volatility.localVolFromImpliedVol(calendar, [100, 100, 100]),
    message(/calendar arbitrage/)
  );
  assert.throws(
    () => volatility.localVolFromImpliedVol({ ...skew, quote_type: 'normal' }, forwards),
    kind('validation')
  );
  assert.throws(() => volatility.localVolFromImpliedVol({ ...skew, extra: 1 }, forwards), kind('validation'));
  assert.throws(() => volatility.localVolFromImpliedVol(42, forwards), invalidType);
  assert.throws(() => volatility.localVolFromImpliedVol(skew, 'forwards'), invalidType);
  assert.throws(() => volatility.localVolFromImpliedVolSmoothed(skew, forwards, '5'), invalidType);
  assert.throws(() => volatility.localVolValue(grid, '1', 100), invalidType);
  assert.throws(() => volatility.localVolValue({ ...grid, local_vols: [0.1] }, 1, 100), kind('validation'));
  assert.throws(() => volatility.localVolValue({ ...grid, extra: 1 }, 1, 100), kind('validation'));

  // The same object drives the `local_vol` path process.
  const paths = monteCarlo.simulatePaths({
    process: { type: 'local_vol', r: 0.03, q: 0.01, surface: grid },
    initial_state: [100],
    time_grid: { type: 'uniform', expiry: 1.0, num_steps: 4 },
    num_paths: 3,
    seed: 5,
  });
  assert.deepEqual(paths.factor_names, ['spot']);
  assert.ok(paths.values.every((spot) => spot > 0));
  assert.throws(
    () =>
      monteCarlo.simulatePaths({
        process: { type: 'local_vol', r: 0.03, q: 0.01, surface: grid },
        scheme: 'milstein',
        initial_state: [100],
        time_grid: { type: 'uniform', expiry: 1.0, num_steps: 4 },
        num_paths: 3,
        seed: 5,
      }),
    message(/scheme 'milstein' is not available for process 'local_vol'/)
  );
});

/**
 * Merton structural-credit facade tests for `models.credit`.
 *
 * Covers the typed handles (`MertonModel`, the PIK specifications and the
 * toggle rule), the measure split (risk-neutral versus KMV/EDF), the three
 * spread conventions, and the day-count argument on the hazard-curve export.
 *
 * Requires the wasm-pack web build: npm run build (mise run wasm-build).
 */

import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const PKG_DIR = join(__dirname, '..', '..', 'pkg');
const WASM_BG = join(PKG_DIR, 'finstack_quant_wasm_bg.wasm');

if (!existsSync(WASM_BG)) {
  throw new Error(
    `finstack-quant-wasm web build not found at ${WASM_BG}. Generate it with: npm run build`
  );
}

const facade = await import('../../index.js');
const init = facade.default;
const { models, valuations } = facade;

await init({ module_or_path: readFileSync(WASM_BG) });

const credit = models.credit;
const { core } = facade;
const model = new credit.MertonModel(100.0, 0.25, 80.0, 0.05);

test('models owns all reusable model namespaces exclusively', () => {
  assert.ok(models.monteCarlo);
  assert.ok(models.credit);
  assert.ok(models.correlation);
  assert.ok(models.factor.credit);
  assert.equal('monte_carlo' in facade, false);
  assert.equal('factor_model' in facade, false);
  assert.equal('credit' in valuations, false);
  assert.equal('correlation' in valuations, false);
  assert.equal('bsPrice' in valuations, false);
});

test('the JSON-string Merton surface is gone: MertonModel is a typed handle', () => {
  for (const removed of [
    'mertonModelJson',
    'mertonDefaultProbability',
    'mertonToHazardCurveJson',
    'mertonSimulatePathsJson',
    'dynamicRecoveryAtNotional',
    'creditStateJson',
    'toggleExerciseThresholdJson',
  ]) {
    assert.equal(removed in credit, false, removed);
  }
  assert.equal(model.assetValue, 100.0);
  assert.equal(model.assetVol, 0.25);
  assert.equal(model.debtBarrier, 80.0);
  assert.equal(model.riskFreeRate, 0.05);
  assert.equal(model.payoutRate, 0.0);
  assert.equal(model.barrierType.toJson(), '"terminal"');
  assert.equal(model.dynamics.toJson(), '"geometric_brownian"');
  const copy = credit.MertonModel.fromJson(model.toJson());
  assert.equal(copy.defaultProbability(1.0), model.defaultProbability(1.0));
  assert.deepEqual(Array.from(model.defaultProbabilities([1.0, 5.0])), [
    model.defaultProbability(1.0),
    model.defaultProbability(5.0),
  ]);
});

test('physical-measure default probability sits below the risk-neutral one', () => {
  const riskNeutralDd = model.distanceToDefault(1.0);
  assert.ok(Math.abs(model.distanceToDefaultWithDrift(0.05, 1.0) - riskNeutralDd) < 1e-12);
  assert.ok(model.distanceToDefaultWithDrift(0.12, 1.0) > riskNeutralDd);
  assert.ok(model.defaultProbabilityWithDrift(0.12, 1.0) < model.defaultProbability(1.0));
});

test('kmv default point is short-term debt plus half of long-term debt', () => {
  assert.equal(credit.MertonModel.kmvDefaultPoint(40.0, 60.0), 70.0);
  assert.throws(() => credit.MertonModel.kmvDefaultPoint(-1.0, 60.0));
});

test('the three spread conventions are ordered as their assumptions imply', () => {
  const zeroCoupon = model.impliedSpread(5.0, 0.4);
  const endogenous = model.debtSpread(5.0);
  const parSpread = model.cdsParSpread(5.0, 0.4);
  assert.ok(endogenous > 0 && endogenous < zeroCoupon);
  assert.ok(parSpread > zeroCoupon);
});

test('cds calibration round-trips a par spread quote', () => {
  const quoteBp = model.cdsParSpread(5.0, 0.4) * 10_000;
  const calibrated = credit.MertonModel.fromCdsSpread(quoteBp, 0.4, 80.0, 0.05, 5.0, 100.0, 0.0);
  assert.ok(Math.abs(calibrated.assetVol - 0.25) < 1e-6);
});

test('target-pd calibration honours the payout rate', () => {
  const withoutPayout = credit.MertonModel.fromTargetPd(100.0, 0.25, 0.05, 0.0, 0.05, 1.0);
  const withPayout = credit.MertonModel.fromTargetPd(100.0, 0.25, 0.05, 0.03, 0.05, 1.0);
  assert.ok(withPayout.debtBarrier < withoutPayout.debtBarrier);
  assert.ok(Math.abs(withPayout.defaultProbability(1.0) - 0.05) < 1e-4);
});

test('equity calibration and the CreditGrades and explicit-dynamics factories build models', () => {
  const [equity, equityVol] = model.tryImpliedEquity(1.0);
  const calibrated = credit.MertonModel.fromEquity(equity, equityVol, 80.0, 0.05, 0.0, 1.0);
  assert.ok(Math.abs(calibrated.assetValue - 100.0) < 1e-6);
  const grades = credit.MertonModel.creditGrades(40.0, 0.4, 60.0, 0.03, 0.3, 0.5);
  assert.equal(JSON.parse(grades.dynamics.toJson()).credit_grades.mean_recovery, 0.5);
  const firstPassage = credit.MertonModel.newWithDynamics(
    100.0,
    0.25,
    80.0,
    0.05,
    0.01,
    credit.MertonBarrierType.firstPassage(0.02),
    credit.AssetDynamics.geometricBrownian()
  );
  assert.ok(firstPassage.defaultProbability(1.0) > model.defaultProbability(1.0));
  assert.throws(
    () =>
      credit.MertonModel.newWithDynamics(
        100.0,
        0.25,
        80.0,
        0.05,
        0.0,
        credit.MertonBarrierType.firstPassage(0.0),
        credit.AssetDynamics.jumpDiffusion(0.5, -0.05, 0.1)
      ),
    (e) => e.kind === 'validation'
  );
  assert.equal(
    credit.AssetDynamics.fromJson(credit.AssetDynamics.creditGrades(0.3, 0.5).toJson()).toJson(),
    '{"credit_grades":{"barrier_uncertainty":0.3,"mean_recovery":0.5}}'
  );
  assert.equal(
    credit.MertonBarrierType.fromJson('"terminal"').toJson(),
    credit.MertonBarrierType.terminal().toJson()
  );
});

test('hazard-curve export returns a core.HazardCurve with the requested day count', () => {
  const curve = model.toHazardCurve('ACME-HZD', '2024-01-15', [1.0, 3.0, 5.0], 0.4, 'act_360');
  assert.ok(curve instanceof core.HazardCurve);
  assert.equal(curve.id, 'ACME-HZD');
  assert.equal(JSON.parse(curve.toJson()).day_count, 'act_360');
  assert.ok(Math.abs(1 - curve.sp(5.0) - model.defaultProbability(5.0)) < 1e-9);
  assert.throws(
    () => model.toHazardCurve('ACME-HZD', '2024-01-15', [1.0], 0.4, 'nope'),
    (e) => e.kind === 'validation'
  );
});

// Same cases as finstack-quant-py/tests/test_models_credit_rust_owned.py.
const SEEDED_FIRST_PATH = [
  100.0, 106.3619193858413, 173.31551865077458, 179.24766644780442, 171.48303098644598,
];

test('Merton path simulation is seeded in Rust (PCG64), like Python', () => {
  const paths = model.simulatePaths(2, 4, 1.0, 7, true);
  assert.equal(paths.numPaths, 2);
  assert.equal(paths.numSteps, 4);
  assert.equal(paths.valuesPerPath, 5);
  assert.equal(paths.times.length, 5);
  const firstPath = paths.path(0);
  for (let i = 0; i < 5; i += 1) {
    assert.ok(Math.abs(firstPath[i] / SEEDED_FIRST_PATH[i] - 1) < 1e-12, `step ${i}`);
  }
  assert.equal(paths.get(0, 1), firstPath[1]);
  assert.equal(paths.get(5, 0), undefined);
  assert.equal(paths.path(5), undefined);
  assert.deepEqual(paths.toNested()[0], Array.from(firstPath));
  assert.equal(paths.assetValues.length, 10);
  const copy = credit.SimulatedPaths.fromJson(paths.toJson());
  assert.deepEqual(Array.from(copy.assetValues), Array.from(paths.assetValues));
});

test('toggle label errors carry the Rust list of accepted labels', () => {
  assert.throws(
    () => credit.ToggleExerciseModel.threshold('bogus', 1.0, 'above'),
    (e) =>
      e.kind === 'validation' &&
      e.message.endsWith(
        'unknown credit state variable: bogus (expected one of hazard_rate, distance_to_default, leverage)'
      )
  );
  assert.throws(
    () => credit.ToggleExerciseModel.threshold('leverage', 1.0, 'sideways'),
    (e) =>
      e.kind === 'validation' &&
      e.message.endsWith('unknown threshold direction: sideways (expected one of above, below)')
  );
});

const state = (leverage) => ({
  hazard_rate: 0.05,
  distance_to_default: null,
  leverage,
  accreted_notional: 100,
  coupon_due: 2,
  asset_value: null,
});

test('toggle rules decide PIK from a credit state (F138: the state has a consumer)', () => {
  const threshold = credit.ToggleExerciseModel.threshold('leverage', 0.7, 'above');
  assert.equal(threshold.kind, 'threshold');
  assert.deepEqual(threshold.params, {
    state_variable: 'leverage',
    threshold: 0.7,
    direction: 'above',
  });
  assert.equal(threshold.shouldPikWithUniform(state(0.8), 0.5), true);
  assert.equal(threshold.shouldPikWithUniform(state(0.6), 0.5), false);
  assert.equal(threshold.shouldPikWithUniform(JSON.stringify(state(0.8)), 0.5), true);

  const stochastic = credit.ToggleExerciseModel.stochastic('leverage', -2.0, 4.0);
  assert.equal(stochastic.kind, 'stochastic');
  // logistic(-2 + 4 * 0.8) = logistic(1.2) = 0.7685...
  assert.equal(stochastic.shouldPikWithUniform(state(0.8), 0.5), true);
  assert.equal(stochastic.shouldPikWithUniform(state(0.8), 0.9), false);

  const optimal = credit.ToggleExerciseModel.optimal(100, 0.1, 0.2, 0.03, 1);
  assert.equal(optimal.kind, 'optimal_exercise');
  // Optimal exercise runs a nested simulation seeded from `u`: deterministic per draw.
  assert.equal(
    optimal.shouldPikWithUniform(state(0.8), 0.5),
    optimal.shouldPikWithUniform(state(0.8), 0.5)
  );
  assert.equal(typeof optimal.shouldPikWithUniform(state(0.8), 0.5), 'boolean');
  assert.equal(
    credit.ToggleExerciseModel.fromJson(threshold.toJson()).toJson(),
    threshold.toJson()
  );
  assert.throws(
    () => threshold.shouldPikWithUniform({ ...state(0.8), leverage: NaN }, 0.5),
    (e) => e instanceof TypeError
  );
});

test('credit spec factories reject NaN instead of writing null', () => {
  for (const call of [
    () => credit.ToggleExerciseModel.threshold('leverage', NaN, 'above'),
    () => credit.ToggleExerciseModel.stochastic('leverage', NaN, 1),
    () => credit.ToggleExerciseModel.optimal(100, NaN, 0.2, 0.03, 1),
    () => credit.EndogenousHazardSpec.powerLaw(0.05, 1.5, NaN),
  ]) {
    assert.throws(call, (e) => e.kind === 'validation');
  }
});

test('every dynamic-recovery factory is bound and validates (F149)', () => {
  const R = credit.DynamicRecoverySpec;
  assert.equal(R.constant(0.4).kind, 'constant');
  assert.equal(R.constant(0.4).recoveryAtNotional(120), 0.4);
  const inverse = R.inverseLinear(0.4, 100);
  assert.equal(inverse.kind, 'inverse_linear');
  assert.ok(Math.abs(inverse.recoveryAtNotional(200) - 0.2) < 1e-15);
  assert.equal(inverse.baseRecovery, 0.4);
  assert.equal(inverse.baseNotional, 100);
  assert.equal(inverse.model, 'inverse_linear');
  const power = R.inversePower(0.4, 100, 0.5);
  assert.deepEqual(power.model, { inverse_power: { exponent: 0.5 } });
  assert.ok(Math.abs(power.recoveryAtNotional(400) - 0.2) < 1e-15);
  assert.equal(R.flooredInverse(0.4, 100, 0.25).recoveryAtNotional(400), 0.25);
  assert.equal(R.linearDecline(0.4, 100, 0.1, 0.05).kind, 'linear_decline');
  assert.throws(
    () => R.constant(1.5),
    (e) => e.kind === 'validation'
  );
  for (const json of [
    '{"base_recovery":1.5,"base_notional":-10,"model":"constant"}',
    '{"base_recovery":-0.5,"base_notional":100,"model":"inverse_linear"}',
  ]) {
    assert.throws(
      () => R.fromJson(json),
      (e) => e.kind === 'validation'
    );
  }
  assert.equal(R.fromJson(power.toJson()).toJson(), power.toJson());
});

test('every endogenous-hazard factory is bound and validates (F149)', () => {
  const H = credit.EndogenousHazardSpec;
  const power = H.powerLaw(0.1, 1.5, 2.5);
  assert.equal(power.kind, 'power_law');
  assert.equal(power.baseHazardRate, 0.1);
  assert.equal(power.baseLeverage, 1.5);
  assert.ok(Math.abs(power.hazardAtLeverage(1.5) - 0.1) < 1e-15);
  assert.equal(power.hazardAfterPikAccrual(120, 80), power.hazardAtLeverage(1.5));
  const exponential = H.exponential(0.1, 1.5, 2.0);
  assert.equal(exponential.kind, 'exponential');
  assert.ok(Math.abs(exponential.hazardAtLeverage(2.0) - 0.1 * Math.exp(1.0)) < 1e-15);
  const tabular = H.tabular([1.0, 2.0], [0.05, 0.15]);
  assert.equal(tabular.kind, 'tabular');
  assert.ok(Math.abs(tabular.hazardAtLeverage(1.5) - 0.1) < 1e-15);
  assert.deepEqual(tabular.leverageHazardMap, {
    tabular: { leverage_points: [1.0, 2.0], hazard_points: [0.05, 0.15] },
  });
  assert.throws(
    () => H.tabular([], []),
    (e) => e.kind === 'validation'
  );
  assert.throws(
    () =>
      H.fromJson(
        '{"base_hazard_rate":0.05,"base_leverage":1.5,"leverage_hazard_map":{"tabular":{"leverage_points":[],"hazard_points":[]}}}'
      ),
    (e) => e.kind === 'validation'
  );
});

test('rating-factor tables come from the Rust registry', () => {
  const table = credit.RatingFactorTable.moodysStandard();
  assert.equal(typeof table.agency, 'string');
  assert.equal(typeof table.methodology, 'string');
  assert.equal(table.getFactor('B2'), credit.moodysWarfFactor('B2'));
  assert.ok(table.defaultFactor > 0);
  assert.equal(
    credit.RatingFactorTable.fromJson(table.toJson()).getFactor('B2'),
    table.getFactor('B2')
  );
  assert.throws(
    () => credit.RatingFactorTable.fromRegistryId('nope'),
    (e) => e.kind === 'validation'
  );
  assert.throws(
    () => credit.moodysWarfFactor('not-a-rating'),
    (e) => e.kind === 'validation'
  );
  assert.throws(
    () => credit.moodysWarfFactor(7),
    (e) => e instanceof TypeError
  );
});

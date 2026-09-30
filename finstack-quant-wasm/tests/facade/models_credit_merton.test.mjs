/**
 * Merton structural-credit facade tests for `models.credit`.
 *
 * Covers the measure split (risk-neutral versus KMV/EDF), the three spread
 * conventions, and the day-count argument on the hazard-curve export.
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
const modelJson = credit.mertonModelJson(100.0, 0.25, 80.0, 0.05);

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

test('physical-measure default probability sits below the risk-neutral one', () => {
  const riskNeutralDd = credit.mertonDistanceToDefault(modelJson, 1.0);
  assert.ok(
    Math.abs(credit.mertonDistanceToDefaultWithDrift(modelJson, 0.05, 1.0) - riskNeutralDd) < 1e-12
  );
  assert.ok(credit.mertonDistanceToDefaultWithDrift(modelJson, 0.12, 1.0) > riskNeutralDd);
  assert.ok(
    credit.mertonDefaultProbabilityWithDrift(modelJson, 0.12, 1.0) <
      credit.mertonDefaultProbability(modelJson, 1.0)
  );
});

test('kmv default point is short-term debt plus half of long-term debt', () => {
  assert.equal(credit.mertonKmvDefaultPoint(40.0, 60.0), 70.0);
  assert.throws(() => credit.mertonKmvDefaultPoint(-1.0, 60.0));
});

test('the three spread conventions are ordered as their assumptions imply', () => {
  const zeroCoupon = credit.mertonImpliedSpread(modelJson, 5.0, 0.4);
  const endogenous = credit.mertonDebtSpread(modelJson, 5.0);
  const parSpread = credit.mertonCdsParSpread(modelJson, 5.0, 0.4);
  assert.ok(endogenous > 0 && endogenous < zeroCoupon);
  assert.ok(parSpread > zeroCoupon);
});

test('cds calibration round-trips a par spread quote', () => {
  const quoteBp = credit.mertonCdsParSpread(modelJson, 5.0, 0.4) * 10_000;
  const calibrated = credit.mertonFromCdsSpreadJson(quoteBp, 0.4, 80.0, 0.05, 5.0, 100.0, 0.0);
  assert.ok(Math.abs(JSON.parse(calibrated).asset_vol - 0.25) < 1e-6);
});

test('target-pd calibration honours the payout rate', () => {
  const withoutPayout = credit.mertonFromTargetPdJson(100.0, 0.25, 0.05, 0.0, 0.05, 1.0);
  const withPayout = credit.mertonFromTargetPdJson(100.0, 0.25, 0.05, 0.03, 0.05, 1.0);
  assert.ok(JSON.parse(withPayout).debt_barrier < JSON.parse(withoutPayout).debt_barrier);
  assert.ok(Math.abs(credit.mertonDefaultProbability(withPayout, 1.0) - 0.05) < 1e-4);
});

test('hazard-curve export carries the requested day count', () => {
  const curveJson = credit.mertonToHazardCurveJson(
    modelJson,
    'ACME-HZD',
    '2024-01-15',
    [1.0, 3.0, 5.0],
    0.4,
    'act_360'
  );
  const curve = JSON.parse(curveJson);
  assert.equal(curve.id, 'ACME-HZD');
  assert.equal(curve.day_count, 'act_360');
  assert.throws(() =>
    credit.mertonToHazardCurveJson(modelJson, 'ACME-HZD', '2024-01-15', [1.0], 0.4, 'nope')
  );
});

// Same cases as finstack-quant-py/tests/test_models_credit_rust_owned.py.
const SEEDED_FIRST_PATH = [
  100.0, 106.3619193858413, 173.31551865077458, 179.24766644780442, 171.48303098644598,
];

test('Merton path simulation is seeded in Rust (PCG64), like Python', () => {
  const paths = JSON.parse(credit.mertonSimulatePathsJson(modelJson, 2, 4, 1.0, 7, true));
  const firstPath = paths.asset_values.slice(0, 5);
  for (let i = 0; i < 5; i += 1) {
    assert.ok(Math.abs(firstPath[i] / SEEDED_FIRST_PATH[i] - 1) < 1e-12, `step ${i}`);
  }
});

test('toggle label errors carry the Rust list of accepted labels', () => {
  assert.throws(
    () => credit.toggleExerciseThresholdJson('bogus', 1.0, 'above'),
    (e) =>
      e.kind === 'validation' &&
      e.message.endsWith(
        'unknown credit state variable: bogus (expected one of hazard_rate, distance_to_default, leverage)'
      )
  );
  assert.throws(
    () => credit.toggleExerciseThresholdJson('leverage', 1.0, 'sideways'),
    (e) =>
      e.kind === 'validation' &&
      e.message.endsWith('unknown threshold direction: sideways (expected one of above, below)')
  );
});

test('credit spec builders reject NaN instead of writing null', () => {
  for (const call of [
    () => credit.creditStateJson(NaN, null, 0, 0, 0, null),
    () => credit.toggleExerciseThresholdJson('leverage', NaN, 'above'),
    () => credit.toggleExerciseOptimalJson(100, NaN, 0.2, 0.03, 1),
    () => credit.endogenousHazardPowerLawJson(0.05, 1.5, NaN),
  ]) {
    assert.throws(call, (e) => e.kind === 'validation');
  }
  const state = JSON.parse(credit.creditStateJson(0.05, null, 0.5, 100, 2, 200));
  assert.equal(state.hazard_rate, 0.05);
});

test('spec JSON is validated by the Rust constructors on the way in', () => {
  for (const spec of [
    '{"base_recovery":1.5,"base_notional":-10,"model":"constant"}',
    '{"base_recovery":-0.5,"base_notional":100,"model":"inverse_linear"}',
  ]) {
    assert.throws(
      () => credit.dynamicRecoveryAtNotional(spec, 120),
      (e) => e.kind === 'validation'
    );
  }
  assert.throws(
    () =>
      credit.endogenousHazardAtLeverage(
        '{"base_hazard_rate":0.05,"base_leverage":1.5,"leverage_hazard_map":{"tabular":{"leverage_points":[],"hazard_points":[]}}}',
        2
      ),
    (e) => e.kind === 'validation'
  );
  const constant = credit.dynamicRecoveryConstantJson(0.4);
  assert.equal(credit.dynamicRecoveryAtNotional(constant, 120), 0.4);
});

/**
 * Public-facade tests for structured calibration errors.
 *
 * Requires the wasm-pack web build: npm run build
 */

import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const PKG_DIR = join(__dirname, '..', '..', 'pkg');
const WASM_BG = join(PKG_DIR, 'finstack_quant_wasm_bg.wasm');
const EQUITY_VOL_EXAMPLE = join(
  __dirname,
  '..',
  '..',
  '..',
  'finstack-quant',
  'calibration',
  'examples',
  'market_bootstrap',
  '08_equity_vol_surface.json'
);

if (!existsSync(WASM_BG)) {
  throw new Error(
    `finstack-quant-wasm web build not found at ${WASM_BG}. Generate it with: npm run build`
  );
}

const facade = await import('../../index.js');
const { default: init, calibration, valuations } = facade;
await init({ module_or_path: readFileSync(WASM_BG) });

function captureError(operation) {
  try {
    operation();
  } catch (error) {
    return error;
  }
  assert.fail('operation must throw');
}

function assertStructuredError(error) {
  assert.equal(error.name, 'CalibrationEnvelopeError');
  assert.equal(typeof error.kind, 'string');
  assert.equal(typeof error.stage, 'string');
  assert.ok(error.step_id === undefined || typeof error.step_id === 'string');
  assert.ok(error.solver_diagnostics === undefined || typeof error.solver_diagnostics === 'object');
  assert.equal(typeof error.details, 'string');
  assert.deepEqual(error.cause, JSON.parse(error.details));
}

function swapEnvelope(spread) {
  return {
    schema: 'finstack_quant.calibration/1',
    plan: {
      id: 'swap-spread',
      quote_sets: { quotes: ['USD-SWAP-2Y'] },
      settings: {},
      steps: [
        {
          id: 'USD-OIS',
          kind: 'discount',
          curve_id: 'USD-OIS',
          currency: 'USD',
          base_date: '2026-09-30',
          quote_set: 'quotes',
        },
      ],
    },
    market_data: [
      {
        kind: 'rate_quote',
        type: 'swap',
        id: 'USD-SWAP-2Y',
        index: 'USD-SOFR-OIS',
        pillar: { tenor: { count: 2, unit: 'years' } },
        rate: 0.04,
        spread_decimal: spread,
      },
    ],
  };
}

test('object calibration inputs reject non-finite optional spreads before JSON conversion', () => {
  for (const spread of [NaN, Infinity, -Infinity]) {
    for (const operation of [
      calibration.calibrate,
      calibration.validateCalibrationJson,
      calibration.dryRun,
    ]) {
      assert.throws(() => operation(swapEnvelope(spread)), {
        name: 'TypeError',
        message: 'Calibration input cannot contain non-finite numbers',
      });
    }
  }
});

test('object calibration inputs preserve finite and absent optional spreads', () => {
  for (const spread of [null, 0, 0.001, -0.001]) {
    const envelope = swapEnvelope(spread);
    const canonical = JSON.parse(calibration.validateCalibrationJson(envelope));
    assert.equal(canonical.market_data[0].spread_decimal, spread);
    assert.equal(calibration.calibrate(envelope).result.report.success, true);
  }
});

test('calibration is owned only by the calibration namespace', () => {
  for (const name of [
    'calibrate',
    'calibrateBermudanLmmBaseVol',
    'dryRun',
    'validateCalibrationJson',
  ]) {
    assert.equal(typeof calibration[name], 'function');
    assert.equal(valuations[name], undefined);
  }
});

test('calibrated final market restores through the reusable market handle', () => {
  const result = calibration.calibrate({
    schema: 'finstack_quant.calibration/1',
    plan: { id: 'market-round-trip', quote_sets: {}, steps: [], settings: {} },
  });
  const state = result.result.final_market;
  const market = new valuations.Market(JSON.stringify(state));
  assert.deepEqual(JSON.parse(market.toJson()), state);
});

test('market re-ingestion rejects hierarchy nesting beyond canonical limits', () => {
  const result = calibration.calibrate({
    schema: 'finstack_quant.calibration/1',
    plan: { id: 'market-depth', quote_sets: {}, steps: [], settings: {} },
  });
  const state = result.result.final_market;
  let node = {};
  for (let level = 0; level < 50; level += 1) {
    node = { children: { [`Level${level}`]: node } };
  }
  state.hierarchy = { roots: { Rates: node } };
  assert.throws(() => new valuations.Market(JSON.stringify(state)), /JSON depth.*96/);
});

test('malformed calibration input exposes canonical ingestion details', () => {
  const error = captureError(() => calibration.validateCalibrationJson('{ malformed'));

  assertStructuredError(error);
  assert.equal(error.kind, 'strict_load');
  assert.equal(error.stage, 'ingestion');
  assert.equal(error.step_id, undefined);
  assert.equal(error.solver_diagnostics, undefined);
  assert.equal(error.cause.category, 'strict_load');
});

test('Hull-White calibration requires an explicit quoted-volatility fit budget', () => {
  const envelope = {
    schema: 'finstack_quant.calibration/1',
    plan: {
      id: 'hw-budget',
      quote_sets: { quotes: [] },
      settings: {},
      steps: [
        {
          id: 'hw',
          quote_set: 'quotes',
          kind: 'hull_white',
          curve_id: 'USD-OIS',
          currency: 'USD',
          base_date: '2025-01-01',
        },
      ],
    },
  };
  const error = captureError(() => calibration.validateCalibrationJson(JSON.stringify(envelope)));
  assertStructuredError(error);
  assert.equal(error.stage, 'ingestion');
  assert.match(error.message, /fit_tolerance/);
});

test('cap/floor Hull-White inputs require index conventions and reject frequency overrides', () => {
  const envelope = {
    schema: 'finstack_quant.calibration/1',
    plan: {
      id: 'cap-conventions',
      quote_sets: { caps: [] },
      settings: {},
      steps: [
        {
          id: 'HW-CAPS',
          quote_set: 'caps',
          kind: 'cap_floor_hull_white',
          discount_curve_id: 'EUR-OIS',
          forward_curve_id: 'EUR-EURIBOR-3M',
          currency: 'EUR',
          base_date: '2026-09-30',
          fit_tolerance: 1e-4,
        },
      ],
    },
  };
  const missingIndex = captureError(() => calibration.dryRun(envelope));
  assertStructuredError(missingIndex);
  assert.match(missingIndex.message, /index_id/);

  envelope.plan.steps[0].index_id = 'EUR-EURIBOR-3M';
  const report = JSON.parse(calibration.dryRun(envelope));
  assert.ok(Array.isArray(report.errors));

  envelope.plan.steps[0].payment_frequency = 'quarterly';
  const override = captureError(() => calibration.dryRun(envelope));
  assertStructuredError(override);
  assert.match(override.message, /payment_frequency/);
});

test('step-scoped validation error keeps kind distinct from step id', () => {
  const envelope = {
    schema: 'finstack_quant.calibration/1',
    plan: {
      id: 'invalid-step',
      description: null,
      quote_sets: {},
      steps: [
        {
          id: 'discount_step',
          quote_set: 'missing_quotes',
          kind: 'discount',
          curve_id: 'USD-OIS',
          currency: 'USD',
          base_date: '2026-05-08',
        },
      ],
      settings: {},
    },
  };

  const error = captureError(() => calibration.calibrate(envelope));

  assertStructuredError(error);
  assert.equal(error.kind, 'undefined_quote_set');
  assert.equal(error.stage, 'ingestion');
  assert.equal(error.step_id, 'discount_step');
  assert.notEqual(error.kind, error.step_id);
  assert.equal(error.solver_diagnostics, undefined);
  assert.equal(error.cause.category, 'undefined_quote_set');
});

test('unacceptable SABR slices fail before a surface report exists', () => {
  const envelope = JSON.parse(readFileSync(EQUITY_VOL_EXAMPLE, 'utf8'));
  envelope.plan.settings.fail_on_bad_fit = true;
  envelope.plan.settings.vol_surface = { validation_tolerance: 1e-4 };

  const error = captureError(() => calibration.calibrate(envelope));

  assertStructuredError(error);
  assert.equal(error.kind, 'vol_surface');
  assert.equal(error.stage, 'target');
  assert.equal(error.step_id, 'AAPL-EQUITY-VOL-STEP');
  assert.match(error.message, /no acceptable deterministic SABR start/);
  assert.match(error.message, /best_rejected_residual=/);
  assert.equal(error.solver_diagnostics, undefined);
});

test('surface acceptance uses the published grid rather than fitted SABR slices', () => {
  const envelope = JSON.parse(readFileSync(EQUITY_VOL_EXAMPLE, 'utf8'));
  envelope.plan.steps[1].target_strikes = [140, 180, 220];
  envelope.plan.settings.fail_on_bad_fit = true;
  envelope.plan.settings.vol_surface = { validation_tolerance: 0.001 };
  const error = captureError(() => calibration.calibrate(envelope));
  assertStructuredError(error);
  assert.equal(error.stage, 'solver');
  assert.equal(error.kind, 'solver_not_converged');
  assert.ok(error.solver_diagnostics.max_residual > 0.001);
  assert.equal(error.solver_diagnostics.tolerance, 0.001);
  assert.equal(typeof error.solver_diagnostics.iterations, 'number');
  assert.equal(typeof error.solver_diagnostics.worst_quote_id, 'string');
  assert.equal(typeof error.solver_diagnostics.worst_quote_residual, 'number');
  assert.deepEqual(error.solver_diagnostics, error.cause.solver_diagnostics);
});

test('parametric calibration rejects a separate discount curve', () => {
  const envelope = JSON.parse(readFileSync(EQUITY_VOL_EXAMPLE, 'utf8'));
  const discount = envelope.plan.steps[0];
  envelope.plan.steps = [
    discount,
    {
      id: 'NS',
      kind: 'parametric',
      curve_id: 'NS',
      model: 'ns',
      base_date: discount.base_date,
      discount_curve_id: discount.curve_id,
      quote_set: discount.quote_set,
    },
  ];
  const error = captureError(() => calibration.calibrate(envelope));
  assertStructuredError(error);
  assert.equal(error.stage, 'ingestion');
  assert.match(error.message, /discount_curve_id/);
});

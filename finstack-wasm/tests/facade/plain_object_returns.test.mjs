/**
 * Facade-shape tests for map-returning WASM functions.
 *
 * `index.d.ts` declares plain JS objects for these returns; the bindings
 * must therefore serialize with JSON-compatible conventions rather than the
 * serde-wasm-bindgen default, which emits ES2015 `Map`s (whose property
 * reads silently yield `undefined`).
 *
 * Requires the wasm-pack Node build: npm run build:node
 */

import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath, pathToFileURL } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const PKG_NODE_DIR = join(__dirname, '..', '..', 'pkg-node');
const WASM_JS = join(PKG_NODE_DIR, 'finstack_wasm.js');

if (!existsSync(WASM_JS)) {
  throw new Error(
    `finstack-wasm Node build not found at ${WASM_JS}. Generate it with: npm run build:node`
  );
}

const wasm = await import(pathToFileURL(WASM_JS).href);

function assertPlainObject(value, label) {
  assert.ok(!(value instanceof Map), `${label} must not be an ES2015 Map`);
  assert.equal(typeof value, 'object', `${label} must be an object`);
  assert.notEqual(value, null, `${label} must not be null`);
  assert.ok(!Array.isArray(value), `${label} must not be an array`);
}

test('tarnCouponProfile returns a plain object with readable properties', () => {
  const profile = wasm.tarnCouponProfile(0.05, 0.0, [0.01, 0.02, 0.03], 0.08, 0.5);
  assertPlainObject(profile, 'tarnCouponProfile result');
  assert.ok(Array.isArray(profile.coupons_paid), 'coupons_paid is an array');
  assert.ok(Array.isArray(profile.cumulative), 'cumulative is an array');
  assert.equal(typeof profile.redeemed_early, 'boolean');
});

test('SabrSmile.arbitrageDiagnostics returns a plain object', () => {
  const params = new wasm.SabrParameters(0.2, 1.0, 0.3, -0.2);
  const smile = new wasm.SabrSmile(params, 100.0, 1.0);
  const diag = smile.arbitrageDiagnostics([80, 90, 100, 110, 120]);
  assertPlainObject(diag, 'arbitrageDiagnostics result');
  assert.equal(typeof diag.arbitrageFree, 'boolean');
  assert.ok(Array.isArray(diag.butterflyViolations));
  assert.ok(Array.isArray(diag.monotonicityViolations));
});

test('listStandardMetricsGrouped returns a plain object of string arrays', () => {
  const grouped = wasm.listStandardMetricsGrouped();
  assertPlainObject(grouped, 'listStandardMetricsGrouped result');
  const groups = Object.keys(grouped);
  assert.ok(groups.length > 0, 'at least one metric group');
  for (const group of groups) {
    assert.ok(Array.isArray(grouped[group]), `group ${group} maps to an array`);
  }
});

test('FxOption.greeks returns a plain object keyed by metric name', () => {
  // Reuse the canonical FX-option golden fixture (instrument + calibration
  // envelope) so the market shape cannot drift from the pricing pipeline.
  const fixturePath = join(
    __dirname,
    '..',
    '..',
    '..',
    'finstack',
    'valuations',
    'tests',
    'golden',
    'data',
    'pricing',
    'fx_option',
    'gk_eurusd_atm_3m.json'
  );
  const fixture = JSON.parse(readFileSync(fixturePath, 'utf8'));
  const asOf = fixture.metadata.valuation_date;

  const calibrated = JSON.parse(wasm.calibrate(JSON.stringify(fixture.market.envelope)));
  const marketJson = JSON.stringify(calibrated.result.final_market);
  const option = new wasm.FxOption(fixture.instrument.spec);

  const greeks = option.greeks(marketJson, asOf);
  assertPlainObject(greeks, 'FxOption.greeks result');
  const entries = Object.entries(greeks);
  assert.ok(entries.length > 0, 'greeks object has own enumerable properties');
  assert.equal(typeof greeks.delta, 'number', 'delta is directly readable');
  for (const [key, value] of entries) {
    assert.equal(typeof value, 'number', `greek ${key} is a number`);
  }
});

test('SabrCalibrator surface: withTolerance, calibrate, calibrateAutoShift, params', () => {
  const forward = 0.03;
  const strikes = [0.01, 0.02, 0.03, 0.04, 0.05];
  const t = 1.0;
  const beta = 0.5;
  const base = new wasm.SabrParameters(0.05, beta, 0.4, -0.1);
  const smile = new wasm.SabrSmile(base, forward, t);
  const vols = smile.generateSmile(strikes);

  const calibrator = new wasm.SabrCalibrator().withTolerance(1e-8);
  const fitted = calibrator.calibrate(forward, strikes, vols, t, beta);
  assert.equal(fitted.beta, beta);
  assert.ok(fitted.alpha > 0);

  const auto = calibrator.calibrateAutoShift(forward, strikes, vols, t, beta);
  assert.equal(auto.beta, beta);
  assert.equal(auto.shift, undefined, 'positive-rate smile needs no shift');

  const model = new wasm.SabrModel(fitted);
  const params = model.params;
  assert.equal(params.beta, beta);
  assert.equal(typeof params.alpha, 'number');
});

test('calibrateAutoShift fits a negative-rate smile with a shift', () => {
  const forward = -0.005;
  const strikes = [-0.015, -0.01, -0.005, 0.0, 0.005];
  const t = 1.0;
  const beta = 0.5;
  const shifted = new wasm.SabrParameters(0.05, beta, 0.4, -0.1, 0.03);
  const smile = new wasm.SabrSmile(shifted, forward, t);
  const vols = smile.generateSmile(strikes);

  const fitted = new wasm.SabrCalibrator().calibrateAutoShift(forward, strikes, vols, t, beta);
  assert.equal(typeof fitted.shift, 'number', 'negative-rate fit must carry a shift');
  assert.ok(fitted.shift > 0);
  assert.ok(fitted.isShifted());
});

/**
 * Closed-form, Monte Carlo and SABR behaviour owned by Rust (WASM-audit slice S15).
 *
 * The same cases are asserted for Python in
 * `finstack-quant-py/tests/models/test_closed_form_rust_owned.py`.
 */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { models } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

const kind = (expected, pattern) => (error) => {
  assert.ok(error instanceof Error, `expected an Error, got ${typeof error}: ${error}`);
  assert.equal(error.kind, expected, `kind of: ${error.message}`);
  if (pattern) assert.match(error.message, pattern);
  return true;
};

test('black76Price rejects a non-positive df like its inverse', () => {
  for (const df of [0, -0.95]) {
    assert.throws(
      () => models.black76Price(100, 100, df, 1, 0.2, true),
      kind('validation', /Black-76 df must be positive/)
    );
    assert.throws(() => models.black76ImpliedVol(100, 100, df, 1, 7.5, true), kind('validation'));
  }
  assert.ok(Math.abs(models.black76Price(100, 100, 0.95, 1, 0.2, true) - 7.5673) < 1e-4);
});

test('forward kernels reject out-of-domain inputs in Rust', () => {
  assert.throws(
    () => models.black76Price(100, 100, 0.95, 1, -0.2, true),
    kind('validation', /Black-76 vol must be non-negative/)
  );
  assert.throws(
    () => models.bachelierPrice(0.03, 0.03, -0.0075, 1, true),
    kind('validation', /Bachelier normal_vol must be non-negative/)
  );
  assert.throws(
    () => models.blackShiftedPrice(-0.005, -0.02, 0.25, 1, 0.01, false),
    kind('validation', /shifted Black strike \+ shift must be positive/)
  );
  assert.throws(
    () => models.blackShiftedVega(-0.02, 0.01, 0.25, 1, 0.01),
    kind('validation', /shifted Black forward \+ shift must be positive/)
  );
});

test('forward Greeks are the Rust ForwardGreeks object', () => {
  const g = models.black76Greeks(100, 100, 1, 0.2, false);
  assert.deepEqual(Object.keys(g).sort(), ['delta', 'gamma', 'vega']);
  assert.ok(g.delta > -0.5 && g.delta < -0.4);
  assert.ok(Math.abs(models.bachelierGreeks(0.03, 0.03, 0.0075, 1, true).delta - 0.5) < 1e-12);
});

test('hestonPrice dispatches the put through Rust', () => {
  const args = [100, 100, 1, 0.05, 0.02, 2, 0.04, 0.3, -0.7, 0.04];
  const call = models.hestonPrice(...args);
  const put = models.hestonPrice(...args, false);
  const parity = call - put - (100 * Math.exp(-0.02) - 100 * Math.exp(-0.05));
  assert.ok(Math.abs(parity) < 1e-8, `parity residual ${parity}`);
});

test('priceHestonCall defaults numPaths and seed from the Rust registry and returns MoneyEstimate', () => {
  const heston = [100, 100, 0.05, 0, 2, 0.04, 0.3, -0.7, 0.04, 1];
  const est = models.monteCarlo.priceHestonCall(...heston, undefined, undefined, 1);
  assert.equal(est.num_paths, 100000);
  assert.equal(typeof est.mean.amount, 'string');
  assert.equal(est.mean.currency, 'USD');
  assert.equal(est.ci_95.length, 2);
  for (const key of ['currency', 'ci_lower', 'ci_upper', 'relative_stderr']) {
    assert.equal(key in est, false, `${key} is not a MoneyEstimate field`);
  }
  // The registry seed is fixed, so the default run is reproducible.
  const again = models.monteCarlo.priceHestonCall(...heston, null, null, 1);
  assert.deepEqual(again, est);
  const explicit = models.monteCarlo.priceHestonPut(...heston, 500, 7n, 4);
  assert.equal(explicit.num_paths, 500);
});

test('SabrSmile single-strike vol and arbitrage verdict come from Rust', () => {
  const params = new models.volatility.SabrParameters(0.2, 0.5, 0.3, -0.2);
  const smile = new models.volatility.SabrSmile(params, 100, 1);
  const strikes = [80, 90, 100, 110, 120];
  assert.deepEqual(
    strikes.map((k) => smile.impliedVol(k)),
    Array.from(smile.generateSmile(strikes))
  );
  assert.deepEqual(smile.validateNoArbitrage(strikes, 0), {
    arbitrage_free: true,
    butterfly_violations: [],
    monotonicity_violations: [],
  });
  assert.equal('arbitrageDiagnostics' in smile, false);
  smile.free();
  params.free();
});

test('SabrCalibrator.withShift parses "auto" in Rust and rejects booleans', () => {
  const { SabrCalibrator } = models.volatility;
  new SabrCalibrator().withShift('auto').free();
  assert.throws(() => new SabrCalibrator().withShift(true), kind('invalid_type', /^shift:/));
  assert.throws(
    () => new SabrCalibrator().withShift('AUTO'),
    kind('validation', /SABR shift must be null\/None, a number, or "auto"/)
  );
});

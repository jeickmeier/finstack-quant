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
      () => models.black76Price(100, 100, df, 0.2, 1, true),
      kind('validation', /Black-76 df must be positive/)
    );
    assert.throws(() => models.black76ImpliedVol(100, 100, df, 1, 7.5, true), kind('validation'));
  }
  assert.ok(Math.abs(models.black76Price(100, 100, 0.95, 0.2, 1, true) - 7.5673) < 1e-4);
});

test('forward kernels reject out-of-domain inputs in Rust', () => {
  assert.throws(
    () => models.black76Price(100, 100, 0.95, -0.2, 1, true),
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
  const g = models.black76Greeks(100, 100, 0.2, 1, false);
  assert.deepEqual(Object.keys(g).sort(), ['delta', 'gamma', 'vega']);
  assert.ok(g.delta > -0.5 && g.delta < -0.4);
  assert.ok(Math.abs(models.bachelierGreeks(0.03, 0.03, 0.0075, 1, true).delta - 0.5) < 1e-12);
});

test('hestonPrice dispatches the put through Rust', () => {
  const params = { r: 0.05, q: 0.02, v0: 0.04, kappa: 2, theta: 0.04, sigma_v: 0.3, rho: -0.7 };
  const call = models.hestonPrice(100, 100, 1, params);
  const put = models.hestonPrice(100, 100, 1, params, false);
  const parity = call - put - (100 * Math.exp(-0.02) - 100 * Math.exp(-0.05));
  assert.ok(Math.abs(parity) < 1e-8, `parity residual ${parity}`);
});

test('priceHeston defaults numPaths and seed from the Rust registry and returns MoneyEstimate', () => {
  const params = { r: 0.05, q: 0, v0: 0.04, kappa: 2, theta: 0.04, sigma_v: 0.3, rho: -0.7 };
  const est = models.monteCarlo.priceHeston(100, 100, 1, params, true, { num_steps: 1 });
  assert.equal(est.num_paths, 100000);
  assert.equal(typeof est.mean.amount, 'string');
  assert.equal(est.mean.currency, 'USD');
  assert.equal(est.ci_95.length, 2);
  for (const key of ['currency', 'ci_lower', 'ci_upper', 'relative_stderr']) {
    assert.equal(key in est, false, `${key} is not a MoneyEstimate field`);
  }
  // The registry seed is fixed, so the default run is reproducible.
  const again = models.monteCarlo.priceHeston(100, 100, 1, params, true, {
    num_paths: null,
    seed: null,
    num_steps: 1,
  });
  assert.deepEqual(again, est);
  const explicit = models.monteCarlo.priceHeston(100, 100, 1, params, false, {
    num_paths: 500,
    seed: 7n,
    num_steps: 4,
  });
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
  // The host union is the Rust `SabrShift` serde form: null, a number or "auto".
  assert.throws(
    () => new SabrCalibrator().withShift(true),
    kind('validation', /shift: .*got bool/)
  );
  assert.throws(() => new SabrCalibrator().withShift(Number.NaN), kind('invalid_type', /^shift:/));
  const fixed = new SabrCalibrator().withShift(0.02);
  assert.equal(fixed.shift, 0.02);
  assert.equal(fixed.withShift(undefined).shift, null);
  assert.throws(
    () => new SabrCalibrator().withShift('AUTO'),
    kind('validation', /SABR shift must be null\/None, a number, or "auto"/)
  );
});

test('optionCalculation prices, inverts and returns the model Greeks in one call', () => {
  const blackScholes = {
    kind: 'black_scholes',
    spot: 100,
    strike: 105,
    rate: 0.03,
    div_yield: 0.01,
    expiry: 0.75,
    is_call: false,
  };
  const priced = models.optionCalculation({
    model: blackScholes,
    solve: { kind: 'price', vol: 0.25 },
  });
  assert.equal(priced.price, models.bsPrice(100, 105, 0.03, 0.01, 0.25, 0.75, false));
  assert.equal(priced.implied_vol, 0.25);
  assert.deepEqual(priced.greeks, {
    kind: 'black_scholes',
    ...models.bsGreeks({
      spot: 100,
      strike: 105,
      rate: 0.03,
      dividend_yield: 0.01,
      volatility: 0.25,
      time_to_expiry: 0.75,
      option_type: 'put',
    }),
  });
  const bachelier = {
    kind: 'bachelier',
    forward: -0.002,
    strike: 0.001,
    expiry: 1.5,
    is_call: true,
  };
  const normal = models.optionCalculation({
    model: bachelier,
    solve: { kind: 'price', vol: 0.006 },
  });
  const solved = models.optionCalculation(
    JSON.stringify({ model: bachelier, solve: { kind: 'implied_vol', price: normal.price } })
  );
  assert.ok(Math.abs(solved.implied_vol - 0.006) < 1e-7);
  assert.equal(solved.greeks.kind, 'forward');
  assert.throws(
    () =>
      models.optionCalculation({
        model: { ...blackScholes, spot: 120, is_call: true },
        solve: { kind: 'implied_vol', price: 1 },
      }),
    (error) => error.kind === 'validation' && /intrinsic/.test(error.message)
  );
  assert.throws(
    () =>
      models.optionCalculation({
        model: { ...blackScholes, df: 1 },
        solve: { kind: 'price', vol: 0.2 },
      }),
    (error) => error.kind === 'validation' && /unknown field/.test(error.message)
  );
});

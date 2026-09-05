/** Valuation audit contracts shared with Python and exercised through the facade. */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { valuations } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});
const fixtures = JSON.parse(
  readFileSync(
    new URL(
      '../../../finstack-quant/valuations/tests/fixtures/valuation_binding_regressions.json',
      import.meta.url
    )
  )
);
const price = (instrument, market, model, metrics = []) =>
  valuations.instruments.priceInstrument(
    JSON.stringify(instrument),
    JSON.stringify(market),
    '2025-01-02',
    model,
    metrics
  );

for (const model of ['black76', 'static_replication']) {
  for (const missing of ['curves', 'surfaces']) {
    test(`${model}: missing ${missing} preserves not_found`, () => {
      const market = structuredClone(fixtures.market);
      market[missing] = [];
      assert.throws(
        () => price(fixtures.cms, market, model),
        (error) => {
          assert.equal(error.kind, 'not_found');
          assert.match(error.message, /USD-/);
          return true;
        }
      );
    });
  }
  test(`${model}: invalid CMS contract preserves validation`, () => {
    const instrument = structuredClone(fixtures.cms);
    instrument.instrument.spec.accrual_fractions = [];
    assert.throws(
      () => price(instrument, fixtures.market, model),
      (error) => {
        assert.equal(error.kind, 'validation');
        return true;
      }
    );
  });
}

test('metric fixing requirement preserves validation', () => {
  const instrument = structuredClone(fixtures.cms);
  Object.assign(instrument.instrument.spec, {
    fixing_dates: ['2025-01-03'],
    payment_dates: ['2025-04-03'],
    metric_pricing_overrides: { theta_period: '2D' },
  });
  assert.throws(
    () => price(instrument, fixtures.market, 'static_replication', ['theta']),
    (error) => {
      assert.equal(error.kind, 'validation');
      assert.match(error.message, /fixing/i);
      return true;
    }
  );
});

test('commodity MC diagnostics survive serialization and position scaling', () => {
  const instrument = structuredClone(fixtures.commodity);
  const run = () => price(instrument, fixtures.market, 'monte_carlo_schwartz_smith');
  const base = run();
  const replay = run();
  assert.deepEqual(base.value, replay.value);
  assert.deepEqual(base.details, replay.details);
  assert.equal(base.details.type, 'monte_carlo');
  const details = base.details.data;
  assert.ok(details.standard_error > 0);
  assert.equal(Number(details.estimator_paths), 2000);
  assert.equal(Number(details.simulated_paths), 2000);
  assert.equal(details.seed, 42n);
  assert.equal(details.time_grid.length, 253);
  assert.equal(details.time_grid[0], 0);
  assert.equal(details.antithetic || details.sobol || details.brownian_bridge, false);
  instrument.instrument.spec.quantity *= 10;
  const scaled = run();
  assert.ok(Math.abs(Number(scaled.value.amount) / Number(base.value.amount) - 10) < 1e-10);
  assert.ok(Math.abs(scaled.details.data.standard_error / details.standard_error - 10) < 1e-10);
});

test('metric missing volatility preserves not_found', () => {
  assert.throws(
    () => price(fixtures.commodity, fixtures.market, 'monte_carlo_schwartz_smith', ['vega']),
    (error) => {
      assert.equal(error.kind, 'not_found');
      assert.match(error.message, /WTI-VOL/);
      return true;
    }
  );
});

for (const strike of ['0.00001', '0.02', '0.03', '0.04']) {
  test(`CMS zero-vol replication matches discounted intrinsic at strike ${strike}`, () => {
    const instrument = structuredClone(fixtures.cms);
    instrument.instrument.spec.strike = strike;
    const market = structuredClone(fixtures.market);
    market.surfaces[0].vols_row_major.fill(0);
    const analytic = price(instrument, market, 'black76');
    const replicated = price(instrument, market, 'static_replication');
    assert.ok(Math.abs(Number(analytic.value.amount) - Number(replicated.value.amount)) < 1e-7);
  });
}

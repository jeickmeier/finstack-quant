import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { analytics } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

function dates(n) {
  return Array.from({ length: n }, (_, i) =>
    new Date(Date.UTC(2024, 0, 1 + i)).toISOString().slice(0, 10)
  );
}

function panel(values, frequency = 'daily') {
  return analytics.Performance.fromReturns(dates(values.length), [values], ['A'], null, frequency);
}

function close(actual, expected) {
  assert.ok(Math.abs(actual - expected) < 1e-12, `${actual} != ${expected}`);
}

test('tail metrics retain exact empirical probability mass', () => {
  const values = [-0.2, ...Array(99).fill(0)];
  close(panel(values).expectedShortfall(0.95)[0], -0.04);
  values[1] = 0.25;
  close(panel(values).cdar(0.95)[0], -0.04);
  close(panel([-0.2, -0.1, 0.1, 0.2]).expectedShortfall(0.625)[0], -0.25 / 1.5);
});

test('all scalar facade exports exist and preserve invalid risk', () => {
  for (const name of ['maxDrawdown', 'sharpe', 'sortino', 'volatility']) {
    assert.equal(typeof analytics[name], 'function');
  }
  assert.ok(Number.isNaN(analytics.maxDrawdown([NaN, -0.2])));
  assert.ok(Number.isNaN(analytics.sortino([NaN, 0.01])));
  assert.ok(Number.isNaN(analytics.sharpe([0, 0], NaN)));
  assert.ok(Number.isNaN(analytics.sharpe([0.01])));
  assert.ok(Number.isNaN(panel([0.01]).sharpe()[0]));
  assert.ok(Array.from(panel([0.01, 0.02]).rollingSharpe(0, 1).sharpe).every(Number.isNaN));
});

test('integer arguments reject truncation, wrapping and non-finite values', () => {
  const perf = panel([0.01, -0.02, 0.03]);
  for (const bad of [0.9, -1, NaN, Infinity, 4294967296]) {
    for (const call of [
      () => perf.returnsForTicker(bad),
      () => perf.activeDatesForTicker(bad),
      () => perf.rollingReturns(0, bad),
      () => perf.rollingSharpe(0, bad),
      () => perf.rollingSortino(0, bad),
      () => perf.rollingVolatility(0, bad),
      () => perf.rollingGreeks(0, bad),
      () => perf.drawdownDetails(0, bad),
      () => perf.sterlingRatio(0, bad),
      () => perf.burkeRatio(0, bad),
      () => perf.periodStats(bad),
      () => perf.periodStats(0, 'monthly', bad),
    ])
      assert.throws(call, /integer|range/);
  }
  assert.deepEqual(Array.from(perf.returnsForTicker(0)), [0.01, -0.02, 0.03]);
  assert.throws(() => perf.rollingReturns(0, 0), /at least 1/);
});

test('duplicate ticker labels and unrepresentable price returns are rejected', () => {
  assert.throws(
    () =>
      analytics.Performance.fromReturns(
        dates(2),
        [
          [0.01, 0.02],
          [0.03, 0.04],
        ],
        ['A', 'A']
      ),
    /duplicate ticker/
  );
  assert.throws(() => new analytics.Performance(dates(2), [[1e-300, 1e300]], ['A']), /finite/);
});

test('drawdown dates, cash basis, Kelly and undefined factor statistics agree with Rust', () => {
  const perf = new analytics.Performance(dates(3), [[100, 90, 100]], ['A']);
  const episode = perf.drawdownDetails(0)[0];
  assert.equal(episode.duration_days, 2);
  assert.equal(episode.truncated_at_start, false);
  close(panel([0.01, -0.02, 0.03, 0.01], 'monthly').mSquared(0.12)[0], 0.09);
  close(panel([0.02, 0.02, -0.01, 0]).periodStats(0, 'daily').kelly_criterion, 0.5);
  for (const n of [5, 29, 57, 58]) {
    const fit = panel(Array(n).fill(0.01)).multiFactorGreeks(0, [
      Array.from({ length: n }, (_, i) => 0.001 * i),
    ]);
    assert.ok(Number.isNaN(fit.r_squared));
    assert.ok(Number.isNaN(fit.adjusted_r_squared));
  }
});

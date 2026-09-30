/**
 * Analytics results keep Rust shapes, and omitted arguments resolve to the
 * Rust-owned `finstack_quant_analytics::DEFAULT_*` constants.
 *
 * The same cases run in Python in
 * `finstack-quant-py/tests/test_analytics_rust_defaults.py`.
 */

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { analytics, models } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

function dates(n) {
  return Array.from({ length: n }, (_, i) =>
    new Date(Date.UTC(2024, 0, 1 + i)).toISOString().slice(0, 10)
  );
}

const RETURNS = Array.from(
  { length: 120 },
  (_, i) => 0.004 * Math.sin(i) + 0.0005 * (i % 7) - 0.001
);
const BENCH = Array.from({ length: 120 }, (_, i) => 0.003 * Math.cos(i) + 0.0002);

function perf() {
  return analytics.Performance.fromReturns(
    dates(RETURNS.length),
    [RETURNS, BENCH],
    ['FUND', 'BENCH'],
    'BENCH'
  );
}

function sameArray(actual, expected) {
  assert.deepEqual(Array.from(actual), Array.from(expected));
}

test('rolling series carry the Rust DatedSeries field names and metric label', () => {
  const p = perf();
  const cases = [
    [p.rollingVolatility(0, 10), 'volatility'],
    [p.rollingSortino(0, 10), 'sortino'],
    [p.rollingSharpe(0, 10), 'sharpe'],
    [p.rollingReturns(0, 10), 'return'],
  ];
  for (const [series, label] of cases) {
    assert.deepEqual(Object.keys(series).sort(), ['dates', 'value_column', 'values']);
    assert.ok(series.values instanceof Float64Array);
    assert.equal(series.values.length, series.dates.length);
    assert.equal(series.value_column, label);
  }
});

test('periodicReturns emits Rust PeriodicReturn points with ISO dates', () => {
  const [[point]] = perf().periodicReturns();
  assert.deepEqual(Object.keys(point).sort(), ['date', 'value']);
  assert.equal(typeof point.date, 'string');
  assert.match(point.date, /^\d{4}-\d{2}-\d{2}$/);
  assert.equal(typeof point.value, 'number');
});

test('omitted arguments equal the Rust defaults', () => {
  const p = perf();
  assert.equal(analytics.sharpe(RETURNS), analytics.sharpe(RETURNS, 0, 252));
  assert.equal(analytics.sortino(RETURNS), analytics.sortino(RETURNS, 0, 252));
  assert.equal(analytics.volatility(RETURNS), analytics.volatility(RETURNS, 252));
  sameArray(p.valueAtRisk(), p.valueAtRisk(0.95));
  sameArray(p.meanReturn(), p.meanReturn(true));
  sameArray(p.omegaRatio(), p.omegaRatio(0));
  sameArray(p.sterlingRatio(), p.sterlingRatio(0, 5));
  sameArray(p.rollingSharpe(0).values, p.rollingSharpe(0, 63, 0).values);
  assert.deepEqual(p.drawdownDetails(0), p.drawdownDetails(0, 5));
  assert.deepEqual(p.periodicReturns(), p.periodicReturns('monthly'));
  assert.deepEqual(p.periodStats(0), p.periodStats(0, 'monthly'));
  assert.deepEqual(p.multiFactorGreeks(0, [BENCH]), p.multiFactorGreeks(0, [BENCH], 'excess', 0));
  sameArray(p.cagr(), p.cagr('act365_25'));
  const fromPrices = new analytics.Performance(dates(3), [[100, 101, 102]], ['A']);
  assert.equal(fromPrices.frequency(), 'daily');
});

test('a partial fiscal-year start fills the other half per FiscalConfig::from_parts', () => {
  const p = perf();
  const ref = '2024-04-29';
  assert.deepEqual(p.lookbackReturns(ref), p.lookbackReturns(ref, 1, 1));
  assert.deepEqual(p.lookbackReturns(ref, 3), p.lookbackReturns(ref, 3, 1));
  assert.deepEqual(p.lookbackReturns(ref, undefined, 20), p.lookbackReturns(ref, 1, 20));
  assert.deepEqual(p.periodStats(0, 'annual', 3), p.periodStats(0, 'annual', 3, 1));
  assert.deepEqual(p.periodStats(0, 'annual', undefined, 20), p.periodStats(0, 'annual', 1, 20));
  assert.throws(() => p.lookbackReturns(ref, 13), /start_month/);
});

test('non-finite result fields named by Rust NonFiniteFields stay numeric', () => {
  const allUp = analytics.Performance.fromReturns(dates(90), [Array(90).fill(0.01)], ['UP']);
  const stats = allUp.periodStats(0);
  assert.equal(stats.profit_factor, Infinity);
  assert.equal(stats.payoff_ratio, Infinity);

  const flat = analytics.Performance.fromReturns(dates(10), [Array(10).fill(0.01)], ['FLAT']);
  const mf = flat.multiFactorGreeks(0, [
    [0.01, -0.02, 0.03, 0.0, 0.01, -0.01, 0.02, 0.0, 0.01, -0.02],
  ]);
  assert.ok(Number.isNaN(mf.r_squared));
  assert.equal(typeof mf.adjusted_r_squared, 'number');

  const constantBench = analytics.Performance.fromReturns(
    dates(10),
    [RETURNS.slice(0, 10), Array(10).fill(0.001)],
    ['FUND', 'BENCH'],
    'BENCH'
  );
  for (const row of constantBench.beta()) {
    assert.deepEqual(Object.keys(row).sort(), ['beta', 'ci_lower', 'ci_upper', 'std_err']);
    assert.ok(Object.values(row).every((v) => typeof v === 'number'));
  }
  for (const row of constantBench.greeks()) {
    assert.deepEqual(Object.keys(row).sort(), ['adjusted_r_squared', 'alpha', 'beta', 'r_squared']);
    assert.ok(Object.values(row).every((v) => typeof v === 'number'));
  }

  const budget = models.factor.risk.evaluateRiskBudget(['A', 'B'], [60, 40], [1, 0], 100);
  const unbudgeted = budget.positions.find((row) => row.position_id === 'B');
  assert.equal(unbudgeted.utilization, Infinity);
});

test('constrainedLeastSquares leaves the zero-factor rule to Rust', () => {
  assert.throws(
    () => analytics.constrainedLeastSquares([1, 2], 0, [0.01, 0.02], [0.5, 0.5]),
    (err) => err.kind === 'validation' && !/nFactors must be/.test(err.message)
  );
  assert.throws(
    () => analytics.constrainedLeastSquares([1, 2], -1, [0.01, 0.02], [0.5, 0.5]),
    (err) => err.kind === 'invalid_type'
  );
});

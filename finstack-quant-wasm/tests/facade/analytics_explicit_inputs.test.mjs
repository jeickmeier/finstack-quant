import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { analytics, core } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

const dates = (n) =>
  Array.from({ length: n + 1 }, (_, i) =>
    new Date(Date.UTC(2025, 0, 1 + i)).toISOString().slice(0, 10)
  );
const returns = [0.01, -0.02, 0.03, 0, -0.01];
const panel = (context = {}) =>
  new analytics.Performance({
    dates: dates(returns.length),
    series: [returns],
    kind: 'returns',
    ticker_names: ['FUND'],
    ...context,
  });
const complete = () => panel({ frequency: 'daily', benchmark_ticker: 'FUND' });

test('omitted frequency and benchmark retain absence until requested', () => {
  const p = panel();
  assert.equal(p.getFrequency(), undefined);
  assert.equal(p.getBenchmarkIdx(), undefined);
  assert.ok(Number.isFinite(p.meanReturn(false)[0]));
  assert.ok(Number.isFinite(p.volatility(false)[0]));
  assert.ok(Number.isFinite(p.cagr()[0]));
  assert.throws(() => p.volatility(true));
  assert.throws(() => p.sharpe(0));
  assert.throws(() => p.beta());
  assert.throws(() => p.getPairedDates(0));
  p.resetBenchTicker('FUND');
  assert.equal(p.getBenchmarkIdx(), 0);
  p.free();
});

test('financial request inputs and units are explicit across hosts', () => {
  const p = complete();
  const missing = [
    () => analytics.sharpe(returns),
    () => analytics.sortino(returns),
    () => analytics.volatility(returns),
    () => p.meanReturn(),
    () => p.volatility(),
    () => p.sharpe(),
    () => p.sortino(),
    () => p.downsideDeviation(),
    () => p.omegaRatio(),
    () => p.valueAtRisk(),
    () => p.expectedShortfall(),
    () => p.tailRatio(),
    () => p.cdar(),
    () => p.parametricVar(),
    () => p.cornishFisherVar(),
    () => p.modifiedSharpe(),
    () => p.martinRatio(),
    () => p.burkeRatio(),
    () => p.sterlingRatio(0),
    () => p.drawdownDetails(0),
    () => p.periodicReturns(),
    () => p.periodStats(0),
    () => p.rollingVolatility(0),
    () => p.rollingSharpe(0, 2),
    () => p.rollingSortino(0, 2),
    () => p.rollingGreeks(0, 2),
    () => p.summary(0, 0, 0),
  ];
  for (const call of missing) assert.throws(call);
  for (const bad of [NaN, Infinity, -1]) assert.throws(() => p.sharpe(bad));
  for (const bad of [0.5, 1, NaN]) assert.throws(() => p.valueAtRisk(bad));
  for (const bad of [0, -1, 1.5, NaN, 9007199254740992]) {
    assert.throws(() => p.parametricVar(0.95, bad));
    assert.throws(() => p.cornishFisherVar(0.95, bad));
  }
  assert.throws(() => p.sterlingRatio(0, 0));
  assert.deepEqual(p.drawdownDetails(0, 0), []);
  p.free();
});

test('unavailable estimates and genuine zero risk stay distinct numeric values', () => {
  assert.ok(Number.isNaN(analytics.volatility([], 12)));
  assert.ok(Number.isNaN(analytics.volatility([0.01], 12)));
  assert.equal(analytics.volatility([0.01, 0.01], 12), 0);
  assert.equal(analytics.sharpe([0.01, 0.01], 0, 12), Infinity);
  assert.ok(Number.isNaN(analytics.sharpe([0, 0], 0, 12)));
  assert.ok(Number.isNaN(analytics.sortino([0, 0], 0, 12)));
  assert.throws(() => analytics.sharpe([NaN, 0.01], 0, 12));
  assert.equal(analytics.maxDrawdown([0.01, -1]), -1);
  assert.throws(() => analytics.maxDrawdown([-1, 0]));
  assert.throws(() => analytics.maxDrawdown([-1.01]));
});

test('actual opening dates and day-count contexts control CAGR', () => {
  const p = new analytics.Performance({
    dates: ['2024-01-01', '2025-01-01'],
    series: [[0.1]],
    kind: 'returns',
    ticker_names: ['A'],
  });
  const expected = Math.expm1((Math.log1p(0.1) * 365) / 366);
  assert.ok(Math.abs(p.cagr()[0] - expected) < 1e-12);
  assert.throws(() => p.cagr('bus_252'));
  const ctx = new core.DayCountContext('nyse');
  assert.ok(Number.isFinite(p.cagr('bus_252', ctx)[0]));
  ctx.free();
  p.free();
});

test('rolling carriers and summary retain native unavailable numbers', () => {
  const p = complete();
  for (const [series, label] of [
    [p.rollingVolatility(0, 1), 'volatility'],
    [p.rollingSortino(0, 1, 0), 'sortino'],
    [p.rollingSharpe(0, 1, 0), 'sharpe'],
    [p.rollingReturns(0, 1), 'return'],
  ]) {
    assert.equal(series.value_column, label);
    assert.equal(series.values.length, series.dates.length);
    assert.ok(series.values instanceof Float64Array);
  }
  const singleton = new analytics.Performance({
    dates: dates(1),
    series: [[0]],
    kind: 'returns',
    ticker_names: ['A'],
    frequency: 'daily',
    benchmark_ticker: 'A',
  });
  const summary = singleton.summary(0, 0, 0, 0.95);
  analytics.validateSummary(summary);
  const invalid = structuredClone(summary);
  invalid.metadata.rows[0].paired_sample_count = 2;
  assert.throws(() => analytics.validateSummary(invalid));
  const vol = summary.columns.find((column) => column.name === 'volatility');
  assert.ok(Number.isNaN(vol.data.values[0]));
  const sharpe = summary.columns.find((column) => column.name === 'sharpe');
  assert.ok(Number.isNaN(sharpe.data.values[0]));
  p.free();
  singleton.free();
});

test('canonical raw multifactor OLS differs from the total-only panel adapter', () => {
  const raw = analytics.multiFactorGreeks([0.03, 0.01, 0.05], [[0.01, 0, 0.02]], 12, 'excess');
  assert.ok(Math.abs(raw.betas[0] - 2) < 1e-12);
  assert.throws(() => analytics.multiFactorGreeks([0.01, 0.02], [[0, 1]], 12, 'total'));
  assert.throws(() => analytics.multiFactorGreeks([0.01, 0.02], [[0, 1]], 12, 'excess', 0));
  const p = complete();
  assert.equal(p.multiFactorGreeks(0, [returns], 0).n_obs, returns.length);
  p.free();
});

test('summary numeric conversion leaves literal sentinel-like ticker text untouched', () => {
  const p = new analytics.Performance({
    dates: dates(2),
    series: [[0.01, 0.01]],
    kind: 'returns',
    ticker_names: ['nan'],
    benchmark_ticker: 'nan',
    frequency: 'daily',
  });
  const summary = p.summary(0, 0, 0, 0.95);
  assert.equal(summary.columns.find((column) => column.name === 'ticker').data.values[0], 'nan');
  assert.equal(summary.columns.find((column) => column.name === 'sharpe').data.values[0], Infinity);
  assert.ok(
    Number.isNaN(summary.columns.find((column) => column.name === 'r_squared').data.values[0])
  );
  p.free();
});

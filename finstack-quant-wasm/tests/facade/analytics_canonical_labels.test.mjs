import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { analytics, core } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});
const panel = (frequency = 'daily') =>
  analytics.Performance.fromReturns(
    ['2025-01-01', '2025-01-02', '2025-01-03', '2025-01-04'],
    [[0.01, -0.02, 0.03, 0.005]],
    ['FUND'],
    undefined,
    frequency
  );

test('canonical frequencies are accepted by core and analytics', () => {
  for (const label of ['daily', 'weekly', 'monthly', 'quarterly', 'semi_annual', 'annual']) {
    assert.equal(core.PeriodKind.fromName(label).toString(), label);
    assert.equal(panel(label).getFrequency(), label);
    assert.ok(panel().periodicReturns(label).length);
  }
});

test('frequency aliases are rejected at every public boundary', () => {
  for (const label of [
    'D',
    'B',
    'W',
    'M',
    'ME',
    'Q',
    'QE',
    'A',
    'Y',
    'YE',
    'semiannual',
    'DAILY',
    'monthly ',
    ' daily',
    '',
  ]) {
    assert.throws(() => core.PeriodKind.fromName(label));
    assert.throws(() => panel(label));
    assert.throws(() => panel().periodicReturns(label));
    assert.throws(() => panel().periodStats(0, label));
  }
});

test('CAGR accepts its canonical spelling and rejects aliases', () => {
  assert.equal(panel().cagr('act365_25').length, 1);
  for (const label of [
    'act_365_25',
    'act/365.25',
    'Act365_25',
    ' act365_25',
    'ACT/365F',
    'act_365_fixed',
    '',
  ]) {
    assert.throws(() => panel().cagr(label));
  }
});

test('Performance publishes only canonical get accessors', () => {
  const perf = panel();
  for (const name of [
    'dates',
    'activeDates',
    'activeDatesForTicker',
    'tickerNames',
    'tickerIndex',
    'benchmarkIdx',
    'frequency',
    'returns',
    'returnsForTicker',
    'drawdownSeries',
  ]) {
    assert.equal(name in perf, false, name);
    const canonical = `get${name[0].toUpperCase()}${name.slice(1)}`;
    assert.equal(typeof perf[canonical], 'function', canonical);
  }
  assert.deepEqual(perf.getTickerNames(), ['FUND']);
  assert.equal(perf.getTickerIndex('FUND'), 0);
  assert.equal(perf.getBenchmarkIdx(), 0);
  assert.equal(perf.getFrequency(), 'daily');
  assert.deepEqual(perf.getDates(), perf.getActiveDates());
  assert.deepEqual(perf.getActiveDatesForTicker(0), perf.getDates());
  assert.deepEqual(Array.from(perf.getReturnsForTicker(0)), Array.from(perf.getReturns()[0]));
  assert.equal(perf.getDrawdownSeries()[0].length, perf.getDates().length);
  assert.throws(() => perf.getTickerIndex('fund'));
  assert.throws(() => perf.getTickerIndex('MISSING'));
  perf.free();
});

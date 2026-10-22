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
    assert.equal(panel(label).frequency(), label);
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

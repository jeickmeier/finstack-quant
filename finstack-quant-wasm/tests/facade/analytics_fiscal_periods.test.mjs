import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { analytics, core } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});
const cases = JSON.parse(
  readFileSync(
    new URL('../../../finstack-quant/analytics/tests/data/fiscal_buckets.json', import.meta.url),
    'utf8'
  )
);
const close = (actual, expected) =>
  assert.ok(Math.abs(actual - expected) < 1e-12, `${actual} != ${expected}`);

for (const example of cases) {
  test(`fiscal aggregation matches boundary groups: ${example.name}`, () => {
    const perf = analytics.Performance.fromReturns(example.dates, [example.returns], ['FUND']);
    const fiscal = new core.FiscalConfig(example.start_month, example.start_day);
    for (const [frequency, groups] of Object.entries(example.groups)) {
      const expected = groups.map(
        (group) => group.reduce((wealth, index) => wealth * (1 + example.returns[index]), 1) - 1
      );
      const stats = perf.periodStats(0, frequency, fiscal);
      close(stats.best, Math.max(...expected));
      close(stats.worst, Math.min(...expected));
      close(stats.avg_return, expected.reduce((sum, value) => sum + value, 0) / expected.length);
    }
    fiscal.free();
    perf.free();
  });
}

test('aggregation rejects unrepresentable calendar and fiscal boundaries', () => {
  const perf = analytics.Performance.fromReturns(
    ['9999-12-30', '9999-12-31'],
    [[0.01, 0.02]],
    ['FUND']
  );
  const fiscal = new core.FiscalConfig(10, 1);
  const isValidation = (error) => error.kind === 'validation';
  for (const frequency of ['daily', 'weekly', 'monthly', 'quarterly', 'semi_annual', 'annual']) {
    assert.throws(() => perf.periodicReturns(frequency), isValidation);
    assert.throws(() => perf.periodStats(0, frequency), isValidation);
    assert.throws(() => perf.periodStats(0, frequency, fiscal), isValidation);
  }
  fiscal.free();
  perf.free();
});

test('fiscal aggregation preserves ragged active-window alignment', () => {
  const example = cases[0];
  const ragged = [NaN, ...example.returns.slice(1, -1), NaN];
  const perf = analytics.Performance.fromReturns(
    example.dates,
    [example.returns, ragged],
    ['BENCH', 'FUND']
  );
  perf.resetDateRange(example.dates[1], example.dates.at(-2));
  assert.deepEqual(perf.getActiveDatesForTicker(1), example.dates.slice(1, -1));
  const expected =
    example.returns.slice(1, -1).reduce((wealth, value) => wealth * (1 + value), 1) - 1;
  const usFederal = core.FiscalConfig.usFederal();
  close(perf.periodStats(1, 'monthly', usFederal).avg_return, expected);
  usFederal.free();
  perf.free();
});

test('an omitted fiscalConfig buckets weekly periods with ISO weeks', () => {
  const dates = [
    '2024-12-30',
    '2024-12-31',
    '2025-01-01',
    '2025-01-02',
    '2025-01-03',
    '2025-01-04',
    '2025-01-05',
    '2025-01-06',
    '2025-01-07',
  ];
  const returns = [0.01, -0.02, 0.03, 0.01, -0.01, 0.02, 0.01, -0.03, 0.02];
  const perf = analytics.Performance.fromReturns(dates, [returns], ['FUND']);
  const calendar = core.FiscalConfig.calendarYear();
  const omitted = perf.periodStats(0, 'weekly');
  assert.deepEqual(perf.periodStats(0, 'weekly', null), omitted);
  assert.deepEqual(perf.periodStats(0, 'weekly', undefined), omitted);
  assert.notDeepEqual(perf.periodStats(0, 'weekly', calendar), omitted);
  calendar.free();
  perf.free();
});

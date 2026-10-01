import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

import init, { statements_analytics } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

test('zero forecast percentages use round-trippable null values', () => {
  const metrics = statements_analytics.backtestForecast([0, 0], [0, 0]);
  assert.equal(metrics.mae, 0);
  assert.equal(metrics.rmse, 0);
  assert.equal(metrics.mape, null);
  assert.equal(metrics.smape, null);
  assert.equal(metrics.mape_effective_n, 0);
  assert.deepEqual(JSON.parse(JSON.stringify(metrics)), metrics);
  assert.equal(statements_analytics.backtestForecast([0], [1]).smape, 200);
});

test('backtesting rejects non-finite observations and arithmetic overflow', () => {
  for (const value of [NaN, Infinity, -Infinity]) {
    assert.throws(() => statements_analytics.backtestForecast([value], [0]), /finite/);
    assert.throws(() => statements_analytics.backtestForecast([0], [value]), /finite/);
  }
  assert.throws(() => statements_analytics.backtestForecast([1e200], [0]), /overflow/);
});

test('exit-multiple DCF sensitivity has no terminal growth shock', () => {
  const model = {
    id: 'dcf-review',
    periods: [{ id: '2025', start: '2025-01-01', end: '2026-01-01', is_actual: false }],
    nodes: {
      ufcf: {
        node_id: 'ufcf',
        node_type: 'value',
        values: { 2025: { amount: '100', currency: 'USD' } },
      },
    },
    meta: { currency: 'USD' },
    schema_version: 1,
  };
  const result = statements_analytics.dcfSensitivity(
    JSON.stringify(model),
    0.1,
    JSON.stringify({ type: 'exit_multiple', multiple: 9, terminal_metric: 100 }),
    'ufcf',
    0
  );
  assert.equal(result.terminal_growth_up, null);
  assert.equal(result.terminal_growth_up_clamped, false);
  assert.ok(result.entries.some((entry) => entry.parameter_id === 'exit_multiple'));
});

test('growth terminals require a full year even for annual period identifiers', () => {
  const model = {
    id: 'annual-stub-review',
    periods: [{ id: '2025', start: '2025-07-01', end: '2026-01-01', is_actual: false }],
    nodes: {
      ufcf: {
        node_id: 'ufcf',
        node_type: 'value',
        values: { 2025: { amount: '100', currency: 'USD' } },
      },
    },
    meta: { currency: 'USD' },
    schema_version: 1,
  };
  assert.throws(
    () =>
      statements_analytics.dcfSensitivity(
        JSON.stringify(model),
        0.1,
        JSON.stringify({ type: 'gordon_growth', stable_growth_rate: 0.02 }),
        'ufcf',
        0
      ),
    /complete contiguous history/
  );
});

test('tornado rejects joint scenarios mislabeled as diagonal JSON', () => {
  const model = {
    id: 'joint-shocks',
    periods: [{ id: '2025Q1', start: '2025-01-01', end: '2025-04-01', is_actual: false }],
    nodes: {
      revenue: { node_id: 'revenue', node_type: 'value', values: { '2025Q1': 100 } },
      cost: { node_id: 'cost', node_type: 'value', values: { '2025Q1': 20 } },
      profit: { node_id: 'profit', node_type: 'calculated', formula_text: 'revenue - cost' },
    },
    schema_version: 1,
  };
  const result = statements_analytics.runSensitivity(
    JSON.stringify(model),
    JSON.stringify({
      mode: 'full_grid',
      parameters: [
        { node_id: 'revenue', period_id: '2025Q1', base_value: 100, perturbations: [90, 110] },
        { node_id: 'cost', period_id: '2025Q1', base_value: 20, perturbations: [10, 30] },
      ],
      target_metrics: ['profit'],
    })
  );
  result.config.mode = 'diagonal';
  assert.throws(
    () => statements_analytics.generateTornadoEntries(JSON.stringify(result), 'profit', '2025Q1'),
    /exactly one/
  );
});

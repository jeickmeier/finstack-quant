import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { features } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});
const close = (a, b) => assert.ok(Math.abs(a - b) < 1e-11, `${a} != ${b}`);
const validation = (error) =>
  error instanceof Error && error.name === 'FinstackError' && error.kind === 'validation';

test('EWMA settings fail before readiness and constant series have zero mature volatility', () => {
  assert.throws(
    () => features.transformTimeseries([], [], [], 'ewma_mean', { span: 0.5 }),
    validation
  );
  assert.throws(
    () =>
      features.transformTimeseries([1], ['a'], ['1'], 'rolling_quantile', {
        window: 3,
        quantile: 2,
      }),
    validation
  );
  assert.deepEqual(
    features.transformTimeseries([0.01, 0.01], ['a', 'a'], ['1', '2'], 'ewma_vol', { span: 3 }),
    [null, 0]
  );
  assert.deepEqual(
    features.transformTimeseries([1, 1, 100], ['a', 'a', 'a'], ['1', '2', '3'], 'hampel_filter', {
      window: 3,
    }),
    [null, null, 1]
  );
});

test('rank, scale and regression invariants survive the published facade', () => {
  assert.deepEqual(features.rankToWeights([-0, 0], ['d', 'd']), [0, 0]);
  for (const scale of [1e-7, 1, 1e7]) {
    const x = [scale, 2 * scale, 3 * scale];
    close(
      features.transformTimeseriesPairwise(x, x, ['a', 'a', 'a'], ['1', '2', '3'], 'rolling_corr', {
        window: 3,
      })[2],
      1
    );
    const r = features.neutralize([1, 0, 0], ['d', 'd', 'd'], [x]);
    r.forEach((value, i) => close(value, [1 / 6, -1 / 3, 1 / 6][i]));
  }
  assert.deepEqual(
    features.neutralizeAndZscore([3, 5, 7], ['d', 'd', 'd'], [[1, 2, 3]]),
    [0, 0, 0]
  );
  assert.throws(
    () =>
      features.neutralizeAndZscore([1, 0, 0], ['d', 'd', 'd'], [[1, 2, 3]], {
        fit_intercept: false,
      }),
    validation
  );
});

test('weights enforce positive risk, final caps and neutrality', () => {
  assert.throws(() => features.riskScaledWeights([1, 2], ['d', 'd'], [1, -1]), validation);
  const capped = features.transformCrossSectional(
    [-10, -1, 1, 10],
    ['d', 'd', 'd', 'd'],
    'cap_weights',
    { max_abs: 0.3 }
  );
  capped.forEach((value, i) => close(value, [-0.3, -0.2, 0.2, 0.3][i]));
  assert.ok(capped.every((value) => Math.abs(value) <= 0.3));
  close(
    capped.reduce((a, b) => a + b, 0),
    0
  );
  assert.throws(
    () => features.transformCrossSectional([-1, 1], ['d', 'd'], 'cap_weights', { max_abs: 0.3 }),
    validation
  );
});

test('JSON and direct results agree and never hide arithmetic overflow', () => {
  assert.deepEqual(
    features.transformTimeseries([1e308, 1e308], ['a', 'a'], ['1', '2'], 'rolling_mean', {
      window: 2,
    }),
    [null, 1e308]
  );
  const spec = {
    values: [1e308, 1e308],
    entity: ['a', 'a'],
    order: ['1', '2'],
    operations: [{ name: 'm', family: 'timeseries', op: 'rolling_mean', params: { window: 2 } }],
  };
  assert.deepEqual(
    JSON.parse(features.transformPanelJson(JSON.stringify(spec))).columns[0].values,
    [null, 1e308]
  );
  spec.operations[0].op = 'rolling_sum';
  assert.throws(() => features.transformPanelJson(JSON.stringify(spec)), validation);
  assert.throws(
    () =>
      features.transformTimeseries([1e308, 1e308], ['a', 'a'], ['1', '2'], 'rolling_sum', {
        window: 2,
      }),
    validation
  );
});

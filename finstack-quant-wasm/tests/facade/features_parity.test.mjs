/**
 * Facade tests for the typed panel pipeline and the operation-selector twins
 * of the Python `features` classes (parity slice P3).
 *
 * Expected values come from `finstack-quant-py/tests/data/features_wasm_parity.json`,
 * which holds the Python outputs for the same inputs and is asserted by
 * `finstack-quant-py/tests/test_features_wasm_parity.py`, so each case is a
 * cross-host golden: both hosts call the same Rust entry point.
 *
 * Requires the wasm-pack web build: npm run build (mise run wasm-build).
 */

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { features } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

const GOLDEN = JSON.parse(
  readFileSync(
    new URL('../../../finstack-quant-py/tests/data/features_wasm_parity.json', import.meta.url),
    'utf8'
  )
);
const core = (message) => message.replace(/^Validation error: /, '');

test('transformPanel returns the Python transform_panel columns as an object', () => {
  const result = features.transformPanel(GOLDEN.spec);
  assert.deepEqual(result, GOLDEN.expected);
  assert.deepEqual(features.transformPanel(JSON.stringify(GOLDEN.spec)), GOLDEN.expected);
  assert.deepEqual(result, JSON.parse(features.transformPanelJson(GOLDEN.spec)));
});

test('transformPanel rejects what Python transform_panel rejects', () => {
  const { reserved_name_error: reserved } = GOLDEN;
  assert.equal(reserved.exception, 'ValueError');
  assert.throws(
    () =>
      features.transformPanel({
        ...GOLDEN.spec,
        operations: [{ family: 'timeseries', name: 'values', op: 'diff' }],
      }),
    (error) => error.kind === 'validation' && error.message.includes(core(reserved.message))
  );
  assert.throws(
    () => features.transformPanel({ ...GOLDEN.spec, unknown_field: 1 }),
    (error) => error.kind === 'validation'
  );
  assert.throws(() => features.transformPanel(5), TypeError);
});

test('panelTransformResultGetColumn matches Python PanelTransformResult.get_column', () => {
  const result = features.transformPanel(GOLDEN.spec);
  for (const [name, values] of Object.entries(GOLDEN.columns)) {
    assert.deepEqual(features.panelTransformResultGetColumn(result, name), values);
    assert.deepEqual(
      features.panelTransformResultGetColumn(features.transformPanelJson(GOLDEN.spec), name),
      values
    );
  }
  // Python raises KeyError for a missing column.
  assert.equal(GOLDEN.missing_column_error.exception, 'KeyError');
  assert.throws(
    () => features.panelTransformResultGetColumn(result, 'nope'),
    (error) => error.kind === 'not_found' && /nope/.test(error.message)
  );
});

for (const [type, golden] of Object.entries(GOLDEN.ops)) {
  const prefix = type[0].toLowerCase() + type.slice(1);
  test(`${prefix}Values / ${prefix}ParamKeys match Python ${type}`, () => {
    assert.deepEqual(features[`${prefix}Values`](), golden.values);
    for (const [op, keys] of Object.entries(golden.param_keys)) {
      assert.deepEqual(features[`${prefix}ParamKeys`](op), keys, op);
    }
    assert.equal(golden.unknown_error.exception, 'ValueError');
    assert.throws(
      () => features[`${prefix}ParamKeys`]('nope'),
      (error) =>
        error.kind === 'validation' && error.message.includes(core(golden.unknown_error.message))
    );
    assert.throws(() => features[`${prefix}ParamKeys`](1), TypeError);
  });
}

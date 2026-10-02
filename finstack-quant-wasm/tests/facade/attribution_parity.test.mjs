/**
 * Facade test for `ReturnContributionResult`, the plain-object twin of the
 * Python `ReturnContributionResult` class (parity slice P3).
 *
 * Expected values come from
 * `finstack-quant-py/tests/data/attribution_wasm_parity.json`, which holds the
 * Python output for the same spec and is asserted by
 * `finstack-quant-py/tests/test_attribution_wasm_parity.py`, so the case is a
 * cross-host golden: both hosts call the same Rust entry point.
 *
 * Requires the wasm-pack web build: npm run build (mise run wasm-build).
 */

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { attribution } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

const GOLDEN = JSON.parse(
  readFileSync(
    new URL('../../../finstack-quant-py/tests/data/attribution_wasm_parity.json', import.meta.url),
    'utf8'
  )
);

test('attributeReturnContribution returns the Python ReturnContributionResult document', () => {
  const result = attribution.attributeReturnContribution(GOLDEN.spec);
  assert.deepEqual(JSON.parse(JSON.stringify(result)), GOLDEN.result);
  assert.deepEqual(
    JSON.parse(attribution.attributeReturnContributionJson(GOLDEN.spec)),
    GOLDEN.result
  );
  for (const key of Object.keys(result)) {
    assert.ok(GOLDEN.properties.includes(key), `${key} is a ReturnContributionResult property`);
  }
});

/**
 * Shared loader and comparison for the statements cross-host goldens.
 *
 * The inputs and the pinned outputs live beside the Python twin
 * (`finstack-quant-py/tests/test_statements_wasm_parity.py`), which writes the
 * pinned file; the facade tests assert the same documents.
 */

import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const PKG_DIR = join(__dirname, '..', '..', 'pkg');
const WASM_BG = join(PKG_DIR, 'finstack_quant_wasm_bg.wasm');
const FIXTURES = join(__dirname, '..', '..', '..', 'finstack-quant-py', 'tests', 'fixtures');

if (!existsSync(WASM_BG)) {
  throw new Error(
    `finstack-quant-wasm web build not found at ${WASM_BG}. Generate it with: npm run build`
  );
}

export const facade = await import('../../index.js');
await facade.default({ module_or_path: readFileSync(WASM_BG) });

export const INPUTS = JSON.parse(
  readFileSync(join(FIXTURES, 'statements_parity_inputs.json'), 'utf8')
);
export const EXPECTED = JSON.parse(
  readFileSync(join(FIXTURES, 'statements_parity_expected.json'), 'utf8')
);
export const Q1 = '2025Q1';
export const Q2 = '2025Q2';

/** `undefined` (absent) compares as JSON `null`, as the Python `None` does. */
export const orNull = (value) => (value === undefined ? null : value);

/** A `Money` wire object as `{amount: number, currency}`. */
export const money = (value) =>
  value === undefined ? null : { amount: Number(value.amount), currency: value.currency };

/** Deep equality with a relative tolerance on numbers (native vs wasm32). */
export function assertMatches(actual, expected, path) {
  if (typeof expected === 'number') {
    assert.equal(typeof actual, 'number', `${path}: ${JSON.stringify(actual)} is not a number`);
    const scale = Math.max(1, Math.abs(expected));
    assert.ok(Math.abs(actual - expected) <= 1e-12 * scale, `${path}: ${actual} != ${expected}`);
  } else if (Array.isArray(expected)) {
    assert.ok(Array.isArray(actual), `${path}: ${JSON.stringify(actual)} is not an array`);
    assert.equal(actual.length, expected.length, `${path}: length`);
    expected.forEach((value, index) => assertMatches(actual[index], value, `${path}[${index}]`));
  } else if (expected !== null && typeof expected === 'object') {
    assert.ok(actual !== null && typeof actual === 'object', `${path}: not an object`);
    assert.deepEqual(Object.keys(actual).sort(), Object.keys(expected).sort(), `${path}: keys`);
    for (const [key, value] of Object.entries(expected)) {
      assertMatches(actual[key], value, `${path}.${key}`);
    }
  } else {
    assert.equal(actual, expected, path);
  }
}

/** Plain JSON view of a facade result (drops `undefined`, keeps key order). */
export const plain = (value) => JSON.parse(JSON.stringify(value));

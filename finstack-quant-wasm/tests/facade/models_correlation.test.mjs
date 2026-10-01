import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { models } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

const correlation = models.correlation;

test('matrix dimensions reject fractional, non-finite and wrapped JavaScript numbers', () => {
  for (const n of [0.9, 1.9, -1, NaN, Infinity, -Infinity, 4294967296, 4294967297]) {
    assert.throws(() => correlation.validateCorrelationMatrix([1], n), /n must be.*integer/);
    assert.throws(() => correlation.nearestCorrelation([1], n), /n must be.*integer/);
  }
});

test('nearest correlation validates iteration counts before the WASM ABI converts them', () => {
  for (const maxIter of [0.9, -1, NaN, Infinity, -Infinity, 4294967296, 4294967297]) {
    assert.throws(
      () => correlation.nearestCorrelation([1], 1, maxIter),
      /maxIter must be.*integer/
    );
  }
});

test('overflowing square dimensions produce ordinary errors instead of WASM traps', () => {
  for (const operation of [correlation.validateCorrelationMatrix, correlation.nearestCorrelation]) {
    assert.throws(
      () => operation([], 65536),
      (error) => error instanceof Error && !(error instanceof WebAssembly.RuntimeError)
    );
  }
});

test('integer dimensions and canonical zero-size and zero-iteration behavior remain valid', () => {
  assert.equal(correlation.validateCorrelationMatrix([1], 1), undefined);
  assert.deepEqual(Array.from(correlation.nearestCorrelation([1], 1, 0)), [1]);
  assert.equal(correlation.validateCorrelationMatrix([], 0), undefined);
  assert.deepEqual(Array.from(correlation.nearestCorrelation([], 0)), []);
});

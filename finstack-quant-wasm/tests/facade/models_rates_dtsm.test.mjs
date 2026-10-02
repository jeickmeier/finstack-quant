import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const testDir = dirname(fileURLToPath(import.meta.url));
const wasmPath = join(testDir, '..', '..', 'pkg', 'finstack_quant_wasm_bg.wasm');

if (!existsSync(wasmPath)) {
  throw new Error(`WASM build not found at ${wasmPath}. Run mise run wasm-build.`);
}

const facade = await import('../../index.js');
await facade.default({ module_or_path: readFileSync(wasmPath) });

test('DTSM is exposed only under models.rates.dtsm', () => {
  assert.equal('nelsonSiegelYields' in facade.core, false);
  assert.equal(typeof facade.models.rates.dtsm.nelsonSiegelYields, 'function');

  const yields = facade.models.rates.dtsm.nelsonSiegelYields(
    0.7308,
    [0.03, -0.01, 0.005],
    [1, 5, 10]
  );
  const expected = [0.024045061287046227, 0.02853762303631049, 0.02931292600971276];
  assert.equal(yields.length, 3);
  Array.from(yields).forEach((value, i) => assert.ok(Math.abs(value - expected[i]) < 1e-15));
});

test('nelsonSiegelYields takes factors as one [level, slope, curvature] array like Rust and Python', () => {
  const { nelsonSiegelYields } = facade.models.rates.dtsm;
  assert.throws(
    () => nelsonSiegelYields(0.7308, [0.03, -0.01], [1, 5, 10]),
    (err) => err.kind === 'validation' && /exactly 3 entries/.test(err.message)
  );
  assert.throws(
    () => nelsonSiegelYields(0.7308, 0.03, [1, 5, 10]),
    (err) => err.kind === 'invalid_type'
  );
});

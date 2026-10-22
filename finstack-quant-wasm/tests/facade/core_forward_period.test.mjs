import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { core } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

test('forward period validates endpoints and arithmetic through Rust', () => {
  const curve = core.ForwardCurve.flat('USD-SOFR-3M', 0.25, '2025-01-01', 0.04);
  for (const [start, end] of [
    [1, 0.5],
    [NaN, 1],
    [0, NaN],
    [-Infinity, 1],
    [0, Infinity],
    [-1e308, 1e308],
  ]) {
    assert.throws(
      () => curve.ratePeriod(start, end),
      (error) => error.kind === 'validation'
    );
  }
  for (const time of [-1, 0, 1]) {
    assert.equal(curve.ratePeriod(time, time), curve.rate(time));
    assert.equal(curve.ratePeriod(time, time + 1e-13), curve.rate(time));
  }
  assert.ok(Math.abs(curve.ratePeriod(-1, -0.5) - 0.04) < 1e-14);
  curve.free();
});

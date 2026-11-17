import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { core } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

test('forward interval validates endpoints, accrual and arithmetic through Rust', () => {
  const curve = core.ForwardCurve.flat('USD-SOFR-3M', '2025-01-01', 0.04, 'act_360');
  try {
    for (const [start, end] of [
      [1, 0.5],
      [1, 1],
      [NaN, 1],
      [0, NaN],
      [-Infinity, 1],
      [0, Infinity],
    ]) {
      assert.throws(
        () => curve.rateBetween(start, end, 0.25),
        (error) => error.kind === 'validation' && /requires finite t1 < t2/.test(error.message)
      );
    }
    for (const accrual of [0, -0.25, NaN, Infinity]) {
      assert.throws(
        () => curve.rateBetween(0, 0.25, accrual),
        (error) => error.kind === 'validation' && /accrual_year_fraction/.test(error.message)
      );
    }
    for (const [start, end] of [
      [0, 1],
      [-1, -0.5],
      [2, 2.25],
    ]) {
      const tau = end - start;
      const expected = Math.expm1(0.04 * tau) / tau;
      assert.ok(Math.abs(curve.rateBetween(start, end, tau) - expected) < 1e-14);
    }
  } finally {
    curve.free();
  }
});

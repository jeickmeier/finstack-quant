/**
 * Sizes that overflow a 32-bit `usize` throw a structured error instead of
 * trapping the instance.
 *
 * On wasm32, `65536 * 65536` wraps to 0 in release builds. Each case below
 * used to pass a wrapped size check (or overflow an allocation) and abort the
 * whole WebAssembly instance with `RuntimeError: unreachable`; Rust now uses
 * checked arithmetic, so the same inputs throw a `validation` FinstackError,
 * exactly as they raise `ValueError` in Python.
 */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { core, models } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

const WRAPS = 65536; // 65536 * 65536 === 2 ** 32 wraps to 0 in a 32-bit usize

const validation = (error) => {
  assert.ok(!(error instanceof WebAssembly.RuntimeError), `trapped: ${error}`);
  assert.equal(error.name, 'FinstackError', String(error));
  assert.equal(error.kind, 'validation', error.message);
  return true;
};

test('Cholesky entry points reject a wrapping dimension', () => {
  const empty = new Float64Array(0);
  assert.throws(() => core.choleskyDecomposition(empty, WRAPS), validation);
  assert.throws(() => core.applyLowerTriangular(empty, WRAPS, new Float64Array(WRAPS)), validation);
  assert.throws(() => core.choleskySolve(empty, new Float64Array(WRAPS)), validation);
});

test('choleskySolve takes its dimension from b, as in Rust and Python', () => {
  // L = [[2, 0], [1, 1]] so A = L Lᵀ = [[4, 2], [2, 2]]; A x = [2, 1] gives x = [0.5, 0].
  const x = core.choleskySolve([2, 0, 1, 1], [2, 1]);
  assert.deepEqual(Array.from(x), [0.5, 0]);
  assert.throws(
    () => core.choleskySolve([2, 0, 1, 1], [2, 1, 0]),
    (error) => {
      validation(error);
      assert.match(error.message, /4 entries but a right-hand side of length 3 needs 9/);
      return true;
    }
  );
});

test('correlation checks reject a wrapping dimension', () => {
  const empty = new Float64Array(0);
  assert.throws(() => models.correlation.validateCorrelationMatrix(empty, WRAPS), validation);
  assert.throws(() => models.correlation.nearestCorrelation(empty, WRAPS), validation);
});

test('Merton path simulation rejects sizes that overflow or cannot be allocated', () => {
  const merton = new models.credit.MertonModel(100.0, 0.25, 80.0, 0.05);
  const simulate = (paths, steps) => merton.simulatePaths(paths, steps, 1.0, 7n, false);
  assert.throws(() => simulate(2 ** 32 - 1, 4), validation);
  assert.throws(() => simulate(1, 2 ** 32 - 1), validation);
  assert.throws(() => simulate(1e8, 4), validation);
  assert.equal(simulate(2, 4).assetValues.length, 2 * 5);
});

test('Student-t tail dependence propagates NaN instead of trapping', () => {
  const copula = models.correlation.CopulaSpec.studentT(5).build();
  assert.ok(Number.isNaN(copula.tailDependence(NaN)));
  assert.ok(copula.tailDependence(0.5) > 0);
});

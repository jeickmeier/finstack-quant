import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { models } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

const correlation = models.correlation;

/** The Rust `js_uint` boundary rejection for a count that is not a whole u32. */
const invalidCount = (name) => (error) => {
  assert.ok(error instanceof TypeError, `expected a TypeError, got ${error}`);
  assert.equal(error.kind, 'invalid_type');
  assert.match(
    error.message,
    new RegExp(`^${name}: (expected a non-negative whole number, got |\\d+ is out of range)`)
  );
  return true;
};

test('matrix dimensions reject fractional, non-finite and wrapped JavaScript numbers', () => {
  for (const n of [0.9, 1.9, -1, NaN, Infinity, -Infinity, 4294967296, 4294967297]) {
    assert.throws(() => correlation.validateCorrelationMatrix([1], n), invalidCount('n'));
    assert.throws(() => correlation.nearestCorrelation([1], n), invalidCount('n'));
  }
});

test('nearest correlation validates iteration counts before the WASM ABI converts them', () => {
  for (const maxIter of [0.9, -1, NaN, Infinity, -Infinity, 4294967296, 4294967297]) {
    assert.throws(() => correlation.nearestCorrelation([1], 1, maxIter), invalidCount('maxIter'));
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

test('checked latent construction preserves zero and uncapped finite inputs', () => {
  for (const vol of [0, 0.001, 3]) {
    const model = new correlation.LatentSingleFactor(vol, 20);
    assert.equal(model.volatility, vol);
    assert.equal(model.meanReversion, 20);
    const spec = correlation.LatentFactorSpec.singleFactor(vol, 20);
    assert.deepEqual(Array.from(spec.build().volatilities), [vol]);
    assert.deepEqual(
      Array.from(correlation.LatentMultiFactor.uncorrelated(1, [vol]).volatilities),
      [vol]
    );
  }
  for (const rho of [-1, 1]) {
    const model = new correlation.LatentTwoFactor(0, 3, rho);
    assert.equal(model.correlation, rho);
    assert.equal(model.choleskyL11, 0);
    assert.equal(model.creditVol, 3);
    assert.equal(model.prepayVol, 0);
  }
});

test('invalid factor inputs fail direct and specification construction', () => {
  for (const bad of [-0.1, NaN, Infinity, -Infinity]) {
    assert.throws(() => new correlation.LatentSingleFactor(bad, 0));
    assert.throws(() => new correlation.LatentSingleFactor(0, bad));
    assert.throws(() => correlation.LatentFactorSpec.singleFactor(bad, 0));
    assert.throws(() => correlation.LatentFactorSpec.singleFactor(0, bad));
  }
  for (const bad of [-1.1, 1.1, NaN, Infinity]) {
    assert.throws(() => new correlation.LatentTwoFactor(0.2, 0.3, bad));
    assert.throws(() => correlation.LatentFactorSpec.twoFactor(0.2, 0.3, bad));
  }
});

test('recovery construction and JSON reject out-of-range inputs', () => {
  for (const [mean, vol, rho] of [
    [-0.1, 0.2, 0.4],
    [1.1, 0.2, 0.4],
    [0.4, -0.1, 0.4],
    [0.4, 0.6, 0.4],
    [0.4, 0.2, -1.1],
    [0.4, 0.2, 1.1],
  ]) {
    assert.throws(() => correlation.RecoverySpec.marketCorrelated(mean, vol, rho));
    assert.throws(() =>
      correlation.RecoverySpec.fromJson(
        JSON.stringify({
          type: 'market_correlated',
          mean_recovery: mean,
          recovery_volatility: vol,
          factor_correlation: rho,
        })
      )
    );
  }
  for (const rho of [-1, 1]) {
    const spec = correlation.RecoverySpec.marketCorrelated(0.4, 0.5, rho);
    assert.equal(correlation.RecoverySpec.fromJson(spec.toJson()).toJson(), spec.toJson());
    assert.ok(Number.isFinite(spec.build().expectedRecovery));
  }
});

test('exposure JSON cannot bypass checked construction', () => {
  for (const wire of [
    { drawn: -1, undrawn: 2, ccf: { ccf: 0.75 } },
    { drawn: 1, undrawn: -2, ccf: { ccf: 0.75 } },
    { drawn: 1, undrawn: 2, ccf: { ccf: 1.1 } },
  ])
    assert.throws(() => models.credit.lgd.EadCalculator.fromJson(JSON.stringify(wire)));
  assert.equal(
    models.credit.lgd.EadCalculator.fromJson('{"drawn":100,"undrawn":20,"ccf":{"ccf":0.75}}').ead,
    115
  );
});

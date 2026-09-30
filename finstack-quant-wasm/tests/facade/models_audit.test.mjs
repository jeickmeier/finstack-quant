import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

const facade = await import('../../index.js');
await facade.default({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

test('checked Black-Scholes rejects invalid spot inputs', () => {
  for (const spot of [NaN, Infinity, -100, 0]) {
    assert.throws(() => facade.models.bsPrice(spot, 100, 0.05, 0, 0.2, 1, true));
  }
});

test('COS rejects an empty expansion', () => {
  assert.throws(() => facade.models.bsCosPrice(100, 100, 0.05, 0, 0.2, 1, true, 0));
});

test('seasoned lookbacks match independent continuous-maximum payoff values', () => {
  for (const [strikeType, isCall, expected] of [
    ['fixed', true, 24.326644378668],
    ['floating', false, 21.429719498063],
  ]) {
    const price = facade.models.lookbackOptionPrice(
      100,
      100,
      0.05,
      0.02,
      0.2,
      1,
      120,
      strikeType,
      isCall
    );
    assert.ok(Math.abs(price - expected) < 1e-8, `${strikeType}: ${price} vs ${expected}`);
  }
});

test('tranche ES preserves empirical tail probability under replication', () => {
  const { PortfolioLossResult } = facade.models.correlation;
  const losses = [0, 0, 10, 20];
  const result = PortfolioLossResult.fromLosses(losses, 0.75).trancheLossStatistics(0, 1, 100);
  const repeated = PortfolioLossResult.fromLosses(
    Array(10).fill(losses).flat(),
    0.75
  ).trancheLossStatistics(0, 1, 100);
  assert.ok(Math.abs(result.expected_shortfall_amount - 20) < 1e-12);
  assert.ok(
    Math.abs(result.expected_shortfall_amount - repeated.expected_shortfall_amount) < 1e-12
  );
});

// Same numbers as finstack-quant-py/tests/test_models_correlation_portfolio_loss.py
// (`PortfolioLossResult.from_losses`): both hosts bind the Rust type.
test('PortfolioLossResult mirrors the Rust handle and validates its JSON', () => {
  const { PortfolioLossResult } = facade.models.correlation;
  assert.equal('trancheLossStatistics' in facade.models.correlation, false);
  const result = PortfolioLossResult.fromLosses(new Float64Array([0, 1, 2, 5, 10]), 0.75);
  assert.ok(result.losses instanceof Float64Array);
  assert.deepEqual(Array.from(result.losses), [0, 1, 2, 5, 10]);
  assert.ok(Math.abs(result.expectedLoss - 3.6) < 1e-12);
  assert.equal(result.var, 5);
  assert.equal(result.confidence, 0.75);
  assert.ok(Math.abs(result.expectedShortfall - 9) < 1e-12);

  const tranche = result.trancheLossStatistics(0.02, 0.06, 100);
  assert.deepEqual(Object.keys(tranche).sort(), [
    'attachment',
    'detachment',
    'expected_loss_amount',
    'expected_loss_fraction',
    'expected_shortfall_amount',
    'expected_shortfall_fraction',
    'prob_attachment_breached',
    'prob_full_writedown',
    'tranche_notional',
    'var_amount',
    'var_fraction',
  ]);

  const json = result.toJson();
  const loaded = PortfolioLossResult.fromJson(json);
  assert.equal(loaded.toJson(), json);
  const doc = JSON.parse(json);
  assert.throws(
    () => PortfolioLossResult.fromJson({ ...doc, var: 99 }),
    (e) => e instanceof Error && e.kind === 'validation'
  );
  assert.throws(
    () => PortfolioLossResult.fromJson({ ...doc, losses: [-1, 2, 3, 4, 5] }),
    (e) => e instanceof Error && e.kind === 'validation'
  );
  assert.throws(
    () => PortfolioLossResult.fromLosses([], 0.75),
    (e) => e instanceof Error && e.kind === 'validation'
  );
  assert.throws(
    () => PortfolioLossResult.fromLosses('[1,2]', 0.75),
    (e) => e instanceof TypeError && e.kind === 'invalid_type'
  );
});

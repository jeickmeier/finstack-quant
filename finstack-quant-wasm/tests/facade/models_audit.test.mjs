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
  const losses = [0, 0, 10, 20];
  const result = facade.models.correlation.trancheLossStatistics(losses, 0.75, 0, 1, 100);
  const repeated = facade.models.correlation.trancheLossStatistics(
    Array(10).fill(losses).flat(),
    0.75,
    0,
    1,
    100
  );
  assert.ok(Math.abs(result.expected_shortfall_amount - 20) < 1e-12);
  assert.ok(
    Math.abs(result.expected_shortfall_amount - repeated.expected_shortfall_amount) < 1e-12
  );
});

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { models } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

test('high-intensity zero-size structural jumps preserve GBM default probability', () => {
  const credit = models.credit;
  const diffusion = credit.mertonModelJson(100, 0.2, 80, 0.05);
  const jumps = credit.mertonModelWithDynamicsJson(
    100,
    0.2,
    80,
    0.05,
    0,
    JSON.stringify('terminal'),
    JSON.stringify({
      jump_diffusion: { jump_intensity: 746, jump_mean: 0, jump_vol: 0 },
    })
  );

  const expected = credit.mertonDefaultProbability(diffusion, 1);
  assert.ok(expected > 0.01);
  assert.ok(Math.abs(credit.mertonDefaultProbability(jumps, 1) - expected) < 1e-11);
});

for (const mean of [0.01, 0.99]) {
  test(`zero-volatility recovery preserves location ${mean}`, () => {
    const spec = models.correlation.RecoverySpec.marketCorrelated(mean, 0, 0);
    const recovery = spec.build();

    assert.ok(Math.abs(spec.expectedRecovery - mean) < 1e-14);
    assert.ok(Math.abs(recovery.expectedRecovery - mean) < 1e-14);
    for (const factor of [-6, 0, 6]) {
      assert.ok(Math.abs(recovery.conditionalRecovery(factor) - mean) < 1e-14);
      assert.ok(Math.abs(recovery.conditionalLgd(factor) - (1 - mean)) < 1e-14);
    }

    recovery.free();
    spec.free();
  });
}

test('near-boundary recovery expectation resolves the steep Gaussian curve', () => {
  const spec = models.correlation.RecoverySpec.marketCorrelated(0.01, 0.25, 0.4);
  const recovery = spec.build();
  try {
    assert.ok(Math.abs(recovery.expectedRecovery - 0.3271433591834951) < 1e-10);
  } finally {
    recovery.free();
    spec.free();
  }
});

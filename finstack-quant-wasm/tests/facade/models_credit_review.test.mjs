import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { models } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

test('high-intensity zero-size structural jumps preserve GBM default probability', () => {
  const credit = models.credit;
  const diffusion = new credit.MertonModel(100, 0.2, 80, 0.05);
  const barrier = credit.MertonBarrierType.terminal();
  const dynamics = credit.AssetDynamics.jumpDiffusion(746, 0, 0);
  const jumps = credit.MertonModel.newWithDynamics(100, 0.2, 80, 0.05, 0, barrier, dynamics);
  try {
    const expected = diffusion.defaultProbability(1);
    assert.ok(expected > 0.01);
    assert.ok(Math.abs(jumps.defaultProbability(1) - expected) < 1e-11);
  } finally {
    jumps.free();
    dynamics.free();
    barrier.free();
    diffusion.free();
  }
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

test('labeled transition matrices preserve probabilities and validate every label order', () => {
  const migration = models.credit.migration;
  const labels = ['A', 'B', 'D'];
  const rows = [
    [0.9, 0.08, 0.02],
    [0.1, 0.8, 0.1],
    [0, 0, 1],
  ];
  const scale = migration.RatingScale.custom(labels);
  const inferred = migration.TransitionMatrix.fromLabeled(labels, labels, rows, 1);
  assert.deepEqual(inferred.toRows(), rows);
  assert.deepEqual(inferred.scale.labels(), labels);
  assert.equal(inferred.scale.defaultState(), 2);
  assert.equal(inferred.probability('A', 'B'), 0.08);
  for (const suppliedScale of [scale.toJson(), JSON.parse(scale.toJson())]) {
    const matrix = migration.TransitionMatrix.fromLabeled(
      labels,
      labels,
      rows.flat(),
      1,
      suppliedScale
    );
    assert.deepEqual(matrix.toRows(), rows);
    assert.equal(matrix.probability('A', 'B'), 0.08);
    assert.throws(
      () =>
        migration.TransitionMatrix.fromLabeled(
          ['B', 'A', 'D'],
          ['B', 'A', 'D'],
          rows,
          1,
          suppliedScale
        ),
      /scale mismatch/
    );
  }
  assert.throws(
    () => migration.TransitionMatrix.fromLabeled(labels, ['B', 'A', 'D'], rows, 1),
    /scale mismatch/
  );
  assert.throws(
    () =>
      migration.TransitionMatrix.fromLabeled(
        ['A', 'D'],
        ['A', 'D'],
        [
          [0.9, 0.1],
          [0, 1],
        ],
        1,
        scale.toJson()
      ),
    /scale mismatch/
  );
  assert.throws(
    () => migration.TransitionMatrix.fromLabeled(['A', 'A', 'D'], ['A', 'A', 'D'], rows, 1),
    /duplicate/
  );
});

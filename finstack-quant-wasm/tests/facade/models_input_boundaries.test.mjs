import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

const facade = await import('../../index.js');
await facade.default({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});
const { models } = facade;

test('forward option APIs validate input domains before intrinsic branches', () => {
  for (const invalid of [NaN, Infinity, -Infinity]) {
    for (const expiry of [0, 1]) {
      assert.throws(() => models.black76Price(invalid, 100, 0.95, expiry, 0.2, true));
      assert.throws(() => models.black76Greeks(invalid, 100, expiry, 0.2, true));
      assert.throws(() => models.bachelierPrice(invalid, 100, 0.2, expiry, true));
      assert.throws(() => models.bachelierGreeks(invalid, 100, 0.2, expiry, true));
      assert.throws(() => models.blackShiftedPrice(-0.01, -0.01, 0.2, expiry, invalid, true));
      assert.throws(() => models.blackShiftedVega(-0.01, -0.01, 0.2, expiry, invalid));
    }
  }
  for (const df of [0, -0.95, NaN, Infinity]) {
    assert.throws(() => models.black76Price(100, 100, df, 1, 0.2, true));
  }
  for (const [vol, expiry] of [
    [-0.2, 1],
    [0.2, -1],
  ]) {
    assert.throws(() => models.black76Price(100, 100, 0.95, expiry, vol, true));
    assert.throws(() => models.black76Greeks(100, 100, expiry, vol, true));
    assert.throws(() => models.bachelierPrice(100, 100, vol, expiry, true));
    assert.throws(() => models.bachelierGreeks(100, 100, vol, expiry, true));
    assert.throws(() => models.blackShiftedPrice(-0.01, -0.01, vol, expiry, 0.02, true));
    assert.throws(() => models.blackShiftedVega(-0.01, -0.01, vol, expiry, 0.02));
  }
  assert.equal(models.black76Price(110, 100, 0.95, 1, 0, true), 9.5);
  assert.equal(models.bachelierPrice(-0.01, -0.02, 0, 1, true), 0.01);
  assert.ok(Math.abs(models.blackShiftedPrice(-0.01, -0.02, 0, 1, 0.03, true) - 0.01) < 1e-15);
});

test('models reject fractional, non-finite and wrapped counts before the WASM ABI', () => {
  const invalidCounts = [
    NaN,
    Infinity,
    -Infinity,
    -1,
    1.5,
    4294967296,
    4294967297,
    9007199254740992,
  ];
  const model = models.credit.mertonModelJson(100, 0.2, 80, 0.05);
  const calibrator = new models.volatility.SabrCalibrator();
  try {
    for (const count of invalidCounts) {
      assert.throws(() => models.asianOptionPrice(100, 100, 0.05, 0, 0.2, 1, count), /integer/);
      assert.throws(() => models.bsCosPrice(100, 100, 0.05, 0, 0.2, 1, true, count), /integer/);
      assert.throws(
        () => models.vgCosPrice(100, 100, 0.05, 0, 0.2, -0.1, 0.2, 1, true, count),
        /integer/
      );
      assert.throws(
        () => models.mertonJumpCosPrice(100, 100, 0.05, 0, 0.2, -0.1, 0.1, 0.1, 1, true, count),
        /integer/
      );
      for (const price of [models.monteCarlo.priceHestonCall, models.monteCarlo.priceHestonPut]) {
        assert.throws(
          () => price(100, 100, 0.05, 0, 2, 0.04, 0.3, -0.7, 0.04, 1, count, 42n, 1),
          /integer/
        );
        assert.throws(
          () => price(100, 100, 0.05, 0, 2, 0.04, 0.3, -0.7, 0.04, 1, 2, 42n, count),
          /integer/
        );
      }
      assert.throws(
        () => models.credit.mertonSimulatePathsJson(model, count, 1, 1, 42n, false),
        /integer/
      );
      assert.throws(
        () => models.credit.mertonSimulatePathsJson(model, 2, count, 1, 42n, false),
        /integer/
      );
      assert.throws(
        () => models.credit.toggleExerciseOptimalJson(count, 0.1, 0.25, 0.04, 5),
        /integer/
      );
      assert.throws(() => calibrator.withMaxIterations(count), /integer/);
    }
    const estimate = models.monteCarlo.priceHestonCall(
      100,
      100,
      0.05,
      0,
      2,
      0.04,
      0.3,
      -0.7,
      0.04,
      1,
      2,
      42n,
      1
    );
    assert.equal(estimate.num_paths, 2);
    assert.ok(Number.isFinite(estimate.mean));
  } finally {
    calibrator.free();
  }
});

test('stored expiry and hierarchy indices cannot silently select another grid row', () => {
  const surface = new facade.core.FxDeltaVolSurface('FX', [1], [0.12], [0.01], [0.002]);
  const levels = models.factor.credit.LevelsAtDate.fromJson(
    JSON.stringify({
      date: '2025-01-01',
      generic: 100,
      by_level: [{ level_index: 0, dimension: 'rating', values: { IG: 10 } }],
      adder: {},
    })
  );
  const deltas = models.factor.credit.PeriodDecomposition.fromJson(
    JSON.stringify({
      from: '2025-01-01',
      to: '2025-01-02',
      d_generic: 1,
      by_level: [{ level_index: 0, dimension: 'rating', deltas: { IG: 2 } }],
      d_adder: {},
    })
  );
  try {
    assert.deepEqual(levels.levelValues(0), { IG: 10 });
    assert.deepEqual(deltas.levelDeltas(0), { IG: 2 });
    for (const index of [0.5, NaN, Infinity, -1, 4294967296]) {
      assert.throws(() => models.volatility.getFxDeltaPillarVols(surface, index), /integer/);
      assert.throws(() => levels.levelValues(index), /integer/);
      assert.throws(() => deltas.levelDeltas(index), /integer/);
    }
  } finally {
    surface.free();
    levels.free();
    deltas.free();
  }
});

test('RFL tail dependence reports the calibrated unit-loading mass', () => {
  const spec = models.correlation.CopulaSpec.randomFactorLoading(0.2);
  const copula = spec.build();
  try {
    assert.equal(copula.tailDependence(0), 0);
    assert.equal(copula.tailDependence(1), 1);
    const interior = copula.tailDependence(0.3);
    assert.ok(interior > 0 && interior < 1);
  } finally {
    copula.free();
    spec.free();
  }
});

test('credit model JSON cannot bypass recovery and hazard validation', () => {
  const recovery = JSON.parse(models.credit.dynamicRecoveryConstantJson(0.4));
  for (const invalid of [-0.1, 1.5]) {
    assert.throws(() =>
      models.credit.dynamicRecoveryAtNotional(
        JSON.stringify({ ...recovery, base_recovery: invalid }),
        100
      )
    );
  }
  const hazard = JSON.parse(models.credit.endogenousHazardPowerLawJson(0.1, 1.5, 2));
  hazard.leverage_hazard_map = { tabular: { leverage_points: [], hazard_points: [] } };
  assert.throws(() => models.credit.endogenousHazardAtLeverage(JSON.stringify(hazard), 2));
});

test('risk budget rejects invalid finite-share and threshold inputs', () => {
  const risk = models.factor.risk;
  assert.throws(() => risk.evaluateRiskBudget('["A", "B"]', '[1, 1]', '[-0.5, 1.5]', 2));
  for (const invalid of [NaN, Infinity, -Infinity]) {
    assert.throws(() => risk.evaluateRiskBudget('["A"]', '[1]', '[1]', invalid));
    assert.throws(() => risk.evaluateRiskBudget('["A"]', '[1]', '[1]', 1, invalid));
  }
  assert.throws(() => risk.evaluateRiskBudget('["A"]', '[1]', '[1]', 1, 0));
});

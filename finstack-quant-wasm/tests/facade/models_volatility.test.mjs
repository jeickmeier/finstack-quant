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

test('volatility engines are exposed only under models.volatility', () => {
  assert.equal('SabrParameters' in facade.models, false);
  assert.equal('SabrModel' in facade.models, false);
  assert.equal('SabrSmile' in facade.models, false);
  assert.equal('SabrCalibrator' in facade.models, false);

  const volatility = facade.models.volatility;
  for (const name of [
    'SabrParameters',
    'SabrModel',
    'SabrSmile',
    'SabrCalibrator',
    'getCubeVol',
    'getCubeVolClamped',
    'getCubeNormalVol',
    'getCubeNormalVolClamped',
    'getFxDeltaPillarVols',
    'getFxDeltaVol',
    'deltaToStrike',
    'strikeToDelta',
  ]) {
    assert.ok(name in volatility, `missing models.volatility.${name}`);
  }
});

test('models.volatility evaluates core data artifacts', () => {
  const cube = new facade.core.VolCube(
    'USD-SWAPTION',
    [1],
    [5],
    [0.03, 0.5, -0.2, 0.4, Number.NaN],
    [0.03]
  );
  const vol = facade.models.volatility.getCubeVol(cube, 1, 5, 0.03);
  assert.ok(Number.isFinite(vol));
  assert.ok(vol > 0);

  const surface = new facade.core.FxDeltaVolSurface('EURUSD-VOL', [1], [0.12], [0.01], [0.002]);
  const pillars = facade.models.volatility.getFxDeltaPillarVols(surface, 0);
  assert.deepEqual(
    Array.from(pillars).map((value) => Number(value.toFixed(6))),
    [0.12, 0.117, 0.127]
  );

  surface.free();
  cube.free();
});

test('normal SABR beta zero uses arithmetic moneyness across zero', () => {
  const { SabrParameters, SabrModel } = facade.models.volatility;
  const alpha = 0.008, nu = 0.7, rho = -0.35, expiry = 2;
  const params = new SabrParameters(alpha, 0, nu, rho);
  const model = new SabrModel(params);
  for (const [forward, strike] of [[-0.01, -0.006], [-0.002, 0.002], [0.01, 0.014]]) {
    const z = nu * (forward - strike) / alpha;
    const chi = Math.log((Math.sqrt(1 - 2 * rho * z + z * z) + z - rho) / (1 - rho));
    const expected = alpha * z / chi * (1 + (2 - 3 * rho * rho) * nu * nu * expiry / 24);
    assert.ok(Math.abs(model.impliedVol(forward, strike, expiry) - expected) < 1e-14);
  }
  model.free();
  params.free();
});

test('SABR calibration rejects a poor final quote fit', () => {
  const calibrator = new facade.models.volatility.SabrCalibrator();
  assert.throws(() => calibrator.calibrate(100, new Float64Array([90, 95, 100, 105, 110]),
    new Float64Array([0.22, 0.20, 0.19, 0.195, 0.21]), 1, 0.5), /no acceptable/);
  calibrator.free();
});

test('clamped cube preserves small quotes and invalid model domains', () => {
  const low = new facade.core.VolCube('LOW', [1], [5], [0.0001, 1, 0, 0.3, Number.NaN], [0.02]);
  const checked = facade.models.volatility.getCubeVol(low, 1, 5, 0.02);
  const clamped = facade.models.volatility.getCubeVolClamped(low, 1, 5, 0.02);
  assert.ok(Math.abs(clamped - checked) < 1e-14);
  const invalid = new facade.core.VolCube('INVALID', [1], [5], [0.008, 0.5, 0, 0.3, Number.NaN], [-0.01]);
  assert.ok(Number.isNaN(facade.models.volatility.getCubeVolClamped(invalid, 1, 5, -0.01)));
  low.free();
  invalid.free();
});

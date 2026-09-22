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
  const alpha = 0.008,
    nu = 0.7,
    rho = -0.35,
    expiry = 2;
  const params = new SabrParameters(alpha, 0, nu, rho);
  const model = new SabrModel(params);
  for (const [forward, strike] of [
    [-0.01, -0.006],
    [-0.002, 0.002],
    [0.01, 0.014],
  ]) {
    const z = (nu * (forward - strike)) / alpha;
    const chi = Math.log((Math.sqrt(1 - 2 * rho * z + z * z) + z - rho) / (1 - rho));
    const expected = ((alpha * z) / chi) * (1 + ((2 - 3 * rho * rho) * nu * nu * expiry) / 24);
    assert.ok(Math.abs(model.impliedVol(forward, strike, expiry) - expected) < 1e-14);
  }
  model.free();
  params.free();
});

test('SABR calibration rejects a poor final quote fit', () => {
  const calibrator = new facade.models.volatility.SabrCalibrator();
  assert.throws(
    () =>
      calibrator.calibrate(
        100,
        new Float64Array([90, 95, 100, 105, 110]),
        new Float64Array([0.22, 0.2, 0.19, 0.195, 0.21]),
        1,
        0.5
      ),
    /no acceptable/
  );
  calibrator.free();
});

test('clamped cube preserves small quotes and invalid model domains', () => {
  const low = new facade.core.VolCube('LOW', [1], [5], [0.0001, 1, 0, 0.3, Number.NaN], [0.02]);
  const checked = facade.models.volatility.getCubeVol(low, 1, 5, 0.02);
  const clamped = facade.models.volatility.getCubeVolClamped(low, 1, 5, 0.02);
  assert.ok(Math.abs(clamped - checked) < 1e-14);
  const invalid = new facade.core.VolCube(
    'INVALID',
    [1],
    [5],
    [0.008, 0.5, 0, 0.3, Number.NaN],
    [-0.01]
  );
  assert.ok(Number.isNaN(facade.models.volatility.getCubeVolClamped(invalid, 1, 5, -0.01)));
  low.free();
  invalid.free();
});

test('canonical volatility state constructors preserve named parameters and optional wings', () => {
  const c = {
    id: 'STATE',
    expiries: [1],
    tenors: [5],
    params: [{ alpha: 0.03, beta: 0.5, rho: -0.2, nu: 0.4, shift: null }],
    forwards: [0.03],
    interpolation_mode: 'vol',
  };
  for (const shift of [null, 0.03]) {
    c.params[0].shift = shift;
    const fromJson = facade.core.VolCube.fromJson(JSON.stringify(c));
    const positional = new facade.core.VolCube(
      'STATE',
      [1],
      [5],
      [0.03, 0.5, -0.2, 0.4, shift ?? Number.NaN],
      [0.03],
      'vol'
    );
    try {
      assert.equal(
        facade.models.volatility.getCubeVol(fromJson, 1, 5, 0.025),
        facade.models.volatility.getCubeVol(positional, 1, 5, 0.025)
      );
      assert.equal(
        facade.models.volatility.getCubeNormalVol(fromJson, 1, 5, 0.025),
        facade.models.volatility.getCubeNormalVol(positional, 1, 5, 0.025)
      );
    } finally {
      fromJson.free();
      positional.free();
    }
  }
  assert.throws(
    () => facade.core.VolCube.fromJson(JSON.stringify({ ...c, unknown: true })),
    (error) => error.kind === 'validation'
  );
  assert.throws(
    () =>
      facade.core.VolCube.fromJson(
        JSON.stringify({ ...c, params: [{ ...c.params[0], alpha: -1 }] })
      ),
    (error) => error.kind === 'validation'
  );
  const s = {
    id: 'FX-STATE',
    expiries: [1],
    atm_vols: [0.12],
    rr_25d: [0.01],
    bf_25d: [0.002],
    rr_10d: null,
    bf_10d: null,
  };
  for (const wings of [false, true]) {
    const state = { ...s, rr_10d: wings ? [0.02] : null, bf_10d: wings ? [0.004] : null };
    const fromJson = facade.core.FxDeltaVolSurface.fromJson(JSON.stringify(state));
    const positional = new facade.core.FxDeltaVolSurface(
      'FX-STATE',
      [1],
      [0.12],
      [0.01],
      [0.002],
      wings ? [0.02] : undefined,
      wings ? [0.004] : undefined
    );
    try {
      assert.equal(
        facade.models.volatility.getFxDeltaVol(fromJson, 1, 1.12, 1.1),
        facade.models.volatility.getFxDeltaVol(positional, 1, 1.12, 1.1)
      );
    } finally {
      fromJson.free();
      positional.free();
    }
  }
  assert.throws(
    () => facade.core.FxDeltaVolSurface.fromJson(JSON.stringify({ ...s, rr_10d: [0.02] })),
    (error) => error.kind === 'validation'
  );
  assert.throws(
    () => facade.core.FxDeltaVolSurface.fromJson(JSON.stringify({ ...s, unknown: true })),
    (error) => error.kind === 'validation'
  );
});

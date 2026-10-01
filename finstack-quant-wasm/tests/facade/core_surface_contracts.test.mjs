import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

const facade = await import('../../index.js');
await facade.default({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

test('zero vol-of-vol SABR nodes retain the exact Black cube limit', () => {
  const node = [0.2, 1, -0.5, 0, Number.NaN];
  const cube = new facade.core.VolCube(
    'BLACK',
    [1, 5],
    [2, 10],
    [...node, ...node, ...node, ...node],
    [0.03, 0.03, 0.03, 0.03]
  );
  const restored = facade.core.VolCube.fromJson(
    JSON.stringify({
      id: 'BLACK-JSON',
      expiries: [1, 5],
      tenors: [2, 10],
      params: Array.from({ length: 4 }, () => ({ alpha: 0.2, beta: 1, rho: -0.5, nu: 0 })),
      forwards: [0.03, 0.03, 0.03, 0.03],
      interpolation_mode: 'vol',
    })
  );
  try {
    for (const strike of [0.02, 0.03, 0.04]) {
      for (const source of [cube, restored]) {
        const vol = facade.models.volatility.getCubeVol(source, 3, 6, strike);
        assert.ok(Math.abs(vol - 0.2) < 1e-14, `K=${strike}: ${vol}`);
      }
    }
  } finally {
    restored.free();
    cube.free();
  }
});

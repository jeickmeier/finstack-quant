/**
 * Runtime contract for the raw `Performance.periodicReturns` panel.
 *
 * Requires the wasm-pack web build: npm run build (mise run wasm-build).
 */

import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const PKG_DIR = join(__dirname, '..', '..', 'pkg');
const WASM_BG = join(PKG_DIR, 'finstack_quant_wasm_bg.wasm');

if (!existsSync(WASM_BG)) {
  throw new Error(
    `finstack-quant-wasm web build not found at ${WASM_BG}. Generate it with: npm run build`
  );
}

const facade = await import('../../index.js');
const init = facade.default;
const { analytics } = facade;

await init({ module_or_path: readFileSync(WASM_BG) });

test('periodicReturns exposes ticker-major dated points that reconcile', () => {
  const dates = ['2025-01-30', '2025-01-31', '2025-02-03'];
  const returns = [
    [0.01, 0.02, -0.01],
    [0.005, -0.002, 0.015],
  ];
  const perf = analytics.Performance.fromReturns(
    dates,
    returns,
    ['FUND', 'BENCH'],
    'BENCH',
    'daily'
  );

  assert.equal(typeof perf.periodicReturns, 'function');
  const panel = perf.periodicReturns();
  assert.equal(panel.length, 2);

  const cumulative = perf.cumulativeReturns();
  for (const [tickerIdx, points] of panel.entries()) {
    assert.deepEqual(
      points.map(({ date }) => date),
      ['2025-01-31', '2025-02-03']
    );
    assert.ok(
      points.every(({ date, value }) => typeof date === 'string' && typeof value === 'number')
    );

    const chained = points.reduce((wealth, { value }) => wealth * (1 + value), 1) - 1;
    assert.ok(Math.abs(chained - cumulative[tickerIdx].at(-1)) < 1e-12);
  }
});

test('periodicReturns rejects unsupported frequency tokens', () => {
  const perf = analytics.Performance.fromReturns(
    ['2025-01-30', '2025-01-31'],
    [[0.01, 0.02]],
    ['FUND']
  );
  assert.throws(() => perf.periodicReturns('hourly'), /unknown period kind 'hourly'/);
});

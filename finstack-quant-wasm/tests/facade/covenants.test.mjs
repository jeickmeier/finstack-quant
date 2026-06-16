/**
 * Covenants-namespace facade smoke tests.
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
const { covenants } = facade;

await init({ module_or_path: readFileSync(WASM_BG) });

test('covenants namespace exposes JSON bridge functions', () => {
  assert.equal(typeof covenants.validateCovenantSpec, 'function');
  assert.equal(typeof covenants.validateCovenantReport, 'function');
  assert.equal(typeof covenants.validateCovenantEngine, 'function');
  assert.equal(typeof covenants.evaluateEngine, 'function');
  assert.equal(typeof covenants.lboStandard, 'function');
  assert.equal(typeof covenants.covLite, 'function');
  assert.equal(typeof covenants.realEstate, 'function');
  assert.equal(typeof covenants.projectFinance, 'function');
});

test('covenants facade generates and evaluates template JSON', () => {
  const specs = JSON.parse(covenants.lboStandard(5.0, 1.5, 1.2, 10_000_000.0));
  const engine = JSON.stringify({
    specs: [specs[0]],
    breach_history: [],
    windows: [],
    waivers: [],
  });
  const canonical = covenants.validateCovenantEngine(engine);
  const reports = JSON.parse(
    covenants.evaluateEngine(canonical, JSON.stringify({ debt_to_ebitda: 4.0 }), '2026-03-31')
  );

  assert.equal(reports.max_debt_ebitda.passed, true);
});

test('covenants facade rejects unknown validation fields', () => {
  const engine = JSON.stringify({
    specs: [],
    breach_history: [],
    windows: [],
    waviers: [],
  });

  assert.throws(() => covenants.validateCovenantEngine(engine), /unknown field/);
});

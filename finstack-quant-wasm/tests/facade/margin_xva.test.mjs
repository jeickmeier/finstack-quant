/**
 * Margin-namespace XVA facade runtime tests.
 *
 * Loads the public facade (`index.js` + `exports/margin.js` + `exports/core.js`),
 * initializes the web-target wasm module from bytes, and exercises
 * `margin.computeBilateralXva` end-to-end through the new `core.HazardCurve`.
 *
 * The zero-hazard fixture is byte-equivalent to the Rust integration test
 * `toy_forward_valuer_path_im_flows_into_total_xva` and to
 * `finstack-quant-py/tests/test_margin_mva.py::test_compute_bilateral_xva_aggregates_mva_into_total`,
 * so the Rust, Python and WASM surfaces produce the same MVA figure
 * (cross-language determinism).
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
const { core, margin } = facade;

await init({ module_or_path: readFileSync(WASM_BG) });

// Flat DF = 1 out to 4y, so discounting is a no-op and the arithmetic is
// hand-checkable.
const flatDiscount = () =>
  new core.DiscountCurve(
    'USD-OIS',
    '2025-01-01',
    [0.0, 1.0, 1.0, 1.0, 2.0, 1.0, 3.0, 1.0, 4.0, 1.0],
    'log_linear'
  );

const flatHazard = (lambda) =>
  new core.HazardCurve('HZ', '2025-01-01', [0.0, lambda, 30.0, lambda], 0.4);

const exposureJson = JSON.stringify({
  times: [1.0, 2.0],
  mtm_values: [1e6, 1e6],
  epe: [1e6, 1e6],
  ene: [0.0, 0.0],
});

test('margin namespace exports computeBilateralXva as a live function', () => {
  assert.equal(typeof margin.computeBilateralXva, 'function');
});

test('core namespace exports HazardCurve as a live constructor', () => {
  assert.equal(typeof core.HazardCurve, 'function');
  const hz = flatHazard(0.02);
  assert.equal(hz.id, 'HZ');
  assert.equal(hz.baseDate, '2025-01-01');
  assert.ok(Math.abs(hz.recoveryRate - 0.4) < 1e-12);
  // S(t) = exp(-0.02 t) for a flat 200bp intensity.
  assert.ok(Math.abs(hz.sp(1.0) - Math.exp(-0.02)) < 1e-9);
  assert.ok(Math.abs(hz.hazardRate(1.0) - 0.02) < 1e-9);
});

test('HazardCurve rejects missing recovery', () => {
  assert.throws(
    () => new core.HazardCurve('HZ', '2025-01-01', [0.0, 0.02, 30.0, 0.02]),
    /recovery/i
  );
});

test('HazardCurve recovery boundaries round-trip', () => {
  for (const recovery of [0.0, 1.0]) {
    const hz = new core.HazardCurve(
      `HZ-${recovery}`,
      '2025-01-01',
      [0.0, 0.02, 30.0, 0.02],
      recovery
    );
    assert.equal(hz.recoveryRate, recovery);
  }
});

test('computeBilateralXva folds MVA into total_xva', () => {
  const noDefault = flatHazard(0.0);
  const result = margin.computeBilateralXva(
    exposureJson,
    noDefault,
    noDefault,
    flatDiscount(),
    0.4,
    0.4,
    JSON.stringify({
      funding_spread_bp: 50.0,
      funding_benefit_bp: 30.0,
      im_profile: { times: [1.0, 2.0], im_values: [1e6, 1e6] },
    })
  );

  // Zero hazard ⇒ no credit legs; MVA is exactly the hand-checked 10_000
  // (flat 1e6 IM, 50bp, DF = 1, grid [1, 2]) — same figure as Rust/Python.
  assert.ok(Math.abs(result.cva) < 1e-12);
  assert.ok(Math.abs(result.dva) < 1e-12);
  assert.ok(Math.abs(result.mva - 10_000.0) < 1e-6);
  assert.ok(result.fva > 0.0);
  assert.ok(
    Math.abs(result.total_xva - (result.cva - result.dva + result.fva + result.mva)) < 1e-9
  );
});

test('computeBilateralXva nets posted IM from bilateral DVA', () => {
  const negativeExposureJson = JSON.stringify({
    times: [1.0, 2.0],
    mtm_values: [-1e6, -1e6],
    epe: [0.0, 0.0],
    ene: [1e6, 1e6],
  });
  const args = [negativeExposureJson, flatHazard(0.0), flatHazard(0.03), flatDiscount(), 0.4, 0.4];

  const withoutIm = margin.computeBilateralXva(...args);
  const withIm = margin.computeBilateralXva(
    ...args,
    JSON.stringify({
      funding_spread_bp: 0.0,
      funding_benefit_bp: 0.0,
      im_profile: { times: [1.0, 2.0], im_values: [400_000.0, 400_000.0] },
    })
  );

  assert.ok(withoutIm.dva > 0.0);
  assert.ok(withIm.dva > 0.0 && withIm.dva < withoutIm.dva);
});

test('computeBilateralXva rejects an IM/exposure horizon mismatch', () => {
  assert.throws(
    () =>
      margin.computeBilateralXva(
        exposureJson,
        flatHazard(0.02),
        flatHazard(0.03),
        flatDiscount(),
        0.4,
        0.4,
        JSON.stringify({
          funding_spread_bp: 50.0,
          im_profile: { times: [1.0, 3.0], im_values: [1e6, 1e6] },
        })
      ),
    /horizon/
  );
});

test('computeBilateralXva omits uncomputed legs without funding', () => {
  const result = margin.computeBilateralXva(
    exposureJson,
    flatHazard(0.02),
    flatHazard(0.03),
    flatDiscount(),
    0.4,
    0.4
  );

  // Optional legs are skipped on the wire when not computed.
  assert.equal(result.fva, undefined);
  assert.equal(result.mva, undefined);
  assert.ok(result.cva > 0.0);
  // With no funding legs, the all-in total is CVA minus DVA.
  assert.ok(Math.abs(result.total_xva - (result.cva - result.dva)) < 1e-12);
});

test('computeBilateralXva throws on an out-of-range recovery rate', () => {
  assert.throws(() =>
    margin.computeBilateralXva(
      exposureJson,
      flatHazard(0.02),
      flatHazard(0.02),
      flatDiscount(),
      1.5,
      0.4
    )
  );
});

test('computeBilateralXva throws on a malformed exposure payload', () => {
  assert.throws(() =>
    margin.computeBilateralXva(
      '{"times":[1.0],"mtm_values":[1.0],"epe":[1.0,2.0],"ene":[0.0]}',
      flatHazard(0.02),
      flatHazard(0.02),
      flatDiscount(),
      0.4,
      0.4
    )
  );
});

test('computeBilateralXva rejects unknown funding fields', () => {
  assert.throws(() =>
    margin.computeBilateralXva(
      exposureJson,
      flatHazard(0.02),
      flatHazard(0.02),
      flatDiscount(),
      0.4,
      0.4,
      JSON.stringify({ funding_spread_bp: 50.0, funding_spred_bp: 40.0 })
    )
  );
});

test('VM validation and desk direction retain settlement metadata', () => {
  const csa = margin.csaUsdRegulatoryJson();
  const collect = margin.calculateVm(csa, 1e6, 0, 'USD', '2025-01-10');
  assert.equal(collect.collect_amount, 1e6);
  assert.equal(collect.post_amount, 0);
  assert.equal(collect.currency, 'USD');
  assert.equal(collect.date, '2025-01-10');
  assert.equal(collect.settlement_date, '2025-01-13');
  const post = margin.calculateVm(csa, -1e6, 0, 'USD', '2025-01-10');
  assert.equal(post.post_amount, 1e6);
  const invalid = JSON.parse(csa);
  invalid.calendar_id = 'unknown-calendar';
  assert.throws(() => margin.validateCsaJson(JSON.stringify(invalid)));
});

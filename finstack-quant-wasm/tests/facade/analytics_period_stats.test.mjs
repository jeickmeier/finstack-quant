/**
 * `analytics.Performance.periodStats` non-finite ratio facade tests.
 *
 * `PeriodStats` documents three ratios that reach `+∞` when a sample has wins
 * and no losses (`payoff_ratio`, `profit_factor`, `cpc_index`).  Those fields
 * carry `#[serde(with = "core::wire::non_finite_f64")]` so the *JSON* wire form
 * can round-trip them — `serde_json` writes a bare `f64::INFINITY` as `null`
 * and then refuses to read `null` back as an `f64`.
 *
 * JavaScript numbers have no such limitation: `Infinity` is an ordinary number.
 * These tests pin that the JS surface stays numeric, because a string `"inf"`
 * leaking out of the serializer breaks consumers silently — `"inf" > 2` is
 * `false`, where `Infinity > 2` is `true`.
 *
 * The all-positive fixture mirrors
 * `finstack-quant-py/tests/parity/test_return_shapes.py::test_json_round_trip_survives_non_finite_values`
 * so the two surfaces stay directly comparable.
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

// Fixtures

/** Five consecutive days inside one month — aggregates to a single period. */
const DATES = ['2024-01-01', '2024-01-02', '2024-01-03', '2024-01-04', '2024-01-05'];

/** Five month-ends — aggregates to five separate monthly periods. */
const MONTH_END_DATES = ['2024-01-31', '2024-02-29', '2024-03-31', '2024-04-30', '2024-05-31'];

/** The three ratios documented to reach `+∞`. */
const INFINITE_RATIOS = ['profit_factor'];

/**
 * Aggregate one return series into monthly `PeriodStats`.
 *
 * @param {string[]} dates  Observation dates, ascending.
 * @param {number[]} returns  Per-date returns for a single asset.
 * @returns {object}  PeriodStats object (no JSON.parse — the binding returns a
 *   plain JS object, matching Python's `PeriodStats` getters).
 */
function periodStats(dates, returns) {
  const perf = new analytics.Performance({
    dates: ['2023-12-31', ...dates],
    series: [returns],
    kind: 'returns',
    ticker_names: ['FUND'],
    frequency: 'daily',
  });
  return perf.periodStats(0, 'monthly');
}

/**
 * All-positive within a single month: one winning period and no losing period,
 * so every ratio denominator is zero and the ratios are `+∞`.
 */
function allWinningStats() {
  return periodStats(DATES, [0.01, 0.02, 0.03, 0.01, 0.02]);
}

/**
 * Alternating month-end returns: monthly aggregation yields five separate
 * periods with both winners and losers, so no denominator is zero.
 */
function mixedStats() {
  return periodStats(MONTH_END_DATES, [0.03, -0.02, 0.04, -0.01, 0.02]);
}

test('periodStats returns a plain object, not an ES2015 Map', () => {
  const stats = allWinningStats();
  assert.equal(typeof stats, 'object');
  assert.ok(!(stats instanceof Map), 'result must not be an ES2015 Map');
});

test('all-winning sample actually exercises the +Infinity path', () => {
  const stats = allWinningStats();
  assert.equal(stats.win_rate, 1.0, 'fixture no longer has a 100% win rate');
  assert.ok(Number.isNaN(stats.avg_loss));
  assert.ok(Number.isNaN(stats.payoff_ratio));
  assert.ok(Number.isNaN(stats.cpc_index));
});

test('non-finite ratios reach JS as the number Infinity, not a sentinel string', () => {
  const stats = allWinningStats();
  for (const field of INFINITE_RATIOS) {
    assert.equal(
      typeof stats[field],
      'number',
      `${field} must be a JS number, got ${typeof stats[field]}`
    );
    assert.equal(
      stats[field],
      Infinity,
      `${field} must be Infinity, got ${JSON.stringify(stats[field])}`
    );
  }
});

test('Infinity ratios compare numerically', () => {
  // The regression this guards: a string `"inf"` makes every numeric
  // comparison silently false rather than throwing.
  const stats = allWinningStats();
  for (const field of INFINITE_RATIOS) {
    assert.ok(stats[field] > 2, `${field} > 2 must hold for an infinite ratio`);
    assert.ok(Number.isFinite(stats[field]) === false, `${field} must be non-finite`);
  }
});

test('finite fields are unaffected and stay numeric', () => {
  const stats = allWinningStats();
  for (const field of ['best', 'worst', 'win_rate', 'avg_return', 'avg_win']) {
    assert.equal(typeof stats[field], 'number', `${field} must be a number`);
    assert.ok(Number.isFinite(stats[field]), `${field} must be finite`);
  }
  // One-sided observed samples have no loss-normalized Kelly estimate.
  assert.equal(typeof stats.kelly_risk_fraction, 'number');
  assert.ok(Number.isNaN(stats.kelly_risk_fraction));
});

test('a mixed sample keeps every ratio finite and numeric', () => {
  // Losing periods present, so no denominator is zero and the patched fields
  // must still carry their ordinary computed values.
  const stats = mixedStats();
  assert.ok(stats.win_rate > 0 && stats.win_rate < 1, 'fixture must have both winners and losers');
  for (const field of INFINITE_RATIOS) {
    assert.equal(typeof stats[field], 'number', `${field} must be a number`);
    assert.ok(Number.isFinite(stats[field]), `${field} must be finite for a mixed sample`);
    assert.ok(stats[field] > 0, `${field} must be positive for a net-winning mixed sample`);
  }
  // payoff_ratio = avg_win / |avg_loss|; both legs are real here.
  assert.ok(
    stats.payoff_ratio > 0 && stats.payoff_ratio < 100,
    'payoff_ratio is a plausible ratio'
  );
});

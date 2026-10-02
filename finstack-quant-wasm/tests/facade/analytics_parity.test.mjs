/**
 * Facade tests for the `Performance` array constructors and JSON round trip
 * that mirror Python `from_arrays`, `from_returns_arrays`, `to_json` and
 * `from_json` (parity slice P3).
 *
 * Expected values come from
 * `finstack-quant-py/tests/data/analytics_wasm_parity.json`, which holds the
 * Python outputs for the same inputs and is asserted by
 * `finstack-quant-py/tests/test_analytics_wasm_parity.py`, so each case is a
 * cross-host golden: both hosts call the same Rust entry point.
 *
 * Requires the wasm-pack web build: npm run build (mise run wasm-build).
 */

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { analytics } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

const GOLDEN = JSON.parse(
  readFileSync(
    new URL('../../../finstack-quant-py/tests/data/analytics_wasm_parity.json', import.meta.url),
    'utf8'
  )
);
const { Performance } = analytics;
const NAMES = GOLDEN.ticker_names;
const PRICES = GOLDEN.prices;
const close = (actual, expected, rel, label) =>
  assert.ok(
    Math.abs(actual - expected) <= rel * Math.abs(expected),
    `${label}: ${actual} vs ${expected}`
  );
const pricePanel = () =>
  Performance.fromArrays(
    PRICES.dates,
    PRICES.values,
    NAMES,
    PRICES.benchmark_ticker,
    PRICES.frequency
  );

test('Performance.fromArrays is the price constructor under its Python name', () => {
  const panel = pricePanel();
  const viaConstructor = new Performance(
    PRICES.dates,
    PRICES.values,
    NAMES,
    PRICES.benchmark_ticker,
    PRICES.frequency
  );
  const sharpe = Array.from(panel.sharpe());
  assert.deepEqual(sharpe, Array.from(viaConstructor.sharpe()));
  sharpe.forEach((value, index) => close(value, PRICES.sharpe[index], 1e-12, `sharpe[${index}]`));
  assert.throws(
    () => Performance.fromArrays(PRICES.dates, PRICES.values, NAMES, 'missing'),
    (error) => error.name === 'FinstackError'
  );
});

test('Performance.fromReturnsArrays is fromReturns under its Python name', () => {
  const returns = GOLDEN.returns;
  const panel = Performance.fromReturnsArrays(returns.dates, returns.values, NAMES);
  const viaFromReturns = Performance.fromReturns(returns.dates, returns.values, NAMES);
  const volatility = Array.from(panel.volatility());
  assert.deepEqual(volatility, Array.from(viaFromReturns.volatility()));
  volatility.forEach((value, index) =>
    close(value, returns.volatility[index], 1e-12, `volatility[${index}]`)
  );
});

test('Performance.toJson writes the Python to_json document', () => {
  const json = pricePanel().toJson();
  assert.equal(typeof json, 'string');
  assert.deepEqual(JSON.parse(json), PRICES.json);
});

test('Performance.fromJson rebuilds an equivalent panel', () => {
  const panel = pricePanel();
  const rebuilt = Performance.fromJson(panel.toJson());
  assert.equal(rebuilt.toJson(), panel.toJson());
  assert.deepEqual(Array.from(rebuilt.sharpe()), Array.from(panel.sharpe()));
  const fromObject = Performance.fromJson(PRICES.json);
  assert.deepEqual(Array.from(fromObject.sharpe()), Array.from(panel.sharpe()));
  assert.throws(
    () => Performance.fromJson('{ not json'),
    (error) => error.name === 'FinstackError' && error.kind === 'validation'
  );
});

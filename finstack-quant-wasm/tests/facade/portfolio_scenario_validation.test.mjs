/**
 * Portfolio scenario exports validate the ScenarioSpec in Rust
 * (`ScenarioSpec::from_json`), like the scenarios namespace and Python.
 */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { portfolio } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

const PORTFOLIO = {
  id: 'P',
  as_of: '2025-01-15',
  base_currency: 'USD',
  entities: {},
  positions: [],
};
// The scenario is parsed before the market, so the market is never read here.
const MARKET = {};
const BLANK_ID = { id: '', operations: [] };

const invalidSpec = (error) => {
  assert.equal(error.kind, 'validation', error.message);
  assert.match(error.message, /Scenario ID cannot be empty/);
  return true;
};

test('applyScenarioAndRevalue rejects an invalid scenario spec', () => {
  assert.throws(() => portfolio.applyScenarioAndRevalue(PORTFOLIO, BLANK_ID, MARKET), invalidSpec);
});

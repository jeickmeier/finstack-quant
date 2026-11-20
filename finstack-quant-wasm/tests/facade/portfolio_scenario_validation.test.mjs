/**
 * Portfolio scenario exports validate the ScenarioSpec in Rust
 * (`ScenarioSpec::from_json`), like the scenarios namespace and Python.
 */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { core as portfolioCore, portfolio } from '../../index.js';

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
const MARKET = new portfolioCore.MarketContext();
const BLANK_ID = { id: '', operations: [] };

const invalidSpec = (error) => {
  assert.equal(error.kind, 'validation', error.message);
  assert.match(error.message, /Scenario ID cannot be empty/);
  return true;
};

test('applyScenarioAndRevalue rejects an invalid scenario spec', () => {
  assert.throws(
    () =>
      portfolio.applyScenarioAndRevalue(portfolio.Portfolio.fromSpec(PORTFOLIO), BLANK_ID, MARKET),
    invalidSpec
  );
});

test('scenario attribution uses the ordered native valuation and validates inputs', () => {
  const book = portfolio.Portfolio.fromSpec(PORTFOLIO);
  const result = portfolio.attributeScenarioPnl(
    book,
    { id: 'base', operations: [] },
    MARKET,
    'parallel'
  );
  assert.equal(result.revaluation.report.operations_applied, 0);
  assert.equal(result.mark_pnl.total.amount, '0');
  assert.equal(result.total_pnl.currency, 'USD');
  assert.throws(() => portfolio.attributeScenarioPnl(book, BLANK_ID, MARKET), invalidSpec);
});

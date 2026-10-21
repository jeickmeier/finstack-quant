/**
 * Facade tests for statement-result rollups and scenario forecast overrides
 * evaluated with and without a market.
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
const { statements, statements_analytics: sa } = facade;

await init({ module_or_path: readFileSync(WASM_BG) });

const QUARTERS = [
  ['2025Q1', '2025-01-01', '2025-04-01'],
  ['2025Q2', '2025-04-01', '2025-07-01'],
  ['2025Q3', '2025-07-01', '2025-10-01'],
  ['2025Q4', '2025-10-01', '2026-01-01'],
];

const EMPTY_MARKET = {
  schema_version: 1,
  curves: [],
  fx: null,
  surfaces: [],
  prices: {},
  series: [],
  inflation_indices: [],
  dividends: [],
  credit_indices: [],
  fx_delta_vol_surfaces: [],
  vol_cubes: [],
  hierarchy: null,
};

/** Quarterly 2025 model whose first `actualQuarters` periods are actuals. */
function quarterlyModel(actualQuarters) {
  const values = (series) =>
    Object.fromEntries(QUARTERS.slice(0, series.length).map(([id], i) => [id, series[i]]));
  return {
    id: 'rollup-model',
    periods: QUARTERS.map(([id, start, end], i) => ({
      id,
      start,
      end,
      is_actual: i < actualQuarters,
    })),
    nodes: {
      revenue: {
        node_id: 'revenue',
        node_type: 'mixed',
        values: values([100.0, 110.0, 120.0, 130.0].slice(0, actualQuarters)),
        forecast: { method: 'growth_pct', params: { rate: 0.05 } },
      },
      cash: {
        node_id: 'cash',
        node_type: 'value',
        values: values([10.0, 20.0, 30.0, 40.0]),
      },
      ebitda: { node_id: 'ebitda', node_type: 'calculated', formula_text: 'revenue * 0.3' },
      margin: { node_id: 'margin', node_type: 'calculated', formula_text: 'ebitda / revenue' },
    },
    schema_version: 1,
  };
}

test('statements.statementResultRollup rolls quarters into a year', () => {
  const model = quarterlyModel(4);
  const quarterly = new statements.Evaluator().evaluate(model);
  const annual = statements.statementResultRollup(quarterly, model, {
    frequency: 'annual',
    methods: { cash: 'period_end', margin: 'formula' },
  });

  assert.equal(annual.nodes.revenue['2025'], 460);
  assert.equal(annual.nodes.cash['2025'], 40);
  assert.ok(Math.abs(annual.nodes.ebitda['2025'] - 138) < 1e-9);
  assert.ok(Math.abs(annual.nodes.margin['2025'] - 0.3) < 1e-12);
  assert.throws(
    () => statements.statementResultRollup(quarterly, model, { frequency: 'monthly' }),
    (e) => e.kind === 'validation' && /Cannot roll/.test(e.message)
  );
});

test('statements_analytics scenario forecast overrides replace the forecast spec', () => {
  const model = quarterlyModel(1);
  const scenarioSet = {
    scenarios: {
      base: {},
      curve: {
        forecast_overrides: {
          revenue: { method: 'curve_pct', params: { curve: [0.1, 0.0, -0.1] } },
        },
      },
    },
  };
  const plain = sa.evaluateScenarioSet(model, scenarioSet);
  const priced = sa.evaluateScenarioSetWithMarket(model, scenarioSet, EMPTY_MARKET, '2025-07-01');

  for (const results of [plain, priced]) {
    const curve = QUARTERS.map(([id]) => results.curve.nodes.revenue[id]);
    curve.forEach((value, i) => assert.ok(Math.abs(value - [100, 110, 110, 99][i]) < 1e-9));
    assert.ok(Math.abs(results.base.nodes.revenue['2025Q4'] - 100 * 1.05 ** 3) < 1e-9);
  }
  assert.throws(
    () => sa.evaluateScenarioSetWithMarket(model, scenarioSet, EMPTY_MARKET, 'not-a-date'),
    (e) => e instanceof Error
  );
});

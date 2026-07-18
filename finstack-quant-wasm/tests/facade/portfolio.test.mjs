/**
 * Portfolio-namespace facade runtime contract test.
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
const { portfolio } = facade;

await init({ module_or_path: readFileSync(WASM_BG) });

const EXPORTED_KEYS = [
  'Portfolio',
  'aggregateFullCashflows',
  'aggregateFullCashflowsBuilt',
  'aggregateMetrics',
  'almgrenChrissImpact',
  'amihudIlliquidity',
  'applyScenarioAndRevalue',
  'applyScenarioAndRevalueBuilt',
  'brinsonFachler',
  'buildPortfolioFromSpec',
  'carinoLink',
  'computeFactorSensitivities',
  'computeFactorSensitivitiesWithMarket',
  'computePnlProfiles',
  'computePnlProfilesWithMarket',
  'daysToLiquidate',
  'decomposeFactorRisk',
  'evaluateRiskBudget',
  'historicalVarDecomposition',
  'kyleLambda',
  'liquidityTier',
  'lvarBangia',
  'mwrXirr',
  'optimizePortfolio',
  'parametricEsDecomposition',
  'parametricVarDecomposition',
  'parsePortfolioSpec',
  'portfolioResultGetMetric',
  'portfolioResultTotalValue',
  'replayPortfolio',
  'rollEffectiveSpread',
  'scenarioPnl',
  'scenarioPnlBuilt',
  'twrrLinked',
  'twrrModifiedDietz',
  'valuePortfolio',
  'valuePortfolioBuilt',
];

test('portfolio namespace exposes exactly the pinned contract surface', () => {
  assert.deepEqual(Object.keys(portfolio).sort(), EXPORTED_KEYS);
  for (const key of EXPORTED_KEYS) {
    assert.equal(
      typeof portfolio[key],
      'function',
      `portfolio.${key} must be a function (got ${typeof portfolio[key]})`
    );
  }
});

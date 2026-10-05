/** Models-liquidity namespace facade contract. */

import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const wasmBg = join(__dirname, '..', '..', 'pkg', 'finstack_quant_wasm_bg.wasm');
if (!existsSync(wasmBg)) {
  throw new Error(`WASM build not found at ${wasmBg}. Generate it with: npm run build`);
}

const facade = await import('../../index.js');
await facade.default({ module_or_path: readFileSync(wasmBg) });

const { liquidity } = facade.models;
const expected = [
  'almgrenChrissImpact',
  'amihudIlliquidity',
  'daysToLiquidate',
  'kyleLambda',
  'liquidityProfileHalfSpread',
  'liquidityProfileRelativeSpread',
  'liquidityProfileRelativeSpreadVolatility',
  'liquidityProfileSpread',
  'liquidityTier',
  'lvarBangia',
  'rollEffectiveSpread',
  'tradeParamsEffectiveReferencePrice',
];

function structured(value, label) {
  assert.equal(typeof value, 'object', `${label} must be structured`);
  assert.notEqual(value, null);
  assert.ok(JSON.stringify(value).length > 2);
  return value;
}

const MODEL_CLASSES = ['AlmgrenChrissModel', 'KyleLambdaModel'];

test('models.liquidity exposes exactly the moved API', () => {
  assert.deepEqual(Object.keys(liquidity).sort(), [...MODEL_CLASSES, ...expected]);
  assert.equal(liquidity.kyleLambda.length, 3);
  for (const key of expected) assert.equal(typeof liquidity[key], 'function');
});

test('portfolio retains no liquidity compatibility exports', () => {
  for (const key of expected) assert.equal(key in facade.portfolio, false);
});

test('models.liquidity preserves estimator and risk results', () => {
  assert.equal(liquidity.rollEffectiveSpread([0.01, -0.01, 0.01, -0.01]), 0.02);
  assert.equal(liquidity.rollEffectiveSpread(new Float64Array([0.01, -0.01, 0.01, -0.01])), 0.02);
  assert.equal(liquidity.daysToLiquidate(1_000_000, 250_000, 0.2), 20);
  assert.equal(liquidity.liquidityTier(3), 'tier2');
  assert.equal(liquidity.kyleLambda([0.01, -0.02], [100, 200], 50), 0.005);
  assert.ok(liquidity.amihudIlliquidity([0.01, -0.02], [100, 200]) > 0);

  const lvar = structured(
    liquidity.lvarBangia(-100_000, 0.002, 0.0005, 0.99, 1_000_000),
    'lvarBangia result'
  );
  assert.deepEqual(Object.keys(lvar).sort(), ['lvar', 'lvar_ratio', 'spread_cost', 'var']);
  assert.ok(lvar.lvar <= lvar.var);

  const impact = structured(
    liquidity.almgrenChrissImpact(10_000, 1_000_000, 0.02, 1, 0, 0.01, 100),
    'almgrenChrissImpact result'
  );
  assert.deepEqual(Object.keys(impact).sort(), [
    'cost_bp',
    'execution_risk',
    'permanent_impact',
    'temporary_impact',
    'total_cost',
  ]);
});

test('numeric series are arrays, not JSON strings', () => {
  for (const call of [
    () => liquidity.rollEffectiveSpread('[0.01,-0.01,0.01,-0.01]'),
    () => liquidity.amihudIlliquidity('[0.01]', [100]),
    () => liquidity.kyleLambda([0.01], '[100]', 50),
  ]) {
    assert.throws(call, (e) => e instanceof TypeError && e.kind === 'invalid_type');
  }
});

// Same cases as finstack-quant-py/tests/test_models_liquidity.py: the Rust
// `liquidity_tier` owns the thresholds rule for both hosts.
test('liquidityTier takes custom thresholds validated in Rust', () => {
  assert.equal(liquidity.liquidityTier(3, null), 'tier2');
  assert.equal(liquidity.liquidityTier(3, [0.5, 2, 10, 30]), 'tier3');
  assert.equal(liquidity.liquidityTier(12, [1, 2, 3, 4]), 'tier5');
  for (const bad of [
    [4, 3, 2, 1],
    [-1, 0, 1, 2],
    [0, 1, 2, 3],
    [1, NaN, 2, 3],
  ]) {
    assert.throws(
      () => liquidity.liquidityTier(3, bad),
      (e) => e instanceof Error && e.kind === 'validation'
    );
  }
  assert.throws(
    () => liquidity.liquidityTier(3, [1, 2, 3]),
    (e) => e instanceof TypeError && e.kind === 'invalid_type'
  );
});

// Twins of the Python properties computed by the Rust `LiquidityProfile` and
// `TradeParams` methods; the types are plain objects in WASM.
test('liquidity profile and trade-params derived values are computed in Rust', () => {
  const profile = {
    instrument_id: 'XYZ',
    mid: 100,
    bid: 99.5,
    ask: 100.5,
    avg_daily_volume: 1_000_000,
    avg_trade_size: 200,
    spread_volatility: 0.05,
    spread_volatility_kind: 'absolute',
    observation_days: 20,
  };
  assert.equal(liquidity.liquidityProfileSpread(profile), 1);
  assert.equal(liquidity.liquidityProfileRelativeSpread(profile), 0.01);
  assert.equal(liquidity.liquidityProfileHalfSpread(profile), 0.5);
  assert.equal(liquidity.liquidityProfileRelativeSpreadVolatility(profile), 0.05 / 100);
  assert.equal(
    liquidity.liquidityProfileRelativeSpreadVolatility({
      ...profile,
      spread_volatility_kind: 'relative',
    }),
    0.05
  );
  assert.equal(liquidity.liquidityProfileSpread(JSON.stringify(profile)), 1);

  const params = { quantity: 10_000, horizon_days: 2, daily_volatility: 0.02, profile };
  assert.equal(liquidity.tradeParamsEffectiveReferencePrice(params), 100);
  assert.equal(
    liquidity.tradeParamsEffectiveReferencePrice({ ...params, reference_price: 101 }),
    101
  );

  const validation = (e) => e instanceof Error && e.kind === 'validation';
  assert.throws(() => liquidity.liquidityProfileSpread({ ...profile, bid: 101 }), validation);
  assert.throws(() => liquidity.liquidityProfileHalfSpread({ ...profile, extra: 1 }), validation);
  assert.throws(
    () => liquidity.tradeParamsEffectiveReferencePrice({ ...params, unknown: 1 }),
    validation
  );
  assert.throws(
    () => liquidity.liquidityProfileRelativeSpread(42),
    (e) => e instanceof TypeError && e.kind === 'invalid_type'
  );
});

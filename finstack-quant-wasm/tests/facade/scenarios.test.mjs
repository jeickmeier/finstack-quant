import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { core, scenarios, valuations } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

test('scenario facade returns reusable shocked instrument copies', () => {
  const bond = valuations.instruments.Bond.fixed(
    'BOND',
    new core.Money(100, new core.Currency('USD')),
    new core.Rate(0.05),
    '2024-01-01',
    '2034-01-01',
    'none',
    'USD-OIS'
  );
  const original = bond.toJson();
  const inventory = JSON.stringify([JSON.parse(original)]);
  const market = JSON.stringify({
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
    collateral: {},
    hierarchy: null,
  });
  const op = { kind: 'instrument_price_pct_by_type', instrument_types: ['bond'], pct: -60 };
  const spec = JSON.stringify(scenarios.buildScenarioSpec('losses', [op, op]));
  assert.throws(() => scenarios.applyScenarioToMarket(spec, market, '2025-01-15'), /instruments/);
  const result = scenarios.applyScenarioToMarket(spec, market, '2025-01-15', inventory);
  assert.equal(bond.toJson(), original);
  assert.equal(result.instruments.length, 1);
  const shock =
    result.instruments[0].instrument.spec.scenario_pricing_overrides.scenario_price_shock_pct;
  assert.ok(Math.abs(100 * (1 + shock) - 16) < 1e-12);
  const restored = valuations.instruments.Bond.fromJson(JSON.stringify(result.instruments[0]));
  assert.equal(restored.id, 'BOND');
  const half = JSON.stringify(scenarios.buildScenarioSpec('half', [{ ...op, pct: -50 }]));
  const next = scenarios.applyScenarioToMarket(
    half,
    market,
    '2025-01-15',
    JSON.stringify(result.instruments)
  );
  const nextShock =
    next.instruments[0].instrument.spec.scenario_pricing_overrides.scenario_price_shock_pct;
  assert.ok(Math.abs(100 * (1 + nextShock) - 8) < 1e-12);
});

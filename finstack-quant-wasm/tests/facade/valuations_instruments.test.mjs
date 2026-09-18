/**
 * Typed Bond / TermLoan / RevolvingCredit facade smoke tests for `valuations.instruments`.
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
const { core, valuations } = facade;

await init({ module_or_path: readFileSync(WASM_BG) });

test('instruments namespace exposes typed Bond, TermLoan and RevolvingCredit classes', () => {
  assert.equal(typeof valuations.instruments.Bond, 'function');
  assert.equal(typeof valuations.instruments.TermLoan, 'function');
  assert.equal(typeof valuations.instruments.RevolvingCredit, 'function');
});

test('AssetBackedFacility.example round-trips and reports its borrowing base', () => {
  assert.equal(typeof valuations.instruments.AssetBackedFacility, 'function');
  const facility = valuations.instruments.AssetBackedFacility.example();
  assert.equal(facility.id, 'ABF-EXAMPLE');
  const envelope = JSON.parse(facility.toJson());
  assert.equal(envelope.instrument.type, 'asset_backed_facility');
  const again = valuations.instruments.AssetBackedFacility.fromJson(facility.toJson());
  assert.equal(again.toJson(), facility.toJson());
  const report = facility.borrowingBase();
  assert.equal(report.borrowing_base.currency, 'USD');
  assert.ok(Number(report.borrowing_base.amount) >= 70_000_000);
  assert.throws(() => valuations.instruments.AssetBackedFacility.fromJson('{}'));
});

test('RevolvingCredit.example round-trips through the canonical envelope', () => {
  const facility = valuations.instruments.RevolvingCredit.example();
  assert.equal(facility.id, 'RCF-USD-3Y');
  const json = facility.toJson();
  assert.equal(JSON.parse(json).instrument.type, 'revolving_credit');
  assert.equal(valuations.instruments.RevolvingCredit.fromJson(json).toJson(), json);
  assert.throws(() => valuations.instruments.RevolvingCredit.fromJson('{not json'));
});

test('Bond.fixed constructs, round-trips through the canonical envelope', () => {
  const usd = new core.Currency('USD');
  const bond = valuations.instruments.Bond.fixed(
    'BOND-1',
    new core.Money(1_000_000, usd),
    new core.Rate(0.05),
    '2024-01-01',
    '2034-01-01',
    'none',
    'USD-OIS'
  );
  assert.equal(bond.id, 'BOND-1');
  const json = bond.toJson();
  const payload = JSON.parse(json);
  assert.equal(payload.schema, 'finstack_quant.instrument/1');
  assert.equal(payload.instrument.type, 'bond');
  assert.equal(payload.instrument.spec.id, 'BOND-1');
  assert.equal(payload.instrument.spec.cashflow_spec.fixed.stub, 'none');
  const roundTripped = valuations.instruments.Bond.fromJson(json);
  assert.equal(roundTripped.toJson(), json);
  assert.throws(() => valuations.instruments.Bond.fromJson(JSON.stringify(payload.instrument)));
  assert.throws(() =>
    valuations.instruments.Bond.fixed(
      'BAD-STUB',
      new core.Money(1_000_000, usd),
      new core.Rate(0.05),
      '2024-01-01',
      '2034-01-01',
      'not_a_stub',
      'USD-OIS'
    )
  );
});

test('Bond.fromJson rejects malformed JSON and wrong instrument types', () => {
  assert.throws(() => valuations.instruments.Bond.fromJson('{not valid json'));
  const loanJson = valuations.instruments.TermLoan.example().toJson();
  assert.throws(() => valuations.instruments.Bond.fromJson(loanJson));
});

test('TermLoan.example round-trips through the canonical envelope', () => {
  const loan = valuations.instruments.TermLoan.example();
  assert.equal(loan.id, 'TERM-LOAN-USD-5Y');
  const json = loan.toJson();
  const payload = JSON.parse(json);
  assert.equal(payload.schema, 'finstack_quant.instrument/1');
  assert.equal(payload.instrument.type, 'term_loan');
  assert.equal(valuations.instruments.TermLoan.fromJson(json).toJson(), json);
});

test('listedProductCatalog exposes exact venue-filtered valuation routes', () => {
  const montreal = valuations.market.listedProductCatalog('montreal');
  assert.ok(montreal.length > 0);
  assert.ok(montreal.every((row) => row.exchange === 'montreal'));
  assert.ok(montreal.some((row) => row.symbols.includes('CRA')));
  assert.ok(montreal.some((row) => row.instrument_type === 'interest_rate_future'));
  assert.ok(
    valuations.market
      .listedProductCatalog('sgx')
      .some((row) => row.instrument_type === 'commodity_future')
  );
  const optionRoutes = new Set(
    valuations.market
      .listedProductCatalog()
      .filter((row) => row.product_kind === 'option_on_future')
      .map((row) => row.instrument_type)
  );
  assert.deepEqual(
    optionRoutes,
    new Set([
      'commodity_future_option',
      'equity_future_option',
      'fx_future_option',
      'interest_rate_future_option',
      'volatility_index_future_option',
    ])
  );
  assert.throws(() => valuations.market.listedProductCatalog('mx'));
});

// Flat 3% USD-OIS discounting. `MarketContextState` denies unknown fields and
// requires every key except `fx`, so the payload is spelled out in full.
const FLAT_MARKET = JSON.stringify({
  schema_version: 1,
  curves: [
    {
      type: 'discount',
      id: 'USD-OIS',
      base: '2024-01-15',
      day_count: 'act_365f',
      knot_points: [
        [0.0, 1.0],
        [1.0, 0.9704455335485082],
      ],
      interp_style: 'log_linear',
      extrapolation: 'flat_forward',
      min_forward_rate: null,
      allow_non_monotonic: false,
      min_forward_tenor: 1e-6,
      rate_calibration: null,
      calibration_ois_cutoff_days: null,
      fx_policy: null,
    },
  ],
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

test('RevolvingCredit.priceWithPaths keeps every simulated path of a stochastic facility', () => {
  const envelope = JSON.parse(valuations.instruments.RevolvingCredit.example().toJson());
  const spec = envelope.instrument.spec;
  spec.base_rate_spec = { fixed: { rate: 0.06 } };
  spec.draw_repay_spec = {
    stochastic: {
      utilization_process: { mean_reverting: { target_rate: 0.6, speed: 1.0, volatility: 0.25 } },
      num_paths: 8,
      seed: 42,
      mc_config: { credit_spread_process: { constant: 0.025 } },
    },
  };
  const facility = valuations.instruments.RevolvingCredit.fromJson(JSON.stringify(envelope));
  const result = facility.priceWithPaths(FLAT_MARKET, '2024-01-15');
  assert.equal(result.path_results.length, 8);
  assert.equal(result.mc_result.estimate.num_simulated_paths, 8);
  assert.equal(result.draw_option_cost.mean.currency, 'USD');
  assert.ok(
    result.path_results.every((path) => Number.isFinite(Number(path.draw_option_cost.amount)))
  );
  // A deterministic draw schedule has no paths to simulate.
  assert.throws(
    () =>
      valuations.instruments.RevolvingCredit.example().priceWithPaths(FLAT_MARKET, '2024-01-15'),
    /stochastic/i
  );
});

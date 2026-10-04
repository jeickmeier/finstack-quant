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
  assert.throws(
    () => valuations.instruments.Bond.fromJson(loanJson),
    (error) => {
      assert.equal(error.kind, 'validation');
      assert.match(error.message, /expected instrument type `bond`, got `term_loan`/);
      return true;
    }
  );
});

test('Bond.withConvention applies a preset and withStub overrides its stub', () => {
  const gbp = new core.Currency('GBP');
  const gilt = valuations.instruments.Bond.withConvention(
    'GILT-1',
    new core.Money(1_000_000, gbp),
    new core.Rate(0.04),
    '2024-01-15',
    '2034-03-07',
    'uk_gilt',
    'GBP-SONIA'
  );
  const presetSpec = JSON.parse(gilt.toJson()).instrument.spec;
  const longBack = gilt.withStub('long_back');
  const spec = JSON.parse(longBack.toJson()).instrument.spec;
  assert.equal(spec.cashflow_spec.fixed.stub, 'long_back');
  // Only the stub changes; the receiver keeps the preset's stub.
  assert.equal(
    JSON.parse(gilt.toJson()).instrument.spec.cashflow_spec.fixed.stub,
    presetSpec.cashflow_spec.fixed.stub
  );
  delete spec.cashflow_spec.fixed.stub;
  delete presetSpec.cashflow_spec.fixed.stub;
  assert.deepEqual(spec, presetSpec);
  assert.throws(
    () => gilt.withStub('not_a_stub'),
    (error) => error.kind === 'validation'
  );
  assert.throws(
    () =>
      valuations.instruments.Bond.withConvention(
        'BAD',
        new core.Money(1_000_000, gbp),
        new core.Rate(0.04),
        '2024-01-15',
        '2034-03-07',
        'not_a_convention',
        'GBP-SONIA'
      ),
    (error) => error.kind === 'validation'
  );
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
  assert.throws(
    () => valuations.market.listedProductCatalog('mx'),
    (error) => error.name === 'FinstackError' && error.kind === 'validation'
  );
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
  spec.rate = { fixed: { rate: 0.06 } };
  spec.draw_repay_spec = {
    stochastic: {
      utilization_process: { mean_reverting: { theta: 0.6, kappa: 1.0, sigma: 0.25 } },
      mc_config: { credit_spread_process: { constant: 0.025 } },
    },
  };
  spec.instrument_pricing_overrides = { model_config: { mc_paths: 8 } };
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

test('metricMetadata returns ordered native interpretation for canonical keys', () => {
  const keys = ['ytm', 'bucketed_dv01::A_x3a_x3aB::5y', 'custom_metric', 'pv01::USD-OIS', 'ytm'];
  assert.deepEqual(valuations.instruments.metricMetadata(keys), [
    {
      key: 'ytm',
      metric: 'ytm',
      components: [],
      unit: 'decimal',
      group: 'Pricing',
      bucketed: false,
    },
    {
      key: 'bucketed_dv01::A_x3a_x3aB::5y',
      metric: 'bucketed_dv01',
      components: ['A::B', '5y'],
      unit: 'currency',
      group: 'Sensitivity',
      bucketed: true,
    },
    {
      key: 'custom_metric',
      metric: 'custom_metric',
      components: [],
      unit: 'unknown',
      group: null,
      bucketed: false,
    },
    {
      key: 'pv01::USD-OIS',
      metric: 'pv01',
      components: ['USD-OIS'],
      unit: 'currency',
      group: 'Sensitivity',
      bucketed: false,
    },
    {
      key: 'ytm',
      metric: 'ytm',
      components: [],
      unit: 'decimal',
      group: 'Pricing',
      bucketed: false,
    },
  ]);
  assert.deepEqual(valuations.instruments.metricMetadata([]), []);
  // The Bermudan exercise statistic is a standard years metric, not a
  // mislabelled custom "exercise_probability".
  const [exercise] = valuations.instruments.metricMetadata(['expected_exercise_time']);
  assert.equal(exercise.unit, 'years');
  assert.equal(exercise.group, 'Rates');
  assert.ok(valuations.instruments.listStandardMetrics().includes('expected_exercise_time'));
  assert.ok(!valuations.instruments.listStandardMetrics().includes('exercise_probability'));
  assert.throws(
    () => valuations.instruments.metricMetadata(['pv01::USD_x2dOIS']),
    (error) => error.name === 'FinstackError' && error.kind === 'validation'
  );
});

test('validateInstrumentJson merges metric-pricing overrides before validation', () => {
  const bondUrl = new URL(
    '../../../finstack-quant/valuations/tests/instruments/json_examples/bond.json',
    import.meta.url
  );
  const document = JSON.parse(readFileSync(bondUrl, 'utf8'));
  document.instrument.spec.metric_pricing_overrides = { theta_period: 'invalid' };
  const json = JSON.stringify(document);
  assert.throws(
    () => valuations.instruments.validateInstrumentJson(json),
    (error) => error.name === 'FinstackError' && error.kind === 'validation'
  );
  const prepared = valuations.instruments.validateInstrumentJson(
    json,
    '{"theta_period":{"count":1,"unit":"weeks"}}'
  );
  assert.deepEqual(JSON.parse(prepared).instrument.spec.metric_pricing_overrides.theta_period, {
    count: 1,
    unit: 'weeks',
  });
  assert.equal(valuations.instruments.validateInstrumentJson(prepared), prepared);
  assert.throws(
    () => valuations.instruments.validateInstrumentJson(json, '{'),
    (error) =>
      error.name === 'FinstackError' &&
      error.kind === 'validation' &&
      error.message.includes('invalid metric_pricing_overrides JSON')
  );
});

// Python-binding audit PR 14: VaR, spec envelopes and the typed CdsOption.

function flatBond(id, amount) {
  return valuations.instruments.Bond.fixed(
    id,
    new core.Money(amount, new core.Currency('USD')),
    new core.Rate(0.05),
    '2024-01-15',
    '2029-01-15',
    'none',
    'USD-OIS'
  ).toJson();
}

const RATE_HISTORY = {
  base_date: '2024-01-15',
  window_days: 3,
  scenarios: [0.001, -0.0005, 0.002].map((shift, index) => ({
    date: `2024-01-1${2 - index}`,
    shifts: [{ factor: { type: 'discount_rate', curve_id: 'USD-OIS', tenor_years: 5.0 }, shift }],
  })),
};

test('calculateVarWithPricing matches the hvar metric and aggregates positions', () => {
  const long = JSON.parse(flatBond('LONG', 1_000_000));
  const result = valuations.instruments.calculateVarWithPricing(
    [long],
    FLAT_MARKET,
    RATE_HISTORY,
    '2024-01-15'
  );
  const fromText = valuations.instruments.calculateVarWithPricing(
    JSON.stringify([long]),
    FLAT_MARKET,
    RATE_HISTORY,
    '2024-01-15'
  );
  assert.deepEqual(fromText, result);
  const metric = valuations.instruments.priceInstrument(
    long,
    FLAT_MARKET,
    '2024-01-15',
    'default',
    ['hvar'],
    null,
    RATE_HISTORY
  );
  assert.equal(result.num_scenarios, 3);
  assert.equal(result.confidence_level, 0.95);
  assert.ok(result.var < 0);
  assert.ok(Math.abs(result.var - metric.measures.hvar) < 1e-9);

  const doubled = valuations.instruments.calculateVarWithPricing(
    [long, JSON.parse(flatBond('LONG-2', 1_000_000))],
    FLAT_MARKET,
    JSON.stringify(RATE_HISTORY),
    '2024-01-15',
    { confidence_level: 0.99 },
    'discounting'
  );
  const single99 = valuations.instruments.calculateVarWithPricing(
    [long],
    FLAT_MARKET,
    RATE_HISTORY,
    '2024-01-15',
    { confidence_level: 0.99 },
    'discounting'
  );
  assert.equal(doubled.confidence_level, 0.99);
  assert.ok(Math.abs(doubled.var - 2 * single99.var) < 1e-6);
  assert.throws(
    () =>
      valuations.instruments.calculateVarWithPricing(
        [long],
        FLAT_MARKET,
        RATE_HISTORY,
        '2024-01-15',
        {
          confidence_level: 1.5,
        }
      ),
    (error) => error.name === 'FinstackError' && error.kind === 'validation'
  );
});

test('VaR inventory rejects JSON strings nested inside the envelope array', () => {
  assert.throws(
    () =>
      valuations.instruments.calculateVarWithPricing(
        [flatBond('STRING', 1_000_000)],
        FLAT_MARKET,
        RATE_HISTORY,
        '2024-01-15'
      ),
    (error) => error.kind === 'validation'
  );
});

test('instrumentEnvelopeFromSpec wraps a bare spec and rejects tagged payloads', () => {
  const spec = {
    id: 'EURUSD-SPOT',
    base_currency: 'EUR',
    quote_currency: 'USD',
    settlement_date: '2025-01-17',
    quoted_spot: 1.2,
    notional: { amount: '1000000', currency: 'EUR' },
    attributes: {},
  };
  const envelope = valuations.instruments.instrumentEnvelopeFromSpec('fx_spot', spec);
  // FUP-003: the envelope is a plain object, not JSON text.
  assert.equal(typeof envelope, 'object');
  assert.equal(Object.getPrototypeOf(envelope), Object.prototype);
  assert.equal(envelope.schema, 'finstack_quant.instrument/1');
  assert.equal(envelope.instrument.type, 'fx_spot');
  assert.deepEqual(JSON.parse(JSON.stringify(envelope)), envelope);
  assert.deepEqual(JSON.parse(valuations.instruments.validateInstrumentJson(envelope)), envelope);
  assert.deepEqual(
    valuations.instruments.instrumentEnvelopeFromSpec('fx_spot', JSON.stringify(spec)),
    envelope
  );
  assert.throws(
    () => valuations.instruments.instrumentEnvelopeFromSpec('fx_spot', { type: 'fx_spot', spec }),
    (error) => error.name === 'FinstackError' && error.kind === 'validation'
  );
});

test('CdsOption is a typed class matching the Python wrapper', () => {
  const { CdsOption } = valuations.instruments;
  const option = CdsOption.example();
  assert.deepEqual(option.strike, { spread: '0.01' });
  assert.equal(option.optionType, 'call');
  assert.equal(option.expiry, '2025-06-20');
  assert.equal(option.couponBp, null);
  assert.equal(CdsOption.fromJson(option.toJson()).toJson(), option.toJson());

  const built = CdsOption.builder()
    .id('CDXO-1')
    .strike({ clean_price_pct: '107.0' })
    .optionType('put')
    .exerciseStyle('european')
    .expiry('2025-06-20')
    .underlyingMaturity('2030-06-20')
    .notional(new core.Money(25_000_000, new core.Currency('USD')))
    .settlement('physical')
    .recoveryRate(0.3)
    .discountCurveId('USD-OIS')
    .creditCurveId('CDX-HY-HAZARD')
    .volSurfaceId('CDX-HY-VOL')
    .underlyingIsIndex(true)
    .indexFactor(0.98)
    .strikeIndexFactor(1.0)
    .couponBp(500)
    .protectionStartConvention('forward')
    .build();
  assert.equal(built.couponBp, 500);
  assert.equal(built.protectionStartConvention, 'forward');
  assert.throws(
    () => CdsOption.builder().id('X').build(),
    (error) => error.kind === 'validation' && /missing required field/.test(error.message)
  );
});

test('VaR inventory applies the whole-input byte limit', () => {
  assert.throws(
    () =>
      valuations.instruments.calculateVarWithPricing(
        `[]${' '.repeat(16 * 1024 * 1024)}`,
        FLAT_MARKET,
        RATE_HISTORY,
        '2024-01-15'
      ),
    (error) => error.kind === 'validation'
  );
});

test('IRS accrual flags use typed independent fields', () => {
  const original = valuations.instruments.InterestRateSwap.example();
  assert.equal(original.adjustFixedAccrualDates, false);
  assert.equal(original.adjustFloatAccrualDates, false);
  const wire = JSON.parse(original.toJson());
  wire.instrument.spec.adjust_fixed_accrual_dates = true;
  wire.instrument.spec.adjust_float_accrual_dates = false;
  const restored = valuations.instruments.InterestRateSwap.fromJson(JSON.stringify(wire));
  assert.equal(restored.adjustFixedAccrualDates, true);
  assert.equal(restored.adjustFloatAccrualDates, false);
  const built = valuations.instruments.InterestRateSwap.builder()
    .id('IRS-FLAGS')
    .notional(original.notional)
    .side(original.side)
    .fixedLeg(original.fixedLeg)
    .floatLeg(original.floatLeg)
    .adjustFixedAccrualDates(false)
    .adjustFloatAccrualDates(true)
    .build();
  assert.equal(built.adjustFixedAccrualDates, false);
  assert.equal(built.adjustFloatAccrualDates, true);
});

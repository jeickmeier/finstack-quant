/**
 * Facade tests for the valuations surface bound by parity slice P7: the typed
 * instrument classes and their builders, the instrument data-type
 * constructors, `ConventionRegistry`, the schema accessors and the
 * `ValuationResult` functions.
 *
 * Expected values come from `valuations_parity.golden.json`, which
 * `finstack-quant-py/tests/test_valuations_wasm_parity.py` generates and
 * asserts through the Python binding, so each case is a cross-host golden.
 *
 * Requires the wasm-pack web build: npm run build (mise run wasm-build).
 */

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { core, valuations } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

const golden = JSON.parse(
  readFileSync(new URL('./valuations_parity.golden.json', import.meta.url), 'utf8')
);
const { instruments, fx, composite, market: conventions, schema } = valuations;

const CLASSES = {
  Bond: instruments.Bond,
  TermLoan: instruments.TermLoan,
  RevolvingCredit: instruments.RevolvingCredit,
  AssetBackedFacility: instruments.AssetBackedFacility,
  FxForward: fx.FxForward,
  FxOption: fx.FxOption,
  InterestRateSwap: instruments.InterestRateSwap,
  Swaption: instruments.Swaption,
  CapFloor: instruments.CapFloor,
  CreditDefaultSwap: instruments.CreditDefaultSwap,
  CdsIndex: instruments.CdsIndex,
  CdsTranche: instruments.CdsTranche,
  ConvertibleBond: instruments.ConvertibleBond,
  EquityOption: instruments.EquityOption,
};

// wasm32 and native transcendental functions can differ in the last bits
// (INVARIANTS.md §2.1); Monte Carlo and solver outputs amplify that.
const TIGHT = 1e-9;
const LOOSE = 1e-6;

function close(actual, expected, tol, label) {
  const scale = Math.max(1, Math.abs(expected));
  assert.ok(Math.abs(actual - expected) <= tol * scale, `${label}: ${actual} vs ${expected}`);
}

const DECIMAL = /^-?\d+(\.\d+)?([eE][-+]?\d+)?$/;

/** Deep equality with a relative tolerance on numbers and decimal strings. */
function same(actual, expected, label, tol = TIGHT) {
  if (typeof expected === 'number' && typeof actual === 'number') {
    close(actual, expected, tol, label);
  } else if (typeof expected === 'string' && typeof actual === 'string' && DECIMAL.test(expected)) {
    // Exact-decimal wire strings (Money amounts, rates) carry the same f64 rounding.
    close(Number(actual), Number(expected), tol, label);
  } else if (Array.isArray(expected)) {
    assert.ok(Array.isArray(actual), `${label}: expected an array`);
    assert.equal(actual.length, expected.length, `${label}: length`);
    expected.forEach((item, index) => same(actual[index], item, `${label}[${index}]`, tol));
  } else if (expected && typeof expected === 'object') {
    assert.ok(actual && typeof actual === 'object', `${label}: expected an object`);
    assert.deepEqual(Object.keys(actual).sort(), Object.keys(expected).sort(), `${label}: keys`);
    for (const key of Object.keys(expected))
      same(actual[key], expected[key], `${label}.${key}`, tol);
  } else {
    assert.equal(actual, expected, label);
  }
}

/** JSON round trip, so `undefined` properties and typed arrays compare as JSON does. */
const plain = (value) => JSON.parse(JSON.stringify(value));
const snake = (name) => name.replace(/[A-Z]/g, (c) => `_${c.toLowerCase()}`);
const getterNames = (cls) =>
  Object.entries(Object.getOwnPropertyDescriptors(cls.prototype))
    .filter(([, descriptor]) => descriptor.get)
    .map(([name]) => name);
const methodNames = (cls) =>
  Object.entries(Object.getOwnPropertyDescriptors(cls.prototype))
    .filter(
      ([name, descriptor]) => !descriptor.get && !name.startsWith('__') && name !== 'constructor'
    )
    .map(([name]) => name);

/** Compare one getter value with the matching field of the instrument spec. */
function sameAsSpec(value, expected, label) {
  if (value instanceof core.Money) {
    close(value.amount, Number(expected.amount), TIGHT, label);
    assert.equal(value.currency.code, expected.currency, label);
  } else if (value instanceof core.Tenor) {
    const designator = { days: 'D', weeks: 'W', months: 'M', years: 'Y' }[expected.unit];
    assert.equal(`${value.count}${value.unit}`, `${expected.count}${designator}`, label);
  } else if (value instanceof core.DayCount) {
    assert.equal(value.toString(), core.DayCount.parse(expected).toString(), label);
  } else if (value instanceof core.Currency) {
    assert.equal(value.code, expected, label);
  } else if (typeof value === 'number' && typeof expected === 'string') {
    // Decimal fields are numbers on the handle and exact strings on the wire.
    close(value, Number(expected), TIGHT, label);
  } else if (Array.isArray(value) && value[1] instanceof core.Money) {
    assert.equal(value[0], expected[0], label);
    sameAsSpec(value[1], expected[1], label);
  } else {
    same(plain(value), plain(expected), label);
  }
}

test('typed instrument examples are byte-identical to the Python envelopes', () => {
  for (const [name, cls] of Object.entries(CLASSES)) {
    const instrument = cls.example();
    assert.equal(instrument.toJson(), golden.examples[name], `${name}.toJson`);
    assert.equal(cls.fromJson(golden.examples[name]).toJson(), golden.examples[name]);
    same(plain(instrument.toDict()), golden.to_dict[name], `${name}.toDict`);
    same(plain(instrument.marketDependencies()), golden.market_dependencies[name], `${name}.deps`);
    assert.equal(instrument.defaultModel, golden.default_model[name], `${name}.defaultModel`);
    if (name in golden.expiry) {
      assert.equal(instrument.expiry, golden.expiry[name], `${name}.expiry`);
    }
    instrument.free();
  }
});

test('every field getter agrees with the instrument spec', () => {
  let checked = 0;
  for (const [name, cls] of Object.entries(CLASSES)) {
    const instrument = cls.example();
    const spec = instrument.toDict();
    for (const getter of getterNames(cls)) {
      const key = snake(getter);
      if (!(key in spec)) continue;
      // A `null` getter is an unset optional; the spec then carries the serde default.
      if (instrument[getter] === null) continue;
      sameAsSpec(instrument[getter], spec[key], `${name}.${getter}`);
      checked += 1;
    }
    instrument.free();
  }
  assert.ok(checked > 150, `only ${checked} getters were compared`);
});

/** Arguments of the multi-argument setters, derived from the example's getters. */
const SPECIAL_SETTERS = {
  premium: (builder, value) => builder.premium(value[0], value[1]),
  upfront: (builder, value) => builder.upfront(value[0], value[1]),
  deltaConvention: (builder, value) =>
    builder.deltaConvention(value.kind, new core.Currency(value.premium_currency), value.venue),
  exercise: (builder, value) =>
    builder.exercise(value.date, value.spot, value.settlement_date, value.exercised),
};
const OVERRIDE_KEYS = [
  'instrument_pricing_overrides',
  'metric_pricing_overrides',
  'scenario_pricing_overrides',
];

/** Build a copy of `example` through its builder, feeding each setter the matching getter. */
function rebuild(name, cls, example) {
  let builder = cls.builder();
  for (const setter of methodNames(builder.constructor)) {
    if (setter === 'build' || setter === 'free') continue;
    const value = example[setter];
    if (value === null || value === undefined) continue;
    builder = SPECIAL_SETTERS[setter]
      ? SPECIAL_SETTERS[setter](builder, value)
      : builder[setter](value);
  }
  const built = builder.build();
  const expected = plain(example.toDict());
  const actual = plain(built.toDict());
  for (const key of OVERRIDE_KEYS) {
    delete expected[key];
    delete actual[key];
  }
  same(actual, expected, `${name}Builder`);
  assert.throws(
    () => builder.build(),
    (error) => error.kind === 'validation',
    `${name}: a built builder is consumed`
  );
  built.free();
}

test('each builder rebuilds its example from the example getters', () => {
  for (const [name, cls] of Object.entries(CLASSES)) {
    const example = cls.example();
    rebuild(name, cls, example);
    example.free();
  }
  const deal = instruments.StructuredCredit.fromJson(golden.structured.deal);
  rebuild('StructuredCredit', instruments.StructuredCredit, deal);
});

test('builder setters check their argument types and report missing fields', () => {
  const builder = instruments.InterestRateSwap.builder();
  assert.throws(
    () => builder.id(42),
    (error) => error.kind === 'invalid_type'
  );
  assert.throws(
    () => builder.side('sideways'),
    (error) => error.kind === 'validation'
  );
  // A statement-style call mutates the same staged builder as a chained one.
  builder.id('IRS-STAGED');
  assert.throws(
    () => builder.build(),
    (error) => error.kind === 'validation'
  );
});

test('interest-rate swap pricing and ValuationResult functions match Python', () => {
  const rates = golden.rates;
  const swap = instruments.InterestRateSwap.fromJson(rates.swap);
  const result = swap.price(rates.market, '2024-01-15', null, ['dv01']);
  close(Number(result.value.amount), rates.price, TIGHT, 'price');
  close(swap.metric(rates.market, '2024-01-15', 'dv01'), rates.dv01, TIGHT, 'dv01');
  const canonical = rates.result_json;
  assert.equal(valuations.valuationResultPriceDecimal(canonical), rates.price_decimal);
  close(valuations.valuationResultGetMetric(canonical, 'dv01'), rates.get_metric, TIGHT, 'metric');
  assert.equal(valuations.valuationResultGetMetric(canonical, 'not_a_metric'), undefined);
  assert.deepEqual(valuations.valuationResultMetricKeys(canonical), rates.metric_keys);
  assert.equal(valuations.valuationResultMetricCount(canonical), rates.metric_count);
  assert.deepEqual(valuations.valuationResultMetricUnits(canonical), rates.metric_units);
  assert.equal(valuations.valuationResultAllCovenantsPassed(result), rates.all_covenants_passed);
  assert.deepEqual(valuations.valuationResultFailedCovenants(result), rates.failed_covenants);

  const usd = new core.Currency('USD');
  const conventional = instruments.InterestRateSwap.fromConventions(
    'IRS-CONV',
    new core.Money(10_000_000, usd),
    'pay',
    0.035,
    '2025-01-15',
    '2030-01-15',
    'USD-SOFR',
    'USD-OIS',
    'USD-SOFR'
  );
  assert.equal(conventional.toJson(), rates.from_conventions);

  const swaption = instruments.Swaption.example();
  close(
    swaption.forwardSwapRate(rates.market, '2024-01-15'),
    rates.swaption_forward_swap_rate,
    TIGHT,
    'forward swap rate'
  );
  close(swaption.getStrike(), rates.swaption_strike, TIGHT, 'strike');
  assert.equal(swaption.getUnderlyingStartDate(), rates.swaption_underlying_start);
  assert.equal(swaption.getUnderlyingMaturity(), rates.swaption_underlying_maturity);
});

test('credit instruments match Python', () => {
  const credit = golden.credit;
  const usd = new core.Currency('USD');
  const cds = instruments.CreditDefaultSwap.example();
  const result = cds.price(credit.market, '2024-06-20', 'hazard_rate');
  close(Number(result.value.amount), credit.cds_price, LOOSE, 'cds price');
  close(cds.parSpread(credit.market, '2024-06-20'), credit.cds_par_spread, LOOSE, 'par spread');

  const index = instruments.CdsIndex.fromPreset(
    instruments.cdsIndexParamsCdxNaIg(42, 1, 100),
    'CDX-IG-42',
    new core.Money(10_000_000, usd),
    'pay',
    '2024-03-20',
    '2029-06-20',
    0.4,
    'USD-OIS',
    'CDX.NA.IG.HAZARD'
  );
  assert.equal(index.toJson(), credit.index);
  close(index.parSpread(credit.market, '2024-06-20'), credit.index_par_spread, LOOSE, 'index par');
  close(index.riskyPv01(credit.market, '2024-06-20'), credit.index_risky_pv01, LOOSE, 'rpv01');
  close(index.cs01(credit.market, '2024-06-20'), credit.index_cs01, 1e-4, 'cs01');

  const params = instruments.cdsTrancheParamsMezzanineTranche(
    'CDX.NA.IG',
    42,
    { amount: '10000000', currency: 'USD' },
    '2029-12-20',
    100
  );
  const paramFields = plain(params);
  paramFields.notional.amount = Number(paramFields.notional.amount);
  same(paramFields, golden.data.cds_tranche_params, 'CdsTrancheParams', LOOSE);
  const tranche = instruments.CdsTranche.standard(
    'CDX-42-3X7',
    params,
    'USD-OIS',
    'CDX.NA.IG.HAZARD',
    'pay'
  );
  assert.equal(tranche.toJson(), credit.tranche);
  same(plain(instruments.cdsIndexParamsCdxNaHy(42, 1, 500)), golden.data.cds_index_params, 'hy');
  const equityParams = instruments.cdsTrancheParamsEquityTranche(
    'CDX.NA.IG',
    42,
    { amount: '10000000', currency: 'USD' },
    '2029-12-20',
    500
  );
  assert.deepEqual(
    [equityParams.attach_pct, equityParams.detach_pct],
    golden.data.equity_tranche_attachment
  );
  assert.equal(
    instruments.cdsIndexParamsItraxxEurope(40, 1, 100).index_name,
    golden.data.itraxx_index_name
  );
  // Without the credit index data both loss methods are lookup misses, as in Python.
  const example = instruments.CdsTranche.example();
  assert.throws(
    () => example.expectedLoss(golden.equity.market),
    (e) => e.kind === 'not_found'
  );
  assert.throws(
    () => example.jumpToDefault(golden.equity.market, '2024-06-20'),
    (e) => e.kind === 'not_found'
  );
});

test('equity option and convertible analytics match Python', () => {
  const equity = golden.equity;
  const option = instruments.EquityOption.example();
  const price = Number(option.price(equity.market, '2024-01-15').value.amount);
  close(price, equity.price, TIGHT, 'price');
  same(plain(option.greeks(equity.market, '2024-01-15')), equity.greeks, 'greeks', LOOSE);
  close(option.delta(equity.market, '2024-01-15'), equity.delta, LOOSE, 'delta');
  close(
    option.impliedVol(equity.market, '2024-01-15', equity.price),
    equity.implied_vol,
    LOOSE,
    'iv'
  );
  const european = instruments.EquityOption.european(
    'SPX-CALL',
    'SPX',
    4500,
    '2024-06-21',
    100,
    new core.Currency('USD'),
    'call'
  );
  assert.equal(european.toJson(), equity.european);

  const convertible = instruments.ConvertibleBond.example();
  const data = golden.convertible;
  close(convertible.parity(data.market), data.parity, TIGHT, 'parity');
  close(
    convertible.conversionPremium(data.market, 1_100_000),
    data.conversion_premium,
    TIGHT,
    'premium'
  );
  close(convertible.conversionRatio, data.conversion_ratio, TIGHT, 'ratio');
  same(plain(convertible.greeks(data.market, '2024-06-20')), data.greeks, 'greeks', 1e-4);
});

test('structured-credit deal analytics match Python', () => {
  const sc = golden.structured;
  const deal = instruments.StructuredCredit.fromJson(sc.deal);
  assert.equal(deal.toJson(), sc.deal);
  same(plain(deal.toDict()), sc.to_dict, 'toDict');
  same(
    plain(deal.trancheCashflows('A', sc.market, sc.as_of)),
    sc.tranche_cashflows,
    'flows',
    LOOSE
  );
  same(plain(deal.equityMetrics(sc.market, sc.as_of)), sc.equity_metrics, 'equity', LOOSE);
  same(
    plain(deal.runSimulationWithDiagnostics(sc.market, sc.as_of)),
    sc.diagnostics,
    'diag',
    LOOSE
  );
  const waterfall = deal.createWaterfall();
  same(plain(waterfall), sc.create_waterfall, 'waterfall');
  same(plain(instruments.waterfallCoverageTests(waterfall)), sc.coverage_tests, 'coverage tests');
  assert.equal(deal.withStandardFees().toJson(), sc.with_standard_fees);
  const stochastic = deal.enableStochastic();
  assert.equal(stochastic.toJson(), sc.enable_stochastic);
  const priced = instruments.StructuredCredit.fromJson(sc.stochastic_deal).priceStochastic(
    sc.market,
    sc.as_of,
    8
  );
  assert.equal(priced.num_paths, sc.stochastic_paths);
  close(Number(priced.npv.amount), sc.stochastic_npv, 1e-4, 'stochastic npv');
  assert.throws(
    () => deal.trancheCashflows('Z', sc.market, sc.as_of),
    (error) => error.kind === 'not_found'
  );
});

test('Merton Monte Carlo bond pricing matches Python', () => {
  const merton = golden.merton;
  const config = new instruments.MertonMcConfig(merton.model, 0.4)
    .pikSchedule(instruments.pikScheduleUniform(instruments.pikModePik()))
    .barrierCrossing(instruments.barrierCrossingDiscrete())
    .stepsPerYear(12);
  same(JSON.parse(config.toJson()), merton.config, 'config');
  assert.equal(instruments.MertonMcConfig.fromJson(config.toJson()).toJson(), config.toJson());
  const result = instruments.Bond.fromJson(merton.bond).priceMertonMc(config, 0.04, '2024-01-15');
  assert.equal(result.num_paths, merton.num_paths);
  close(result.clean_price_pct, merton.clean_price_pct, LOOSE, 'clean price');
  close(result.expected_loss, merton.expected_loss, LOOSE, 'expected loss');
  close(result.path_statistics.default_rate, merton.default_rate, LOOSE, 'default rate');
  assert.throws(
    () => new instruments.MertonMcConfig(merton.model, 1.5),
    (error) => error.kind === 'validation'
  );
});

test('instrument data-type constructors match Python', () => {
  const data = golden.data;
  const usd = (amount) => ({ amount: String(amount), currency: 'USD' });
  same(
    [instruments.barrierCrossingDiscrete(), instruments.barrierCrossingBrownianBridge()],
    data.barrier_crossing,
    'BarrierCrossing'
  );
  same(
    plain([
      instruments.pikModeCash(),
      instruments.pikModePik(),
      instruments.pikModeSplit(0.6, 0.4),
      instruments.pikModeToggle(),
    ]),
    data.pik_mode,
    'PikMode'
  );
  same(plain(instruments.pikScheduleUniform('pik')), data.pik_schedule_uniform, 'uniform');
  const stepped = instruments.pikScheduleStepped([
    [0.0, 'pik'],
    [2.0, 'cash'],
  ]);
  same(plain(stepped), data.pik_schedule_stepped, 'stepped');
  same(
    plain([
      instruments.pikScheduleModeAt(stepped, 1.0),
      instruments.pikScheduleModeAt(stepped, 3.0),
    ]),
    data.pik_schedule_mode_at,
    'modeAt'
  );

  const tranche = (id, seniority, balance, coupon) =>
    instruments
      .trancheBuilder()
      .id(id)
      .seniority(seniority)
      .originalBalance(new core.Money(balance, new core.Currency('USD')))
      .couponFixed(coupon)
      .maturity('2031-01-15')
      .build();
  const tranches = [
    tranche('A', 'senior', 72_000_000, 0.05),
    tranche('E', 'equity', 8_000_000, 0.0),
  ];
  same(plain(tranches), data.tranches, 'tranches');
  same(
    plain(instruments.trancheStructureFromBalances(tranches)),
    data.tranche_structure,
    'structure'
  );
  assert.throws(
    () => instruments.trancheBuilder().id('A').attachPct(10).build(),
    (error) => error.kind === 'validation'
  );

  const assets = [
    instruments.poolAssetFixedRateBond('B1', usd(1_000_000), 0.07, '2030-01-15', '30_360'),
    instruments.poolAssetFloatingRateLoan(
      'L1',
      usd(2_000_000),
      'USD-SOFR-3M',
      350,
      '2030-01-15',
      'act_360'
    ),
  ];
  same(plain(assets), data.pool_assets, 'pool assets');
  same(plain(instruments.assetPoolWithAssets(data.pool, assets)), data.pool_with_assets, 'assets');
  same(
    plain(instruments.assetPoolWithRepLines(data.pool, data.pool_with_rep_lines.rep_lines)),
    data.pool_with_rep_lines,
    'rep lines'
  );
  same(
    plain(instruments.assetPoolWithInstruments(data.pool, data.pool_with_instruments.instruments)),
    data.pool_with_instruments,
    'instruments'
  );
  same(
    plain(instruments.assetPoolWithReinvestmentPeriod(data.pool, data.reinvestment_period)),
    data.pool_with_reinvestment_period,
    'reinvestment period'
  );
  same(
    plain(instruments.scenarioTableCells(data.scenario_table)),
    data.scenario_table_cells,
    'scenario cells'
  );
  same(
    plain(instruments.assetPoolWithReserve(data.pool, usd(500_000), 0.02, usd(750_000))),
    data.pool_with_reserve,
    'reserve'
  );
  same(
    plain(
      instruments.assetPoolWithAccounts(data.pool, {
        collection_account: usd(125_000),
        original_balance: usd(3_000_000),
      })
    ),
    data.pool_with_accounts,
    'accounts'
  );
  same(
    plain([
      instruments.prepaymentPenaltyLockout('2026-01-15'),
      instruments.prepaymentPenaltyFixed(3.0),
      instruments.prepaymentPenaltyStepDown([
        ['2026-01-15', 3.0],
        ['2027-01-15', 1.0],
      ]),
      instruments.prepaymentPenaltyYieldMaintenance(0.03, null, 1.0),
    ]),
    data.prepayment_penalty,
    'PrepaymentPenalty'
  );
  const rules = instruments.coverageRulesCloStandard();
  same(plain(rules), data.coverage_rules, 'CoverageRules');
  assert.equal(instruments.coverageRulesValidate(rules), undefined);
  same(
    plain([
      instruments.amortizationEventDate('2026-01-15'),
      instruments.amortizationEventCumulativeLoss(0.05),
      instruments.amortizationEventExcessSpread(0.01),
    ]),
    data.amortization_event,
    'AmortizationEvent'
  );
  assert.throws(
    () => instruments.amortizationEventCumulativeLoss(2.0),
    (error) => error.kind === 'validation'
  );
});

test('composite weighting and rebalance constructors match Python', () => {
  const usd = { amount: '1000000', currency: 'USD' };
  same(
    plain([
      composite.weightingMethodFixedQuantity(),
      composite.weightingMethodNotionalWeighted(usd),
      composite.weightingMethodMetricWeighted('dv01', 'LEG-A', 1.0, true),
      composite.weightingMethodDv01Neutral('LEG-A', 1.0),
      composite.weightingMethodDeltaNeutral('LEG-A', 2.0),
      composite.weightingMethodDurationWeighted('LEG-A', 3.0),
      composite.weightingMethodVolatilityWeighted('LEG-A', 1.0, 60, 20, 252.0),
    ]),
    golden.composite.weighting,
    'WeightingMethod'
  );
  same(
    plain([
      composite.rebalanceRuleManual(),
      composite.rebalanceRuleDates(['2024-03-29', '2024-06-28']),
      composite.rebalanceRuleCalendar(
        '2024-01-31',
        { count: 1, unit: 'months' },
        'nyse',
        'modified_following',
        '2025-01-31'
      ),
    ]),
    golden.composite.rebalance,
    'RebalanceRule'
  );
  same(
    plain(composite.compositeLegSpecInstrumentDict(golden.composite.leg)),
    golden.composite.leg_instrument_dict,
    'leg instrument'
  );
  assert.throws(
    () => composite.rebalanceRuleDates(['2024-06-28', '2024-03-29']),
    (error) => error.kind === 'validation'
  );
  assert.throws(
    () => composite.compositeHistoryResultRowJson([], 0),
    (error) => error.kind === 'validation'
  );
});

test('convention registry lookups match Python', () => {
  const registry = new conventions.ConventionRegistry();
  const expected = golden.conventions;
  same(plain(registry.requireRateIndex('USD-SOFR-OIS')), expected.rate_index, 'rate index');
  same(plain(registry.resolveCds('USD', 'isda_na')), expected.cds, 'cds');
  assert.equal(registry.primaryCdsFamily('EUR'), expected.primary_cds_family);
  same(plain(registry.requireSwaption('USD')), expected.swaption, 'swaption');
  same(plain(registry.requireInflationSwap('USD-CPI')), expected.inflation_swap, 'inflation swap');
  same(plain(registry.requireIrFuture('CME:SR3')), expected.ir_future, 'ir future');
  same(plain(registry.requireXccy('EUR/USD-XCCY')), expected.xccy, 'xccy');
  assert.throws(
    () => registry.requireRateIndex('NOT-AN-INDEX'),
    (error) => error.kind === 'not_found'
  );
  assert.throws(
    () => registry.resolveCds('USD', 'nope'),
    (error) => error.kind === 'validation'
  );
  registry.free();
});

test('schema accessors return the Python schema documents', () => {
  const summary = (document) => ({
    id: document.$id ?? null,
    title: document.title ?? null,
    keys: Object.keys(document).sort(),
    definitions: Object.keys(document.$defs ?? {}).sort(),
  });
  const expected = golden.schema;
  assert.deepEqual(schema.instrumentTypes(), expected.instrument_types);
  assert.deepEqual(summary(schema.instrumentEnvelopeSchema()), expected.instrument_envelope);
  assert.deepEqual(summary(schema.instrumentSchema('bond')), expected.bond);
  assert.deepEqual(summary(schema.valuationResultSchema()), expected.valuation_result);
  assert.throws(
    () => schema.instrumentSchema('not_a_type'),
    (error) => error.kind === 'not_found'
  );
});

test('instrument JSON helpers validate the envelope type', () => {
  const envelope = golden.examples.Bond;
  assert.equal(instruments.validateTypedInstrumentJson('bond', envelope), envelope);
  assert.throws(
    () => instruments.validateTypedInstrumentJson('term_loan', envelope),
    (error) => error.kind === 'validation'
  );
  const pretty = instruments.prettyInstrumentJson(envelope);
  assert.ok(pretty.includes('\n'));
  assert.deepEqual(JSON.parse(pretty), JSON.parse(envelope));
});

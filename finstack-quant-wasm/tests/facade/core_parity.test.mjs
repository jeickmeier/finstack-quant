/**
 * Cross-host goldens for the `core` surface bound in both hosts (parity slice P1).
 *
 * Every case is computed by Rust. The same case names, inputs and expected
 * values are asserted by `finstack-quant-py/tests/test_core_wasm_parity.py`
 * against the shared golden `finstack-quant-py/tests/data/core_wasm_parity.json`.
 * Dates are compared as ISO strings: the `core` date utilities return epoch
 * days here and `datetime.date` in Python, and each test converts.
 */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { core } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

const GOLDEN = JSON.parse(
  readFileSync(
    new URL('../../../finstack-quant-py/tests/data/core_wasm_parity.json', import.meta.url),
    'utf8'
  )
);

const ep = (iso) => core.daysSinceEpoch(iso);
const iso = (days) => {
  if (days === undefined) return null;
  const [year, month, day] = core.dateFromEpochDays(days);
  return `${year}-${String(month).padStart(2, '0')}-${String(day).padStart(2, '0')}`;
};
const isos = (days) => Array.from(days, iso);

const discount = () =>
  core.DiscountCurve.fromZeroRates(
    'USD-OIS',
    '2025-01-02',
    [0.5, 0.04, 1.0, 0.042, 5.0, 0.045],
    'semi_annual'
  );
const inflation = () =>
  new core.InflationCurve({
    id: 'US-CPI',
    baseDate: '2025-01-02',
    baseCpi: 300.0,
    knots: [0.0, 300.0, 5.0, 331.2],
    indexationLagMonths: 3,
  });
const price = () =>
  new core.PriceCurve({
    id: 'WTI',
    baseDate: '2025-01-02',
    knots: [0.0, 72.0, 1.0, 70.5],
    spotPrice: 72.0,
  });
const baseCorrelation = () => new core.BaseCorrelationCurve('CDX-IG', [3.0, 0.25, 7.0, 0.45]);
const surface = () =>
  new core.VolSurface(
    'SPX-VOL',
    [0.5, 1.0],
    [90.0, 100.0, 110.0],
    [0.24, 0.2, 0.22, 0.23, 0.21, 0.22]
  );
const series = () =>
  new core.ScalarTimeSeries(
    'SOFR-FIXINGS',
    [
      ['2025-01-02', 0.0431],
      ['2025-01-06', 0.0433],
    ],
    null,
    'linear'
  );
const index = (lag = '3M') =>
  new core.InflationIndex(
    'US-CPI-U',
    [
      ['2024-10-01', 315.664],
      ['2024-11-01', 315.493],
      ['2024-12-01', 315.605],
    ],
    'USD',
    'linear',
    lag
  );
const fx = () => core.FxMatrix.fromDict({ 'EUR/USD': 1.1, GBPUSD: 1.27 });
const market = () => {
  const context = new core.MarketContext();
  context.insert(core.DiscountCurve.flat('USD-OIS', '2025-01-02', 0.04));
  context.insert(inflation());
  context.insert(price());
  context.insert(baseCorrelation());
  context.insert(surface());
  context.insertFx(fx());
  context.insertPrice('SPX', 5900.0);
  context.insertPrice('ACME', 12.5, 'USD');
  context.insertSeries(series());
  context.insertInflationIndex(index());
  // A credit index is persisted by reference, so its curves are stored too.
  const hazard = core.HazardCurve.flat('CDX-IG-HZD', '2025-01-02', 0.01, 0.4);
  context.insert(hazard);
  context.insertCreditIndex(
    'CDX-IG-43',
    new core.CreditIndexData(125, 0.4, hazard, baseCorrelation())
  );
  context.mapCollateral('USD-CSA', 'USD-OIS');
  return context;
};
const schedule = () =>
  core.Schedule.builder(ep('2025-01-15'), ep('2026-01-15'))
    .frequency('3M')
    .adjustWith('modified_following', 'nyse')
    .paymentLagDays(2)
    .build();
const config = () => {
  const cfg = new core.FinstackConfig('away_from_zero');
  cfg.setOutputScale('JPY', 2);
  cfg.setExtension('core.example.v1', { flag: true, levels: [1, 2] });
  return cfg;
};
const attributes = () => {
  const value = new core.Attributes();
  value.addTag('energy');
  value.setMeta('sector', 'utilities');
  return value;
};

const usFederal = core.FiscalConfig.usFederal();
const nyse = new core.HolidayCalendar('nyse');
const registry = core.embeddedRegistry();
const SERIES = [100.0, 101.5, 99.75, 102.25, 103.0];
const money = (amount, code) => new core.Money(amount, new core.Currency(code));

const CASES = {
  // --- dates: IMM, CDS and option rules ---------------------------------------
  third_wednesday: () => iso(core.thirdWednesday(3, 2025)),
  third_friday: () => iso(core.thirdFriday(3, 2025)),
  next_imm: () => iso(core.nextImm(ep('2025-03-20'))),
  is_imm_date: () => [core.isImmDate(ep('2025-03-19')), core.isImmDate(ep('2025-03-20'))],
  is_cds_date: () => [core.isCdsDate(ep('2025-03-20')), core.isCdsDate(ep('2025-03-19'))],
  next_cds_date: () => iso(core.nextCdsDate(ep('2025-03-20'))),
  prev_cds_date: () => iso(core.prevCdsDate(ep('2025-03-20'))),
  prev_cds_semiannual_roll: () => iso(core.prevCdsSemiannualRoll(ep('2024-02-14'))),
  next_semiannual_cds_maturity: () => iso(core.nextSemiannualCdsMaturity(ep('2028-09-20'))),
  imm_option_expiry: () => iso(core.immOptionExpiry(3, 2025)),
  next_imm_option_expiry: () => iso(core.nextImmOptionExpiry(ep('2025-03-14'))),
  next_equity_option_expiry: () => iso(core.nextEquityOptionExpiry(ep('2025-03-21'))),
  // --- dates: SIFMA ---------------------------------------------------------------
  sifma_settlement_date: () => iso(core.sifmaSettlementDate(3, 2027)),
  sifma_settlement_date_uncovered: () => iso(core.sifmaSettlementDate(3, 1990)),
  sifma_settlement_date_for_class: () =>
    iso(core.sifmaSettlementDateForClass(1, 2026, core.SifmaSettlementClass.b())),
  estimated_sifma_settlement_date_for_class: () =>
    iso(
      core.estimatedSifmaSettlementDateForClass(
        3,
        2031,
        core.SifmaSettlementClass.fromAgencyTerm('GNMA', 30)
      )
    ),
  next_sifma_settlement: () => iso(core.nextSifmaSettlement(ep('2027-03-16'))),
  // --- dates: DateExt, calendars, day counts --------------------------------------
  add_business_days: () => iso(core.addBusinessDays(ep('2025-06-27'), 3, 'target2')),
  add_weekdays: () => iso(core.addWeekdays(ep('2025-06-27'), -3)),
  add_months: () => iso(core.addMonths(ep('2024-01-31'), 1)),
  end_of_month: () => iso(core.endOfMonth(ep('2024-02-15'))),
  is_weekend: () => [core.isWeekend(ep('2025-01-04')), core.isWeekend(ep('2025-01-06'))],
  quarter: () => core.quarter(ep('2025-08-15')),
  fiscal_year: () => [
    core.fiscalYear(ep('2024-11-15'), usFederal),
    core.fiscalYear(ep('2024-09-30'), usFederal),
  ],
  months_until: () => core.monthsUntil(ep('2020-01-15'), ep('2022-03-10')),
  days_since_epoch: () => core.daysSinceEpoch('2025-01-02'),
  days_30_360: () =>
    ['us_sia', 'isda', 'european', 'italian'].map((name) =>
      core.days30360(ep('2025-01-31'), ep('2025-03-31'), name)
    ),
  days_30e_360_isda: () =>
    [false, true].map((flag) => core.days30e360Isda(ep('2025-01-31'), ep('2025-02-28'), flag)),
  holiday_calendar: () => ({
    code: new core.HolidayCalendar('NYSE + gblo').code,
    is_holiday: nyse.isHoliday(ep('2025-01-01')),
    is_business_day: nyse.isBusinessDay(ep('2025-01-06')),
    count_business_days: nyse.countBusinessDays(ep('2025-01-01'), ep('2025-02-01')),
    metadata_id: nyse.metadata.id,
  }),
  // --- dates: periods and schedules ---------------------------------------------
  period_id: () => [
    core.PeriodId.parse('2025M12').next().code,
    core.PeriodId.quarter(2025, 1).prev().code,
    core.PeriodId.parse('FY2025Q4').nextFiscal(usFederal).code,
    core.PeriodId.week(2025, 5).kind.periodsPerYear,
  ],
  period_kind_prior_observation_date: () =>
    iso(core.PeriodKind.quarterly().priorObservationDate(ep('2025-03-31'))),
  build_periods: () => core.buildPeriods('2025Q1..Q4', '2025Q2'),
  build_fiscal_periods: () => core.buildFiscalPeriods('FY2025Q1..Q2', usFederal),
  schedule: () => JSON.parse(schedule().toJson()),
  schedule_dates: () => isos(schedule().paymentDates),
  schedule_cds_imm: () =>
    isos(
      core.Schedule.builder(ep('2025-03-20'), ep('2026-03-20')).frequency('3M').cdsImm().build()
        .dates
    ),
  // --- math -----------------------------------------------------------------------
  symmetric_eigen: () => Array.from(core.symmetricEigen([2.0, 0.0, 0.0, 5.0], 2)[0]).sort(),
  ledoit_wolf_shrinkage: () => {
    const [covariance, shrinkage] = core.ledoitWolfShrinkage(
      [1.0, 1.0, -1.0, -1.0, 2.0, -2.0, -2.0, 2.0],
      4,
      2
    );
    return { covariance: Array.from(covariance), shrinkage };
  },
  linalg_constants: () => [
    core.singularThreshold(),
    core.diagonalTolerance(),
    core.symmetryTolerance(),
  ],
  mean_var: () => Array.from(core.meanVar(SERIES)),
  stats_or_nan: () => [
    core.meanOrNan(SERIES),
    core.sampleVarianceOrNan(SERIES),
    core.sampleStdOrNan(SERIES),
    core.medianOrNan(SERIES),
    core.quantileLinearOrNan(SERIES, 0.3),
    core.finiteMinOrNan(SERIES),
    core.finiteMaxOrNan(SERIES),
  ],
  stats_empty_is_nan: () => [
    Number.isNaN(core.meanOrNan([])),
    Number.isNaN(core.sampleVarianceOrNan([1.0])),
    Number.isNaN(core.finiteMinOrNan([NaN])),
  ],
  finite_count: () => core.finiteCount([1.0, NaN, Infinity, 2.0]),
  log_returns: () => Array.from(core.logReturns(SERIES)),
  norm_with_params: () => [
    core.normCdfWithParams(1.5, 1.0, 2.0),
    core.normPdfWithParams(1.5, 1.0, 2.0),
  ],
  student_t: () => [core.studentTCdf(1.2, 5.0), core.studentTInvCdf(0.975, 5.0)],
  // --- config, rating scales, types, money ------------------------------------------
  finstack_config: () => JSON.parse(config().toJson()),
  finstack_config_scales: () => [
    config().outputScale('JPY'),
    config().outputScale('USD'),
    config().ingestScale('USD'),
    config().roundingMode.name,
    config().extensionKeys(),
  ],
  rating_registry: () => ({
    default_scale_id: registry.defaultScaleId(),
    default_score: registry.defaultScorecardScore(),
    scale_ids: registry.scaleIds(),
    policy: registry.unknownScalePolicy().name,
    known: registry.isKnownRatingScale(registry.defaultScaleId()),
    scale: registry.ratingScale(registry.defaultScaleId()),
    extension_key: core.ratingScalesExtensionKey(),
    from_config: core.registryFromConfig(new core.FinstackConfig()).defaultScaleId(),
  }),
  credit_rating: () => [
    new core.CreditRating('Baa3').name,
    core.CreditRating.bbb().notchesTo('BB'),
    core.CreditRating.aaPlus().toMoodysString(),
    core.CreditRating.bbPlus().isInvestmentGrade(),
    core.CreditRating.nr().isSpeculativeGrade(),
    core.CreditRating.d().isDefault(),
    core.CreditRating.bbbMinus().toJson(),
  ],
  identifiers: () => [
    new core.CurveId('USD-OIS').toJson(),
    new core.InstrumentId('BOND_A').asStr(),
    new core.CurveId('').isEmpty(),
  ],
  attributes: () => ({
    json: JSON.parse(attributes().toJson()),
    matches: [
      attributes().matchesSelector('tag:energy'),
      attributes().matchesSelector('meta:sector=tech'),
    ],
    items: attributes().items(),
  }),
  rate_zero: () => [
    core.Rate.zero().asDecimal,
    core.Bps.zero().asBp,
    core.Percentage.zero().asPercent,
  ],
  money: () => [
    JSON.parse(core.Money.zero(core.Currency.usd()).toJson()),
    core.Money.fromTuple([12.5, 'EUR']).toTuple(),
  ],
  currency: () => [
    core.Currency.fromNumeric(840).code,
    core.Currency.jpy().decimals,
    core.Currency.try().numeric,
  ],
  // --- market data ------------------------------------------------------------------
  discount_curve: () => JSON.parse(discount().toJson()),
  discount_curve_queries: () => [
    discount().zeroAnnual(2.0),
    discount().zeroRate(2.0, 'semi_annual'),
    discount().zeroRateOnDate('2027-01-02', 'quarterly'),
    discount().dfOnDateCurve('2027-01-02'),
    discount().dfBetweenDates('2026-01-02', '2027-01-02'),
    discount().toForwardCurve('USD-FWD', 0.25).rate(1.0),
    discount().dayCount,
    discount().interpStyle,
    discount().extrapolation,
    Array.from(discount().knots),
    Array.from(discount().dfs),
  ],
  discount_curve_from_dates: () =>
    JSON.parse(
      core.DiscountCurve.fromDates('USD-OIS', '2025-01-02', [
        ['2026-01-02', 0.96],
        ['2030-01-02', 0.8],
      ]).toJson()
    ),
  inflation_curve: () => JSON.parse(inflation().toJson()),
  inflation_curve_queries: () => [
    inflation().cpi(2.5),
    inflation().cpiOnDate('2027-07-02'),
    inflation().cpiWithLag(2.5),
    inflation().indexRatio(2.5),
    inflation().inflationRate(0.0, 5.0),
    inflation().indexationLagMonths,
  ],
  price_curve: () => JSON.parse(price().toJson()),
  price_curve_queries: () => [price().price(0.5), price().priceOnDate('2025-07-02'), price().kind],
  base_correlation_curve: () => JSON.parse(baseCorrelation().toJson()),
  base_correlation: () => baseCorrelation().correlation(5.0),
  vol_surface: () => JSON.parse(surface().toJson()),
  vol_surface_queries: () => [
    surface().vol(0.75, 100.0),
    Array.from(surface().gridShape),
    surface().secondaryAxis,
    surface().quoteType,
    surface().interpolationMode,
  ],
  scalar_time_series: () => JSON.parse(series().toJson()),
  scalar_time_series_queries: () => [
    series().valueOn('2025-01-04'),
    series().valueOnExact('2025-01-06'),
    series().firstDate ?? null,
    series().lastDate ?? null,
  ],
  inflation_index: () => JSON.parse(index().toJson()),
  inflation_index_queries: () => [
    index('none').ratio('2024-10-01', '2024-12-01'),
    index('none').valueOn('2024-11-16'),
    index().refCpiMonthsLag('2025-02-15', 3),
    index().dateRange(),
    index().lag,
  ],
  fx_matrix: () => [
    fx().rate('EUR', 'USD', '2025-01-02').rate,
    fx().rate('USD', 'GBP', '2025-01-02').rate,
  ],
  market_context: () => JSON.parse(market().toJson()),
  market_context_queries: () => ({
    curve_ids: market().curveIds(),
    stats: market().stats(),
    contains: [market().contains('USD-OIS'), market().contains('missing')],
    is_empty: [market().isEmpty(), new core.MarketContext().isEmpty()],
    df: market().getDiscount('USD-OIS').df(1.0),
    vol: market().getSurface('SPX-VOL').vol(0.75, 100.0),
    series: market().getSeries('SOFR-FIXINGS').valueOn('2025-01-06'),
    index_lag: market().getInflationIndex('US-CPI-U').lag,
    credit_index: market().getCreditIndex('CDX-IG-43').numConstituents,
    fx_rate: market().fxRequired().rate('EUR', 'USD', '2025-01-02').rate,
    converted: market()
      .convertMoney(money(100.0, 'EUR'), new core.Currency('USD'), '2025-01-02')
      .toTuple(),
    rolled_base_date: market().rollForward(30).getDiscount('USD-OIS').baseDate,
    round_trip: core.MarketContext.fromJson(market().toJson()).toJson() === market().toJson(),
  }),
};

/** Deep comparison with a relative tolerance on numbers (native vs wasm32 libm). */
function close(actual, expected, path) {
  if (typeof expected === 'number') {
    assert.equal(typeof actual, 'number', `${path}: expected a number, got ${typeof actual}`);
    const scale = Math.max(1, Math.abs(expected));
    assert.ok(Math.abs(actual - expected) <= 1e-12 * scale, `${path}: ${actual} != ${expected}`);
  } else if (Array.isArray(expected)) {
    assert.ok(Array.isArray(actual), `${path}: expected an array`);
    assert.equal(actual.length, expected.length, `${path}: length`);
    expected.forEach((value, i) => close(actual[i], value, `${path}[${i}]`));
  } else if (expected !== null && typeof expected === 'object') {
    assert.ok(actual !== null && typeof actual === 'object', `${path}: expected an object`);
    assert.deepEqual(Object.keys(actual).sort(), Object.keys(expected).sort(), `${path}: keys`);
    for (const [key, value] of Object.entries(expected))
      close(actual[key], value, `${path}.${key}`);
  } else {
    assert.equal(actual, expected, path);
  }
}

test('the golden lists exactly these cases', () => {
  assert.deepEqual(Object.keys(GOLDEN).sort(), Object.keys(CASES).sort());
});

for (const [name, run] of Object.entries(CASES)) {
  test(`core parity: ${name}`, () => close(run(), GOLDEN[name], name));
}

const kind = (expected, pattern) => (error) => {
  assert.ok(error instanceof Error, `expected an Error, got ${typeof error}: ${error}`);
  assert.equal(error.kind, expected, `kind of: ${error.message}`);
  if (pattern) assert.match(error.message, pattern);
  return true;
};

test('F350: FX pair keys are parsed by Rust in both hosts', () => {
  assert.throws(
    () => core.FxMatrix.fromDict({ 'é€x': 1.0 }),
    kind('validation', /invalid FX pair/)
  );
  assert.throws(() => core.FxMatrix.fromDict({ 'EUR/XYZ': 1.0 }), kind('validation'));
  const matrix = new core.FxMatrix();
  matrix.setQuotes([
    ['EUR', 'USD', 1.1],
    ['GBP', 'USD', 1.27],
  ]);
  assert.equal(matrix.rate('EUR', 'USD', '2025-01-02').rate, 1.1);
  // A bad batch is atomic: nothing is applied.
  assert.throws(
    () =>
      matrix.setQuotes([
        ['USD', 'JPY', 150.0],
        ['USD', 'CHF', -1.0],
      ]),
    kind('validation')
  );
  assert.throws(() => matrix.rate('USD', 'JPY', '2025-01-02'), kind('not_found'));
});

test('unknown names carry the Rust error kind', () => {
  assert.throws(() => new core.HolidayCalendar('not-a-calendar'), kind('not_found'));
  assert.throws(() => market().getDiscount('missing'), kind('not_found'));
  assert.throws(
    () =>
      new core.PriceCurve({ id: 'X', baseDate: '2025-01-02', knots: [0, 1, 1, 2], kind: 'Price' }),
    kind('validation', /price curve kind/)
  );
  assert.throws(() => core.PeriodId.month(2025, 13), kind('validation'));
  assert.throws(() => core.normCdfWithParams(0.0, 0.0, -1.0), kind('validation'));
  assert.throws(() => core.thirdWednesday(13, 2025), kind('validation'));
  assert.throws(() => core.thirdWednesday(3, 1e9), kind('validation'));
});

test('arguments of the wrong type are TypeErrors', () => {
  assert.throws(() => core.nextImm('2025-03-20'), kind('invalid_type'));
  assert.throws(() => core.daysSinceEpoch(20089), kind('invalid_type'));
  assert.throws(() => new core.MarketContext().insert({}), kind('invalid_type'));
  assert.throws(() => new core.MarketContext().insertPrice('X', '1'), kind('invalid_type'));
  assert.throws(() => core.Money.fromTuple([1.0]), kind('invalid_type'));
  assert.throws(() => new core.FinstackConfig(3), kind('invalid_type'));
});

test('enum classes expose every Rust variant and round-trip their names', () => {
  const families = [
    [
      core.BusinessDayConvention,
      ['unadjusted', 'following', 'modifiedFollowing', 'preceding', 'modifiedPreceding', 'nearest'],
    ],
    [core.StubKind, ['none', 'shortFront', 'shortBack', 'longFront', 'longBack']],
    [core.ScheduleErrorPolicy, ['strict', 'missingCalendarWarning', 'gracefulEmpty']],
    [core.Thirty360Convention, ['usSia', 'isda', 'european', 'italian']],
    [core.PeriodKind, ['daily', 'weekly', 'monthly', 'quarterly', 'semiAnnual', 'annual']],
    [core.RoundingMode, ['bankers', 'awayFromZero', 'towardZero', 'floor', 'ceil']],
    [core.UnknownScalePolicy, ['error', 'fallbackToDefault', 'warnAndFallback']],
  ];
  for (const [family, factories] of families) {
    const names = factories.map((factory) => family[factory]().toString());
    assert.equal(new Set(names).size, names.length);
    for (const name of names) assert.equal(family.fromName(name).toString(), name);
  }
  assert.deepEqual(
    ['days', 'weeks', 'months', 'years'].map((unit) => core.TenorUnit[unit]().toString()),
    ['D', 'W', 'M', 'Y']
  );
  assert.equal(core.TenorUnit.fromChar('y').toString(), 'Y');
  assert.throws(() => core.TenorUnit.fromChar('MM'), kind('validation'));
  assert.deepEqual(
    ['a', 'b', 'c', 'd'].map((cls) => core.SifmaSettlementClass[cls]().toString()),
    ['a', 'b', 'c', 'd']
  );
  assert.equal(core.RoundingMode.fromJson(core.RoundingMode.floor().toJson()).name, 'floor');
  assert.equal(
    core.UnknownScalePolicy.fromJson(core.UnknownScalePolicy.error().toJson()).name,
    'error'
  );
});

test('every ISO-4217 currency has a Currency static', () => {
  const statics = Object.getOwnPropertyNames(core.Currency).filter(
    (name) => /^[a-z]{3}$/.test(name) && typeof core.Currency[name] === 'function'
  );
  assert.equal(statics.length, 159);
  for (const name of statics) assert.equal(core.Currency[name]().code, name.toUpperCase());
});

test('handles round-trip through their JSON wire form', () => {
  const roundTrips = [
    [core.DiscountCurve, discount()],
    [core.InflationCurve, inflation()],
    [core.PriceCurve, price()],
    [core.BaseCorrelationCurve, baseCorrelation()],
    [core.VolSurface, surface()],
    [core.ScalarTimeSeries, series()],
    [core.InflationIndex, index()],
    [core.Schedule, schedule()],
    [core.FinstackConfig, config()],
    [core.Attributes, attributes()],
    [core.RatingScaleRegistry, registry],
    [core.MarketContext, market()],
  ];
  for (const [handle, value] of roundTrips) {
    const json = value.toJson();
    assert.equal(handle.fromJson(json).toJson(), json);
    assert.equal(handle.fromJson(JSON.parse(json)).toJson(), json);
  }
  assert.equal(core.CreditRating.fromJson(core.CreditRating.bbbMinus().toJson()).name, 'BBB-');
  assert.equal(core.CurveId.fromJson(new core.CurveId('USD-OIS').toJson()).toString(), 'USD-OIS');
});

test('ScheduleBuilder setters update the builder in place and round-trip its spec', () => {
  const base = core.Schedule.builder(ep('2025-01-15'), ep('2025-07-15'));
  const quarterly = base.frequency('3M');
  assert.equal(quarterly, base);
  assert.deepEqual(base.toSpec().frequency, { count: 3, unit: 'months' });
  assert.equal(quarterly.build().dates.length, 3);
  assert.equal(core.Schedule.fromSpec(quarterly.toSpec()).toJson(), quarterly.build().toJson());
  const imm = base.cdsImm().imm().toSpec();
  assert.deepEqual([imm.imm_mode, imm.cds_imm_mode], [true, false]);
});

test('MarketContext inserts change the context in place and share curve data', () => {
  const context = new core.MarketContext();
  assert.equal(context.isEmpty(), true);
  assert.equal(context.fx, undefined);
  assert.throws(() => context.fxRequired(), kind('not_found'));
  const matrix = fx();
  context.insertFx(matrix);
  matrix.setQuote('USD', 'JPY', 150.0);
  assert.equal(context.fx.rate('USD', 'JPY', '2025-01-02').rate, 150.0);
  assert.deepEqual(market().getPrice('SPX'), { unitless: 5900.0 });
  assert.deepEqual(market().getPrice('ACME'), { price: { amount: '12.5', currency: 'USD' } });
  assert.equal(market().getPriceCurve('WTI').spotPrice, 72.0);
  assert.equal(market().getInflationCurve('US-CPI').baseCpi, 300.0);
  assert.equal(
    market().getBaseCorrelation('CDX-IG').correlation(5.0),
    baseCorrelation().correlation(5.0)
  );
  assert.equal(market().getCreditIndex('CDX-IG-43').indexCreditCurve.id, 'CDX-IG-HZD');
});

test('FinstackConfig extensions hold JSON values, not JSON text', () => {
  const cfg = new core.FinstackConfig(null, { rate_epsilon: 1e-9 });
  assert.deepEqual(cfg.tolerances, { rate_epsilon: 1e-9, generic_epsilon: 1e-10 });
  cfg.setExtension('core.example.v1', '{"not":"parsed"}');
  assert.equal(cfg.getExtension('core.example.v1'), '{"not":"parsed"}');
  assert.equal(cfg.getExtensionJson('core.example.v1'), '"{\\"not\\":\\"parsed\\"}"');
  assert.equal(cfg.getExtension('core.other.v1'), undefined);
  assert.equal(cfg.removeExtension('core.example.v1'), true);
  assert.equal(cfg.removeExtension('core.example.v1'), false);
  assert.throws(() => cfg.setExtension('bad key', 1), kind('validation'));
  assert.throws(() => new core.FinstackConfig(null, { rate_epsilon: -1 }), kind('validation'));
  cfg.setIngestScale('USD', 8);
  assert.deepEqual(cfg.ingestScaleOverrides(), { USD: 8 });
  assert.deepEqual(cfg.outputScaleOverrides(), {});
});

/**
 * Facade tests for the margin surface bound for Python parity (slice P3).
 *
 * Expected values come from `finstack-quant-py/tests/data/margin_wasm_parity.json`,
 * the Python outputs for the same inputs
 * (`finstack-quant-py/tests/test_margin_wasm_parity.py` pins Python to the
 * same file), so every case is a cross-host golden: both hosts call the same
 * Rust entry point.
 *
 * Requires the wasm-pack web build: npm run build (mise run wasm-build).
 */

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { core, margin } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

const golden = JSON.parse(
  readFileSync(
    new URL('../../../finstack-quant-py/tests/data/margin_wasm_parity.json', import.meta.url),
    'utf8'
  )
);

/** Canonical JSON of a result, without the run-specific `meta` stamp. */
const wire = (value) => {
  const plain = typeof value === 'string' ? JSON.parse(value) : structuredClone(value);
  if (plain && typeof plain === 'object' && !Array.isArray(plain)) delete plain.meta;
  return plain;
};

const numeric = (value) =>
  typeof value === 'string' && value.trim() !== '' && Number.isFinite(Number(value));

/**
 * Deep equality with a relative tolerance on numbers (native and wasm32
 * floating point may differ in the last bits) and on decimal-string Money
 * amounts; `undefined` equals the Python `None` (`null`).
 */
function close(actual, expected, path) {
  if (expected === null) {
    assert.ok(actual === null || actual === undefined, `${path}: expected null, got ${actual}`);
  } else if (Array.isArray(expected)) {
    assert.ok(Array.isArray(actual), `${path}: expected an array`);
    assert.equal(actual.length, expected.length, `${path}: length`);
    expected.forEach((item, index) => close(actual[index], item, `${path}[${index}]`));
  } else if (typeof expected === 'object') {
    assert.equal(typeof actual, 'object', `${path}: expected an object`);
    assert.deepEqual(Object.keys(actual).sort(), Object.keys(expected).sort(), `${path}: keys`);
    for (const key of Object.keys(expected)) close(actual[key], expected[key], `${path}.${key}`);
  } else if (typeof expected === 'number' || (numeric(expected) && numeric(actual))) {
    const [a, e] = [Number(actual), Number(expected)];
    assert.ok(
      Math.abs(a - e) <= 1e-9 * Math.max(1, Math.abs(e)),
      `${path}: ${actual} vs ${expected}`
    );
    if (typeof expected === 'number') assert.equal(typeof actual, 'number', `${path}: type`);
  } else {
    assert.equal(actual, expected, path);
  }
}

const amount = (money) => Number(money.amount);

const throwsKind = (fn, kind, pattern) =>
  assert.throws(fn, (error) => {
    assert.ok(error instanceof Error, 'throws an Error');
    assert.equal(error.kind, kind);
    if (pattern) assert.match(error.message, pattern);
    return true;
  });

function simmSensitivities() {
  const sens = new margin.SimmSensitivities('USD');
  sens.addIrDelta('USD', '5Y', 50_000);
  sens.addIrDelta('USD', '10Y', -20_000);
  sens.addIrVega('USD', '5Y', 4_000);
  sens.addCreditQualifyingDelta('financial', 'ACME', '5Y', 3_000);
  sens.addCreditQualifyingVega('financial', 'ACME', '5Y', 500);
  sens.addCreditNonQualifyingDelta('RMBS_A', '5Y', 1_500);
  sens.addCreditNonQualifyingVega('RMBS_A', '5Y', 200);
  sens.addEquityDelta('AAPL', 10_000);
  sens.addEquityVega('AAPL', 800);
  sens.addFxDelta('EUR', 5_000);
  sens.addFxVega('EUR', 'USD', 300);
  sens.addCommodityDelta('Crude', 2_000);
  sens.addCommodityVega('Crude', 150);
  sens.addCurvature({
    risk_class: 'equity',
    bucket: 'residual',
    factor: 'AAPL',
    expiry_tenor: '1Y',
    volatility_weighted_vega: 1_000,
  });
  return sens;
}

function frtbSensitivities() {
  const sens = new margin.FrtbSensitivities('USD');
  sens.addGirrDelta('5Y', 100_000);
  sens.addGirrDelta('10Y', -40_000, 'EUR');
  sens.addGirrInflationDelta(5_000);
  sens.addGirrXccyBasisDelta(2_000, 'EUR');
  sens.addGirrVega('1Y', '5Y', 3_000);
  sens.addGirrCurvature(-1_500, -2_500);
  sens.addCsrNonsecDelta('ACME', 3, '5Y', 'bond', 5_000);
  sens.addCsrNonsecVega('ACME', 3, '1Y', 700);
  sens.addCsrNonsecCurvature('ACME', 3, -300, -500);
  sens.addCsrSecCtpDelta('TRANCHE_A', 2, '5Y', 'cds', 1_200);
  sens.addCsrSecCtpVega('TRANCHE_A', 2, '1Y', 150);
  sens.addCsrSecCtpCurvature('TRANCHE_A', 2, -80, -120);
  sens.addCsrSecNonctpDelta('RMBS_A', 1, '5Y', 'bond', 900);
  sens.addCsrSecNonctpVega('RMBS_A', 1, '1Y', 110);
  sens.addCsrSecNonctpCurvature('RMBS_A', 1, -60, -90);
  sens.addEquityDelta('AAPL', 1, 25_000);
  sens.addEquityRepoDelta('AAPL', 1, 1_000);
  sens.addEquityVega('AAPL', 1, '1Y', 2_000);
  sens.addEquityCurvature('AAPL', 1, -1_000, -2_000);
  sens.addFxDelta('EUR', 'USD', 10_000);
  sens.addFxVega('EUR', 'USD', '1Y', 400);
  sens.addFxCurvature('EUR', 'USD', -200, -350);
  sens.addCommodityDelta('WTI', 2, '1Y', 'cushing', 2_000);
  sens.addCommodityVega('WTI', 2, '1Y', 250);
  sens.addCommodityCurvature('WTI', 2, -100, -180);
  sens.addDrcPosition('ACME', 1_000_000, 4, 'corporate', 'senior_unsecured', 'corporate', 2.0);
  sens.addDrcPosition('ACME', -250_000, 4, 'corporate', 'subordinated', 'corporate', 0.5, -1_000);
  sens.addRraoPosition('EXOTIC_1', 1_000_000, true);
  sens.addRraoPosition('GAP_1', 2_000_000);
  return sens;
}

test('constants() is the Rust MarginConstants value Python publishes as CONSTANTS', () => {
  close(margin.constants(), golden.constants, 'constants');
});

test('enum-variant twins return the wire labels and fromStr validates', () => {
  const labels = golden.labels;
  assert.deepEqual(
    [
      margin.imMethodologyHaircut(),
      margin.imMethodologySimm(),
      margin.imMethodologySchedule(),
      margin.imMethodologyInternalModel(),
      margin.imMethodologyClearingHouse(),
      margin.imMethodologyFromStr('simm'),
    ],
    labels.im_methodology
  );
  assert.deepEqual(
    [
      margin.marginTenorDaily(),
      margin.marginTenorWeekly(),
      margin.marginTenorMonthly(),
      margin.marginTenorOnDemand(),
      margin.marginTenorFromStr('weekly'),
    ],
    labels.margin_tenor
  );
  assert.deepEqual(
    [
      margin.marginCallTypeInitialMargin(),
      margin.marginCallTypeVariationMarginPost(),
      margin.marginCallTypeVariationMarginCollect(),
      margin.marginCallTypeTopUp(),
      margin.marginCallTypeSubstitution(),
      margin.marginCallTypeFromStr('top_up'),
    ],
    labels.margin_call_type
  );
  assert.deepEqual(
    [
      margin.collateralAssetClassCash(),
      margin.collateralAssetClassGovernmentBonds(),
      margin.collateralAssetClassAgencyBonds(),
      margin.collateralAssetClassCoveredBonds(),
      margin.collateralAssetClassCorporateBonds(),
      margin.collateralAssetClassEquity(),
      margin.collateralAssetClassGold(),
      margin.collateralAssetClassMutualFunds(),
      margin.collateralAssetClassFromStr('gold'),
    ],
    labels.collateral_asset_class
  );
  for (const fromStr of [
    margin.imMethodologyFromStr,
    margin.marginTenorFromStr,
    margin.marginCallTypeFromStr,
    margin.collateralAssetClassFromStr,
  ]) {
    throwsKind(() => fromStr('SIMM!'), 'validation');
    throwsKind(() => fromStr(5), 'invalid_type');
  }
});

test('clearing status and netting-set id factories return the serde values', () => {
  assert.equal(margin.clearingStatusBilateral(), 'bilateral');
  assert.deepEqual(margin.clearingStatusCleared('LCH'), { cleared: { ccp: 'LCH' } });
  close(margin.nettingSetIdBilateral('CPTY', 'CSA'), golden.netting_set_id.bilateral, 'bilateral');
  close(margin.nettingSetIdCleared('LCH'), golden.netting_set_id.cleared, 'cleared');
});

test('collateral asset class haircuts match Python', () => {
  for (const [label, haircut] of Object.entries(golden.collateral_asset_class.standard_haircut)) {
    close(margin.collateralAssetClassStandardHaircut(label), haircut, `standard_haircut.${label}`);
  }
  for (const [label, addon] of Object.entries(golden.collateral_asset_class.fx_addon)) {
    close(margin.collateralAssetClassFxAddon(label), addon, `fx_addon.${label}`);
  }
  throwsKind(() => margin.collateralAssetClassStandardHaircut('bitcoin'), 'validation');
});

test('CsaSpec twins build, amend, apply and validate plain specifications', () => {
  const csa = golden.csa;
  const usd = margin.csaSpecUsdRegulatory();
  close(usd, csa.usd, 'usd');
  close(usd, JSON.parse(margin.csaUsdRegulatoryJson()), 'usd vs json twin');
  close(margin.csaSpecEurRegulatory(), csa.eur, 'eur');
  close(margin.csaSpecRegulatory('GBP', 'GBP-CSA', 'GBP-SONIA'), csa.regulatory_gbp, 'gbp');
  close(
    margin.csaSpecWithVmThreshold(usd, 1_000_000, 250_000, 10_000, 500_000),
    csa.with_vm_threshold,
    'with_vm_threshold'
  );
  close(
    margin.csaSpecWithVmThreshold(JSON.stringify(usd), 1_000_000, 250_000),
    csa.with_vm_threshold_defaults,
    'with_vm_threshold defaults'
  );
  close(margin.csaSpecWithIm(usd, 'schedule', 10, 50_000_000, 500_000, false), csa.with_im, 'im');
  close(
    margin.csaSpecWithIm(usd, margin.imMethodologySimm(), 10, 0, 0),
    csa.with_im_default_segregated,
    'im default segregated'
  );

  const applied = margin.csaSpecApplyImTerms(usd, 60_000_000, 5_000_000);
  close(
    {
      gross_initial_margin: amount(applied.gross_initial_margin),
      required_collateral: amount(applied.required_collateral),
      current_collateral: amount(applied.current_collateral),
      transfer: amount(applied.transfer),
      currency: applied.transfer.currency,
      segregated: applied.segregated,
    },
    csa.apply_im_terms,
    'apply_im_terms'
  );

  assert.equal(margin.csaSpecValidate(usd), undefined);
  throwsKind(() => margin.csaSpecValidate({ ...usd, calendar_id: 'nowhere' }), 'validation');
  throwsKind(() => margin.csaSpecValidate({ ...usd, surprise: 1 }), 'validation');
  throwsKind(() => margin.csaSpecRegulatory('XXX', 'id', 'curve'), 'validation');
  throwsKind(() => margin.csaSpecWithIm(usd, 'simm', 10.5, 0, 0), 'invalid_type');
});

test('EligibleCollateralSchedule twins match Python', () => {
  const schedule = golden.schedule;
  const bcbs = margin.eligibleCollateralScheduleBcbsStandard();
  const cashOnly = margin.eligibleCollateralScheduleCashOnly();
  close(bcbs, schedule.bcbs, 'bcbs');
  close(cashOnly, schedule.cash_only, 'cash_only');
  close(margin.eligibleCollateralScheduleUsTreasuries(), schedule.us_treasuries, 'us_treasuries');
  assert.deepEqual(
    [
      margin.eligibleCollateralScheduleIsEligible(bcbs, 'equity'),
      margin.eligibleCollateralScheduleIsEligible(cashOnly, 'equity'),
    ],
    schedule.is_eligible
  );
  close(
    [
      margin.eligibleCollateralScheduleHaircutFor(bcbs, 'government_bonds'),
      margin.eligibleCollateralScheduleHaircutFor(cashOnly, 'equity'),
    ],
    schedule.haircut_for,
    'haircut_for'
  );
  close(
    [
      margin.eligibleCollateralScheduleHaircutForMaturity(bcbs, 'government_bonds', 0.5),
      margin.eligibleCollateralScheduleHaircutForMaturity(bcbs, 'government_bonds', 7.0),
    ],
    schedule.haircut_for_maturity,
    'haircut_for_maturity'
  );
  close(
    margin.eligibleCollateralScheduleCheckConcentrationLimits(schedule.limited, [
      ['equity', 80],
      ['cash', 20],
    ]),
    schedule.breaches,
    'breaches'
  );
  close(
    margin.eligibleCollateralScheduleCheckConcentrationLimits(bcbs, [
      ['government_bonds', 80],
      ['cash', 20],
    ]),
    schedule.no_breaches,
    'no_breaches'
  );
  throwsKind(
    () => margin.eligibleCollateralScheduleCheckConcentrationLimits(bcbs, [['equity', '80']]),
    'validation'
  );
});

test('margin metric constructors and methods match Python', () => {
  const metrics = golden.metrics;
  const utilization = margin.marginUtilization(8_000_000, 10_000_000, 'USD');
  close(utilization, metrics.utilization.wire, 'utilization');
  close(margin.marginUtilizationRatio(utilization), metrics.utilization.ratio, 'ratio');
  assert.equal(margin.marginUtilizationIsAdequate(utilization), metrics.utilization.is_adequate);
  close(margin.marginUtilizationShortfall(utilization), metrics.utilization.shortfall, 'shortfall');
  assert.equal(margin.marginUtilizationRatio(margin.marginUtilization(1, 0, 'USD')), Infinity);
  throwsKind(() => margin.marginUtilization(-1, 10, 'USD'), 'validation');

  const excess = margin.excessCollateral(12_000_000, 10_000_000, 'USD');
  close(excess, metrics.excess.wire, 'excess');
  assert.equal(margin.excessCollateralHasExcess(excess), metrics.excess.has_excess);
  assert.equal(margin.excessCollateralHasShortfall(excess), metrics.excess.has_shortfall);
  close(
    margin.excessCollateralExcessPercentage(excess),
    metrics.excess.excess_percentage,
    'excess_percentage'
  );

  const fundingCost = margin.marginFundingCost(10_000_000, 0.05, 0.03, 'USD');
  close(fundingCost, metrics.funding_cost.wire, 'funding_cost');
  close(margin.marginFundingCostSpread(fundingCost), metrics.funding_cost.spread, 'spread');
  close(
    margin.marginFundingCostCostForPeriod(fundingCost, 0.25),
    metrics.funding_cost.cost_for_period,
    'cost_for_period'
  );

  const haircut01 = margin.haircut01(10_000_000, 0.02, 'USD');
  close(haircut01, metrics.haircut01.wire, 'haircut01');
  close(margin.haircut01HaircutBp(haircut01), metrics.haircut01.haircut_bp, 'haircut_bp');
});

test('FundingConfig, profile and decay twins match Python', () => {
  const xva = golden.xva;
  close(
    margin.fundingConfigEffectiveBenefitBp(xva.funding.wire),
    xva.funding.effective_benefit_bp,
    'benefit'
  );
  close(
    margin.fundingConfigEffectiveMarginSpreadBp(xva.funding.wire),
    xva.funding.effective_margin_spread_bp,
    'margin spread'
  );
  close(
    margin.fundingConfigEffectiveBenefitBp(xva.funding.symmetric_wire),
    xva.funding.symmetric_benefit_bp,
    'symmetric benefit'
  );
  close(
    margin.fundingConfigEffectiveMarginSpreadBp(xva.funding.symmetric_wire),
    xva.funding.symmetric_margin_spread_bp,
    'symmetric margin spread'
  );
  throwsKind(() => margin.fundingConfigEffectiveBenefitBp({ funding_spread_bp: -1 }), 'validation');

  const constant = margin.imDecayProfileConstant();
  const linear = margin.imDecayProfileLinearToMaturity(5.0);
  const sqrtTime = margin.imDecayProfileSqrtTime(4.0);
  close(constant, xva.decay.constant, 'constant');
  close(linear, xva.decay.linear, 'linear');
  close(sqrtTime, xva.decay.sqrt_time, 'sqrt_time');
  close(
    [
      margin.imDecayProfileFactor(constant, 3.0),
      margin.imDecayProfileFactor(linear, 3.0),
      margin.imDecayProfileFactor(sqrtTime, 3.0),
    ],
    xva.decay.factors,
    'factors'
  );
  close(
    margin.imDecayProfileFactor('{"linear_to_maturity":{"maturity_years":5.0}}', 3.0),
    xva.decay.factors[1],
    'factor from JSON text'
  );
  throwsKind(() => margin.imDecayProfileLinearToMaturity(0), 'validation', /maturity_years/);
  throwsKind(() => margin.imDecayProfileFactor('exponential', 1.0), 'validation');

  assert.equal(margin.imProfileValidate({ times: [1, 2], im_values: [10, 5] }), undefined);
  throwsKind(
    () => margin.imProfileValidate({ times: [2, 1], im_values: [10, 5] }),
    'validation',
    /strictly increasing/
  );
  assert.equal(
    margin.exposureProfileValidate({ times: [1, 2], mtm_values: [0, 0], epe: [0, 0], ene: [0, 0] }),
    undefined
  );
  throwsKind(
    () =>
      margin.exposureProfileValidate({ times: [1, 2], mtm_values: [0], epe: [0, 0], ene: [0, 0] }),
    'validation',
    /vector lengths must be equal/
  );
});

test('SimmSensitivities and SimmCalculator match Python', () => {
  const simm = golden.simm;
  const sens = simmSensitivities();
  close(wire(sens.toJson()), simm.wire, 'wire');
  assert.equal(sens.baseCurrency, simm.base_currency);
  assert.deepEqual([new margin.SimmSensitivities('USD').isEmpty(), sens.isEmpty()], simm.is_empty);
  close(sens.totalIrDelta(), simm.total_ir_delta, 'total_ir_delta');
  close(sens.totalEquityDelta(), simm.total_equity_delta, 'total_equity_delta');
  close(sens.scaled(2).totalIrDelta(), simm.scaled_total_ir_delta, 'scaled');
  const inEur = sens.scaledToCurrency('EUR', 0.9);
  close(
    { base_currency: inEur.baseCurrency, total_ir_delta: inEur.totalIrDelta() },
    simm.eur,
    'eur'
  );
  const merged = simmSensitivities();
  merged.merge(sens);
  close(merged.totalIrDelta(), simm.merged_total_ir_delta, 'merged');
  assert.equal(sens.validate(), undefined);

  const roundTrip = margin.SimmSensitivities.fromJson(sens.toJson());
  close(wire(roundTrip.toJson()), simm.wire, 'round trip');
  close(wire(margin.SimmSensitivities.fromJson(simm.wire).toJson()), simm.wire, 'from object');

  const calculator = new margin.SimmCalculator();
  close(
    { version: calculator.version, mpor_days: calculator.mporDays },
    simm.calculator,
    'calculator'
  );
  assert.equal(new margin.SimmCalculator('v2_6', 5).mporDays, simm.calculator_mpor_override);
  const im = calculator.calculateFromSensitivities(sens, 'USD', '2025-01-15');
  close(wire(im), simm.im, 'im');
  assert.deepEqual(margin.imResultBreakdownKeys(im), simm.breakdown_keys);
  close(margin.imResultBreakdownAmount(im, 'IR_Delta'), simm.breakdown_ir_delta, 'IR_Delta');
  assert.equal(margin.imResultBreakdownAmount(im, 'nope'), undefined);

  throwsKind(
    () => new margin.SimmSensitivities('USD').merge(new margin.SimmSensitivities('EUR')),
    'validation',
    /cannot merge SIMM sensitivities in EUR into a USD container/
  );
  throwsKind(() => new margin.SimmSensitivities('ZZZ'), 'validation');
  throwsKind(() => new margin.SimmCalculator('v9'), 'validation');
  const typo = new margin.SimmSensitivities('USD');
  typo.addIrDelta('USD', '7Y', 1);
  throwsKind(() => typo.validate(), 'validation');
  throwsKind(() => sens.addCurvature({ risk_class: 'equity' }), 'validation');
});

test('imProfileFromSimm and computeMva match Python', () => {
  const sens = simmSensitivities();
  const calculator = new margin.SimmCalculator();
  const profile = margin.imProfileFromSimm(
    calculator,
    sens,
    'USD',
    margin.imDecayProfileLinearToMaturity(5.0),
    [1.0, 2.0, 4.0]
  );
  close(profile, golden.mva.profile, 'profile');

  const discount = core.DiscountCurve.flat('USD-OIS', '2025-01-01', 0.03);
  const hazard = core.HazardCurve.flat('BANK', '2025-01-01', 0.02, 0.4);
  const spreads = [
    [0.0, 50.0],
    [5.0, 80.0],
  ];
  close(
    margin.computeMva(profile, spreads, discount),
    golden.mva.without_survival,
    'without_survival'
  );
  close(
    margin.computeMva(profile, spreads, discount, null),
    golden.mva.without_survival,
    'null survival'
  );
  close(
    margin.computeMva(JSON.stringify(profile), spreads, discount, hazard),
    golden.mva.with_survival,
    'with_survival'
  );
  throwsKind(
    () => margin.imProfileFromSimm(calculator, sens, 'USD', 'constant', [2.0, 1.0]),
    'validation'
  );
});

test('ScheduleImCalculator matches Python', () => {
  const expected = golden.schedule_im;
  const schedule = margin.ScheduleImCalculator.bcbsStandard();
  close(schedule.rate('interest_rate', 5.0), expected.rate, 'rate');
  close(
    {
      asset_class: schedule.defaultAssetClass,
      maturity_years: schedule.defaultMaturityYears,
      mpor_days: schedule.mporDays,
    },
    expected.defaults,
    'defaults'
  );
  assert.equal(schedule.withAssetClass('credit').defaultAssetClass, expected.with_asset_class);
  close(schedule.withMaturity(2.0).defaultMaturityYears, expected.with_maturity, 'with_maturity');
  close(
    margin.ScheduleImCalculator.fromRegistryId(golden.constants.BCBS_IOSCO_SCHEDULE_ID).rate(
      'credit',
      3.0
    ),
    expected.from_registry_rate,
    'from_registry_rate'
  );
  close(
    wire(schedule.calculateForNotional(1_000_000, 'USD', 'interest_rate', 5.0, '2025-01-15')),
    expected.for_notional,
    'for_notional'
  );
  close(
    wire(
      schedule.calculateNettingSetWithNgr(
        [
          [2e6, 1e8, 'interest_rate', 5.0],
          [-1.5e6, 8e7, 'credit', 3.0],
        ],
        'USD',
        '2025-01-15'
      )
    ),
    expected.ngr,
    'ngr'
  );
  assert.equal(schedule.calculateNettingSetWithNgr([], 'USD', '2025-01-15'), undefined);
  throwsKind(() => margin.ScheduleImCalculator.fromRegistryId('nope'), 'not_found');
  throwsKind(() => schedule.withMaturity(-1), 'validation');
});

test('HaircutImCalculator matches Python', () => {
  const expected = golden.haircut_im;
  const haircut = margin.HaircutImCalculator.bcbsStandard();
  close(haircut.haircutFor('cash'), expected.haircut_for_cash, 'haircut_for_cash');
  assert.equal(haircut.defaultAssetClass, expected.default_asset_class);
  assert.equal(
    haircut.withDefaultAssetClass('government_bonds').defaultAssetClass,
    expected.with_default_asset_class
  );
  close(
    [
      haircut.postedCollateralCurrency,
      haircut.withPostedCollateralCurrency('EUR').postedCollateralCurrency,
    ],
    expected.posted_collateral_currency,
    'posted_collateral_currency'
  );
  assert.equal(haircut.mporDays, expected.mpor_days);
  close(
    margin.HaircutImCalculator.usTreasuries()
      .withCollateralTerms(7.0, 'AAA')
      .haircutFor('government_bonds'),
    expected.treasuries_terms_haircut,
    'treasuries_terms_haircut'
  );
  close(
    margin.HaircutImCalculator.fromSchedule(golden.schedule.cash_only).haircutFor('cash'),
    expected.from_schedule_haircut,
    'from_schedule_haircut'
  );
  close(haircut.eligibleCollateral, expected.eligible_collateral, 'eligible_collateral');
  close(
    wire(haircut.calculateForCollateral(10_000_000, 'USD', 'cash', true, '2025-01-15')),
    expected.for_collateral,
    'for_collateral'
  );
  throwsKind(
    () => haircut.calculateForCollateral(10_000_000, 'USD', 'cash', 'yes', '2025-01-15'),
    'invalid_type'
  );
  throwsKind(() => haircut.withCollateralTerms(-1), 'validation');
});

test('VmCalculator and the VmResult twins match Python', () => {
  const expected = golden.vm;
  const calc = new margin.VmCalculator(margin.csaSpecUsdRegulatory());
  assert.equal(calc.csa.id, expected.csa_id);
  const result = calc.calculate(1_000_000, 0, 'USD', '2024-06-17');
  close(result, expected.result, 'result');
  close(result, margin.calculateVm(calc.csa, 1_000_000, 0, 'USD', '2024-06-17'), 'calculateVm');
  close(margin.vmResultNetMargin(result), expected.net_margin, 'net_margin');
  assert.equal(margin.vmResultRequiresCall(result), expected.requires_call);

  const calls = calc.generateMarginCalls(
    [
      ['2024-06-17', 1_000_000],
      ['2024-06-18', 2_500_000],
      ['2024-06-19', 500_000],
    ],
    0
  );
  close(
    calls.map((call) => ({
      call_date: call.call_date,
      settlement_date: call.settlement_date,
      call_type: call.call_type,
      amount: amount(call.amount),
      mtm_trigger: amount(call.mtm_trigger),
      threshold: amount(call.threshold),
      mta_applied: amount(call.mta_applied),
      currency: call.amount.currency,
    })),
    expected.calls,
    'calls'
  );
  assert.deepEqual(calc.marginCallDates('2024-06-17', '2024-06-21'), expected.call_dates);
  throwsKind(() => calc.calculate(1_000_000, 0, 'EUR', '2024-06-17'), 'validation');
  throwsKind(() => new margin.VmCalculator({ id: 'x' }), 'validation');
});

test('FrtbSensitivities, FrtbSbaEngine and frtbSbaCharge match Python', () => {
  const expected = golden.frtb;
  const sens = frtbSensitivities();
  close(wire(sens.toJson()), expected.wire, 'wire');
  assert.equal(sens.baseCurrency, expected.base_currency);
  assert.equal(sens.validate(), undefined);
  close(wire(margin.FrtbSensitivities.fromJson(sens.toJson()).toJson()), expected.wire, 'trip');

  close(wire(margin.frtbSbaCharge(sens)), expected.charge, 'charge');
  close(wire(margin.frtbSbaCharge(sens, 'high')), expected.charge_high, 'charge_high');

  const engine = new margin.FrtbSbaEngine(['low', 'high'], ['girr', 'fx']);
  close(
    { scenarios: engine.scenarios, risk_classes: engine.riskClasses },
    expected.engine,
    'engine'
  );
  const all = new margin.FrtbSbaEngine();
  close(
    { scenarios: all.scenarios, risk_classes: all.riskClasses },
    expected.engine_default,
    'engine_default'
  );
  close(wire(engine.calculate(sens)), expected.engine_charge, 'engine_charge');

  throwsKind(
    () => margin.frtbSbaCharge(sens, 'extreme'),
    'validation',
    /unknown variant `extreme`/
  );
  throwsKind(() => new margin.FrtbSbaEngine([]), 'validation', /at least one correlation scenario/);
  throwsKind(() => sens.addEquityDelta('AAPL', 1.5, 1), 'invalid_type');
  throwsKind(
    () => sens.addDrcPosition('X', 1, 4, 'nope', 'senior_unsecured', 'corporate', 1),
    'validation'
  );
});

test('SA-CCR configuration twins, SaCcrEngine and saccrEad match Python', () => {
  const expected = golden.saccr;
  const nettingSet = margin.nettingSetIdBilateral('CPTY', 'CSA');
  const unmargined = margin.saCcrNettingSetConfigUnmargined(nettingSet, 0, '2025-01-15');
  const margined = margin.saCcrNettingSetConfigMargined(
    nettingSet,
    10_000,
    50_000,
    5_000,
    0,
    10,
    '2025-01-15'
  );
  close(unmargined, expected.unmargined, 'unmargined');
  close(margined, expected.margined, 'margined');
  assert.equal(margin.saCcrNettingSetConfigValidate(margined), undefined);
  throwsKind(
    () => margin.saCcrNettingSetConfigMargined(nettingSet, 0, -1, 0, 0, 10, '2025-01-15'),
    'validation'
  );

  const trades = expected.trades;
  close(wire(margin.saccrEad(trades, unmargined)), expected.ead_unmargined, 'ead_unmargined');
  close(wire(margin.saccrEad(trades, margined)), expected.ead_margined, 'ead_margined');
  close(wire(margin.saccrEad(trades, unmargined, 1.5)), expected.ead_alpha, 'ead_alpha');
  assert.deepEqual(
    [new margin.SaCcrEngine().alpha, new margin.SaCcrEngine(1.5).alpha],
    expected.engine_alpha
  );
  close(
    wire(new margin.SaCcrEngine(1.5).calculateEad(unmargined, JSON.stringify(trades))),
    expected.engine_ead,
    'engine_ead'
  );
  throwsKind(() => margin.saccrEad(trades, unmargined, 0.5), 'validation', /alpha/);
  throwsKind(() => new margin.SaCcrEngine(0.5), 'validation');
});

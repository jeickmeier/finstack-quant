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
import init, { core, margin, valuations } from '../../index.js';

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
  sens.addGirrDelta('USD', 'USD-OIS', '5Y', 100_000);
  sens.addGirrDelta('EUR', 'EUR-ESTR', '10Y', -40_000);
  sens.addGirrInflationDelta('USD', 5_000);
  sens.addGirrXccyBasisDelta('EUR', 2_000);
  sens.addGirrVega('USD', '1Y', '5Y', 3_000);
  sens.addGirrCurvature('USD', -1_500, -2_500);
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
  sens.addDrcPosition({
    issuer: 'ACME',
    jtd_amount: 1_000_000,
    rating_bucket: 4,
    sector: 'corporate',
    seniority: 'senior_unsecured',
    maturity_years: 2.0,
    pnl_adjustment: 0.0,
  });
  sens.addDrcPosition({
    issuer: 'ACME',
    jtd_amount: -250_000,
    rating_bucket: 4,
    sector: 'corporate',
    seniority: 'subordinated',
    maturity_years: 0.5,
    pnl_adjustment: -1_000,
  });
  sens.addRraoPosition('EXOTIC_1', 1_000_000, true);
  sens.addRraoPosition('GAP_1', 2_000_000, false);
  return sens;
}

/** Config carrying the margin-registry overlay shared with the Python golden generator. */
function marginOverlayConfig() {
  const config = new core.FinstackConfig();
  config.setExtension('margin.registry.v1', { defaults: { defaults: { vm: { mta: 250_000 } } } });
  return config;
}

test('calculators build from a FinstackConfig registry overlay', () => {
  const empty = new core.FinstackConfig();
  assert.equal(
    margin.SimmCalculator.fromFinstackConfig('v2_6', empty).mporDays,
    new margin.SimmCalculator('v2_6').mporDays
  );
  assert.equal(
    margin.ScheduleImCalculator.fromFinstackConfig(empty).mporDays,
    margin.ScheduleImCalculator.bcbsStandard().mporDays
  );
  const broken = new core.FinstackConfig();
  broken.setExtension('margin.registry.v1', { simm: { not_a_section: 1 } });
  throwsKind(() => margin.SimmCalculator.fromFinstackConfig('v2_6', broken), 'validation');
});

test('constants() is the Rust MarginConstants value Python publishes as CONSTANTS', () => {
  close(margin.constants(), golden.constants, 'constants');
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
    margin.csaSpecWithIm(usd, 'simm', 10, 0, 0),
    csa.with_im_default_segregated,
    'im default segregated'
  );
  const cleared = margin.csaSpecWithIm(usd, 'clearing_house', 5, 0, 0);
  close(cleared, csa.with_im_cleared_default_segregated, 'im cleared default segregated');
  assert.equal(cleared.im_params.segregated, false);
  close(
    margin.csaSpecRegulatoryFromConfig(marginOverlayConfig(), 'USD', 'CSA-CFG', 'USD-OIS'),
    csa.regulatory_from_config,
    'regulatory_from_config'
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
  close(
    margin.eligibleCollateralScheduleFromFinstackConfig(marginOverlayConfig(), 'bcbs_standard'),
    schedule.from_finstack_config,
    'from_finstack_config'
  );
  throwsKind(
    () => margin.eligibleCollateralScheduleFromFinstackConfig(new core.FinstackConfig(), 'nowhere'),
    'validation'
  );
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
  close(
    margin.marginFundingCostSpreadRate(fundingCost),
    metrics.funding_cost.spread,
    'spread rate'
  );
  close(margin.marginFundingCostSpreadBp(fundingCost), 200, 'spread basis points');
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
    margin.fundingConfigEffectiveBenefitBp(xva.funding.symmetric_wire),
    xva.funding.symmetric_benefit_bp,
    'symmetric benefit'
  );
  throwsKind(() => margin.fundingConfigEffectiveBenefitBp({ funding_spread_bp: -1 }), 'validation');

  const constant = 'constant';
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

  const im = {
    as_of: '2025-01-01',
    currency: 'USD',
    basis: {
      kind: 'external',
      source_id: 'supplied-im',
      netting_set: {
        kind: 'bilateral',
        counterparty_id: 'bank',
        csa_id: 'agreement',
      },
    },
    dates: ['2025-01-01', '2026-01-01'],
    im_values: [10, 5],
  };
  assert.equal(margin.imProfileValidate(im), undefined);
  throwsKind(
    () => margin.imProfileValidate({ ...im, dates: ['2025-01-01', '2025-01-01'] }),
    'validation',
    /strictly increase/
  );
  const exposure = {
    as_of: '2025-01-01',
    currency: 'USD',
    basis: { kind: 'external', source_id: 'parity-test', policy_id: 'unsecured' },
    dates: ['2025-01-01', '2026-01-01'],
    times: [0, 1],
    mtm_values: [0, 0],
    epe: [0, 0],
    ene: [0, 0],
    diagnostics: { market_roll_failures: 0, valuation_failures: 0, total_time_points: 2 },
  };
  assert.equal(margin.exposureProfileValidate(exposure), undefined);
  throwsKind(
    () => margin.exposureProfileValidate({ ...exposure, mtm_values: [0] }),
    'validation',
    /vector lengths must be equal/
  );
  throwsKind(
    () => margin.exposureProfileValidate({ ...exposure, times: [0.5, 1] }),
    'validation',
    /explicit time-zero/
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
  const projection = {
    as_of: '2025-01-01',
    currency: 'USD',
    scope_source: 'shared-parity-risk',
    netting_set: { kind: 'bilateral', counterparty_id: 'bank', csa_id: 'agreement' },
    dates: ['2025-01-01', '2026-01-01', '2027-01-01', '2028-12-31'],
    decay: margin.imDecayProfileLinearToMaturity(5.0),
  };
  const profile = margin.imProfileFromSimm(calculator, sens, projection);
  close(profile, golden.mva.profile, 'profile');

  const discount = core.DiscountCurve.flat('USD-OIS', '2025-01-01', 0.03, 'act_365f');
  const hazard = core.HazardCurve.flat('BANK', '2025-01-01', 0.02, 0.4, 'act_365f');
  const spreads = {
    kind: 'dated',
    knots: [
      ['2025-01-01', 50],
      ['2030-01-01', 80],
    ],
  };
  assert.throws(
    () => margin.computeMva(profile, spreads, discount),
    /Choose a hazard curve or null/
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
    () =>
      margin.imProfileFromSimm(calculator, sens, {
        ...projection,
        dates: [...projection.dates].reverse(),
      }),
    'validation'
  );
});

test('ScheduleImCalculator matches Python', () => {
  const expected = golden.schedule_im;
  const schedule = margin.ScheduleImCalculator.bcbsStandard();
  close(schedule.rate('interest_rate', 5.0), expected.rate, 'rate');
  close(
    {
      mpor_days: schedule.mporDays,
    },
    expected.defaults,
    'defaults'
  );
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
});

test('HaircutImCalculator matches Python', () => {
  const expected = golden.haircut_im;
  const haircut = margin.HaircutImCalculator.bcbsStandard();
  close(haircut.haircutFor('cash'), expected.haircut_for_cash, 'haircut_for_cash');
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

test('FrtbSensitivities and FrtbSbaEngine match Python', () => {
  const expected = golden.frtb;
  const sens = frtbSensitivities();
  close(wire(sens.toJson()), expected.wire, 'wire');
  assert.equal(sens.baseCurrency, expected.base_currency);
  assert.equal(sens.validate(), undefined);
  close(wire(margin.FrtbSensitivities.fromJson(sens.toJson()).toJson()), expected.wire, 'trip');

  close(wire(new margin.FrtbSbaEngine().calculate(sens)), expected.charge, 'charge');
  close(
    wire(new margin.FrtbSbaEngine(['high']).calculate(sens)),
    expected.charge_high,
    'charge_high'
  );

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
    () => new margin.FrtbSbaEngine(['extreme']),
    'validation',
    /unknown variant `extreme`/
  );
  throwsKind(() => new margin.FrtbSbaEngine([]), 'validation', /at least one correlation scenario/);
  throwsKind(() => sens.addEquityDelta('AAPL', 1.5, 1), 'invalid_type');
  throwsKind(
    () =>
      sens.addDrcPosition({
        issuer: 'X',
        jtd_amount: 1,
        rating_bucket: 4,
        sector: 'nope',
        seniority: 'senior_unsecured',
        maturity_years: 1,
        pnl_adjustment: 0.0,
      }),
    'validation'
  );
});

test('SA-CCR configuration twins and SaCcrEngine match Python', () => {
  const expected = golden.saccr;
  const nettingSet = { kind: 'bilateral', counterparty_id: 'CPTY', csa_id: 'CSA' };
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
  const saCcr = new margin.SaCcrEngine();
  close(wire(saCcr.calculateEad(trades, unmargined)), expected.ead_unmargined, 'ead_unmargined');
  close(wire(saCcr.calculateEad(trades, margined)), expected.ead_margined, 'ead_margined');
  close(
    wire(new margin.SaCcrEngine(1.5).calculateEad(trades, unmargined)),
    expected.ead_alpha,
    'ead_alpha'
  );
  assert.deepEqual(
    [new margin.SaCcrEngine().alpha, new margin.SaCcrEngine(1.5).alpha],
    expected.engine_alpha
  );
  close(
    wire(new margin.SaCcrEngine(1.5).calculateEad(JSON.stringify(trades), unmargined)),
    expected.engine_ead,
    'engine_ead'
  );
  throwsKind(() => new margin.SaCcrEngine(0.5), 'validation', /alpha/);
});

test('addRraoPosition requires the exotic flag', () => {
  const sens = new margin.FrtbSensitivities('USD');
  throwsKind(() => sens.addRraoPosition('X', 1_000_000), 'invalid_type');
  sens.addRraoPosition('X', 1_000_000, false);
});

test('GIRR requires explicit risk currency and DRC delegates complete Rust validation', () => {
  const sens = new margin.FrtbSensitivities('USD');
  assert.throws(() => sens.addGirrDelta('5Y', 100));
  sens.addGirrDelta('EUR', 'EUR-ESTR', '5Y', 100);
  assert.deepEqual(JSON.parse(sens.toJson()).girr_delta, [['EUR', 'EUR-ESTR', '5Y', 100]]);
  const position = {
    issuer: 'ACME',
    jtd_amount: 1_000_000,
    rating_bucket: 4,
    sector: 'corporate',
    seniority: 'senior_unsecured',
    maturity_years: 2,
  };
  for (const change of [
    { maturity_years: -1 },
    { rating_bucket: 0 },
    { seniority: 'equity', sector: 'sovereign' },
    { asset_type: 'corporate' },
  ]) {
    assert.throws(() => sens.addDrcPosition({ ...position, ...change }));
  }
  assert.deepEqual(JSON.parse(sens.toJson()).drc_positions, []);
  sens.addDrcPosition(position);
  assert.equal(new margin.FrtbSbaEngine().calculate(sens).drc, 45_000);
});

test('instrument roundtrip preserves the CSA-owned no-IM election', () => {
  const csa = margin.csaSpecUsdRegulatory();
  csa.im_params = null;
  csa.vm_params.frequency = 'weekly';
  csa.vm_params.settlement_lag = 3;
  const spec = { csa, clearing_status: 'bilateral' };
  const payload = JSON.parse(valuations.instruments.InterestRateSwap.example().toJson());
  payload.instrument.spec.margin_spec = spec;
  const swap = valuations.instruments.InterestRateSwap.fromJson(JSON.stringify(payload));
  const canonical = { csa: { ...csa }, clearing_status: 'bilateral' };
  delete canonical.csa.im_params;
  assert.deepEqual(swap.marginSpec, canonical);
  assert.deepEqual(JSON.parse(swap.toJson()).instrument.spec.margin_spec, canonical);
  for (const retired of ['im_methodology', 'vm_frequency', 'settlement_lag']) {
    payload.instrument.spec.margin_spec = { ...spec, [retired]: 'simm' };
    assert.throws(
      () => valuations.instruments.InterestRateSwap.fromJson(JSON.stringify(payload)),
      /unknown field/
    );
  }
});

test('standalone SIMM nets opposing contractual sensitivities before calculating IM', () => {
  const asOf = '2023-12-20';
  const snapshot = new core.MarketContext();
  snapshot.insert(core.DiscountCurve.flat('USD-OIS', asOf, 0.04, 'act_365f'));
  snapshot.insert(core.ForwardCurve.flat('USD-SOFR-3M', asOf, 0.045, 'act_360'));
  const payer = JSON.parse(valuations.instruments.InterestRateSwap.example().toJson());
  const receiver = structuredClone(payer);
  receiver.instrument.spec.id = 'RECEIVER';
  receiver.instrument.spec.side = 'receive';
  const single = valuations.calculateSimm([payer], snapshot, asOf);
  const net = valuations.calculateSimm([payer, receiver], snapshot, asOf);
  assert.ok(Number(single.amount.amount) > 0);
  assert.ok(Number(net.amount.amount) < 1e-5);
  assert.throws(() => valuations.calculateSimm([], snapshot, asOf), /empty/);
  assert.throws(() => valuations.calculateSimm([payer], snapshot, asOf, null, 0), /MPOR/);
});

test('typed margin authoring validates agreement terms and the default-risk position', () => {
  const usd = new core.Currency('USD');
  const base = margin.csaSpecUsdRegulatory();
  const withThreshold = margin
    .csaSpecToBuilder(base)
    .vmThreshold(new core.Money(250_000, usd), new core.Money(50_000, usd))
    .imParams(null)
    .build();
  assert.equal(Number(base.vm_params.threshold.amount), 0);
  assert.equal(Number(withThreshold.vm_params.threshold.amount), 250_000);
  assert.equal(Number(withThreshold.vm_params.mta.amount), 50_000);
  assert.equal(withThreshold.im_params, undefined);
  assert.throws(() => margin.csaSpecBuilder('EXPLICIT', 'USD', 'USD-OIS').build(), /calendar_id/);

  const position = margin.drcPosition({
    issuer: 'ACME',
    jtd_amount: 1_000_000,
    rating_bucket: 3,
    sector: 'corporate',
    seniority: 'senior_unsecured',
    maturity_years: 2,
  });
  const sens = new margin.FrtbSensitivities('USD');
  sens.addDrcPosition(position);
  assert.ok(new margin.FrtbSbaEngine().calculate(sens).drc > 0);
  assert.throws(() => margin.drcPosition({ ...position, maturity_years: -1 }), /maturity/);
});

/**
 * Facade tests for the free-function twins of the Python calibration classes
 * and the direct Hull-White calibrators (parity slice P3).
 *
 * Expected values come from
 * `finstack-quant-py/tests/data/calibration_wasm_parity.json`, which holds the
 * Python outputs for the same inputs and is asserted by
 * `finstack-quant-py/tests/test_calibration_wasm_parity.py`, so each case is a
 * cross-host golden: both hosts call the same Rust entry point.
 *
 * Requires the wasm-pack web build: npm run build (mise run wasm-build).
 */

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { calibration, core } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

const GOLDEN = JSON.parse(
  readFileSync(
    new URL('../../../finstack-quant-py/tests/data/calibration_wasm_parity.json', import.meta.url),
    'utf8'
  )
);
const HW = GOLDEN.hull_white;
const BASE = '2026-05-08';
const plain = (value) => JSON.parse(JSON.stringify(value));
const close = (actual, expected, rel, label) =>
  assert.ok(
    Math.abs(actual - expected) <= rel * Math.abs(expected),
    `${label}: ${actual} vs ${expected}`
  );
const validation = (error) => error.name === 'FinstackError' && error.kind === 'validation';

const discountCurve = () =>
  new core.DiscountCurve({
    id: HW.curve.id,
    baseDate: HW.curve.base_date,
    knots: HW.curve.knots,
    dayCount: HW.curve.day_count,
  });

test('rateQuoteDeposit builds the Python RateQuote.deposit object', () => {
  assert.deepEqual(
    plain(calibration.rateQuoteDeposit('D3M', 'USD-SOFR-OIS', '3M', 0.052)),
    GOLDEN.quotes.deposit
  );
  assert.deepEqual(
    plain(
      calibration.rateQuoteDeposit(
        'D3M',
        'USD-SOFR-OIS',
        { tenor: { count: 3, unit: 'months' } },
        0.052
      )
    ),
    GOLDEN.quotes.deposit
  );
  assert.throws(() => calibration.rateQuoteDeposit('D', 'USD-SOFR-OIS', 'soon', 0.05), validation);
  assert.throws(
    () => calibration.rateQuoteDeposit('D', 'USD-SOFR-OIS', '3M', '0.05'),
    (error) => error.kind === 'invalid_type'
  );
});

test('rateQuoteFra builds the Python RateQuote.fra object', () => {
  assert.deepEqual(
    plain(calibration.rateQuoteFra('F', 'USD-SOFR-OIS', '3M', '6M', 0.05)),
    GOLDEN.quotes.fra
  );
});

test('rateQuoteFutures defaults the convexity adjustment in Rust', () => {
  assert.deepEqual(
    plain(calibration.rateQuoteFutures('FUT', 'CME:SR3', '2026-09-15', 96.5)),
    GOLDEN.quotes.futures
  );
  assert.equal(
    calibration.rateQuoteFutures('FUT', 'CME:SR3', '2026-09-15', 96.5, 0.0002).convexity_adjustment,
    0.0002
  );
  assert.throws(
    () => calibration.rateQuoteFutures('FUT', 'CME:SR3', '15/09/2026', 96.5),
    validation
  );
});

test('rateQuoteSwap builds the Python RateQuote.swap object', () => {
  assert.deepEqual(
    plain(calibration.rateQuoteSwap('S5', 'USD-SOFR-OIS', '5Y', 0.045)),
    GOLDEN.quotes.swap
  );
  assert.deepEqual(
    plain(calibration.rateQuoteSwap('S5d', 'USD-SOFR-OIS', '2031-05-08', 0.045, 0.001)),
    GOLDEN.quotes.swap_dated
  );
});

test('cdsQuoteParSpread builds the Python CdsQuote.par_spread object', () => {
  assert.deepEqual(
    plain(calibration.cdsQuoteParSpread('ACME-5Y', 'ACME', 'USD', 'isda_na', '5Y', 80.0, 0.4)),
    GOLDEN.quotes.par_spread
  );
  assert.throws(
    () => calibration.cdsQuoteParSpread('C', 'ACME', 'USD', 'not_a_clause', '5Y', 80.0, 0.4),
    validation
  );
});

test('cdsQuoteUpfront builds the Python CdsQuote.upfront object', () => {
  assert.deepEqual(
    plain(
      calibration.cdsQuoteUpfront('ACME-5Y-U', 'ACME', 'USD', 'isda_na', '5Y', 100.0, 0.01, 0.4)
    ),
    GOLDEN.quotes.upfront
  );
});

test('volQuoteOptionVol defaults the option type in Rust', () => {
  assert.deepEqual(
    plain(calibration.volQuoteOptionVol('O', 'AAPL', '2027-05-08', 155.0, 0.28)),
    GOLDEN.quotes.option_vol
  );
  assert.throws(
    () => calibration.volQuoteOptionVol('O', 'AAPL', '2027-05-08', 155.0, 0.28, 'straddle'),
    validation
  );
});

test('volQuoteSwaptionVol defaults the quote type and convention in Rust', () => {
  assert.deepEqual(
    plain(calibration.volQuoteSwaptionVol('SV', '2027-05-08', '2032-05-08', 0.04, 0.0072)),
    GOLDEN.quotes.swaption_vol
  );
});

test('volQuoteCapFloorVol builds the Python VolQuote.cap_floor_vol object', () => {
  assert.deepEqual(
    plain(calibration.volQuoteCapFloorVol('CF', '2027-05-08', 0.04, 0.0072, 'normal', false)),
    GOLDEN.quotes.cap_floor_vol
  );
  assert.equal(
    calibration.volQuoteCapFloorVol('CF', '2027-05-08', 0.04, 0.0072).cap_floor_vol.is_cap,
    true
  );
});

test('rateBoundsForCurrency and rateBoundsEmergingMarkets return the Rust bounds', () => {
  assert.deepEqual(plain(calibration.rateBoundsForCurrency('USD')), GOLDEN.rate_bounds.USD);
  assert.deepEqual(plain(calibration.rateBoundsForCurrency('JPY')), GOLDEN.rate_bounds.JPY);
  assert.deepEqual(
    plain(calibration.rateBoundsEmergingMarkets()),
    GOLDEN.rate_bounds.emerging_markets
  );
  assert.throws(() => calibration.rateBoundsForCurrency('not-a-currency'), validation);
});

const STEPS = {
  discount: () => calibration.calibrationStepDiscount('USD-OIS', 'USD', BASE),
  discount_named: () =>
    calibration.calibrationStepDiscount('d2', 'USD', BASE, 'qs', 'USD-OIS', {
      interpolation: 'linear',
    }),
  forward: () => calibration.calibrationStepForward('USD-SOFR-3M', 'USD', BASE, 0.25, 'USD-OIS'),
  hazard: () => calibration.calibrationStepHazard('ACME', 'ACME', 'USD', BASE, 'USD-OIS', 0.4),
  inflation: () =>
    calibration.calibrationStepInflation(
      'USA-CPI',
      'USD',
      BASE,
      'USD-OIS',
      'USA-CPI-U',
      '3M',
      310.0
    ),
  vol_surface: () => calibration.calibrationStepVolSurface('AAPL-VOL', BASE, 'AAPL'),
  swaption_vol: () => calibration.calibrationStepSwaptionVol('USD-SWPT', BASE, 'USD-OIS', 'USD'),
  base_correlation: () =>
    calibration.calibrationStepBaseCorrelation(
      'CDX-CORR',
      'CDX.NA.IG',
      42,
      5.0,
      BASE,
      'USD-OIS',
      'USD'
    ),
  student_t: () => calibration.calibrationStepStudentT('T', 'TRANCHE-1', 'CDX.NA.IG_CORR'),
  hull_white: () =>
    calibration.calibrationStepHullWhite('HW', 'USD-OIS', 'USD', BASE, undefined, {
      fit_tolerance: 1e-4,
    }),
  cap_floor_hull_white: () =>
    calibration.calibrationStepCapFloorHullWhite(
      'HWCF',
      'USD-OIS',
      'USD-SOFR-3M',
      'USD',
      BASE,
      undefined,
      { fit_tolerance: 1e-4 }
    ),
  svi_surface: () => calibration.calibrationStepSviSurface('AAPL-SVI', BASE, 'AAPL'),
  xccy_basis: () => calibration.calibrationStepXccyBasis('EUR-XCCY', 'EUR', BASE, 1.1, 'USD-OIS'),
  parametric: () => calibration.calibrationStepParametric('USD-NS', BASE),
};

for (const [name, build] of Object.entries(STEPS)) {
  test(`calibrationStep twin builds the Python CalibrationStep.${name.replace('_named', '')} object (${name})`, () => {
    assert.deepEqual(plain(build()), GOLDEN.steps[name]);
  });
}

test('calibrationStep twins reject unknown fields and bad dates', () => {
  assert.throws(
    () =>
      calibration.calibrationStepDiscount('USD-OIS', 'USD', BASE, undefined, undefined, {
        bogus: 1,
      }),
    (error) => validation(error) && /invalid discount step/.test(error.message)
  );
  assert.throws(
    () => calibration.calibrationStepDiscount('USD-OIS', 'USD', '08/05/2026'),
    validation
  );
});

test('a plan built from the step and quote twins calibrates', () => {
  const quotes = [
    calibration.rateQuoteDeposit('USD-DEP-3M', 'USD-SOFR-OIS', '3M', 0.052),
    calibration.rateQuoteSwap('USD-SWAP-2Y', 'USD-SOFR-OIS', '2Y', 0.049),
  ];
  const envelope = {
    schema: 'finstack_quant.calibration/1',
    plan: {
      id: 'parity',
      quote_sets: { 'USD-OIS': quotes.map((quote) => quote.id) },
      steps: [calibration.calibrationStepDiscount('USD-OIS', 'USD', BASE)],
    },
    market_data: quotes.map((quote) => ({ kind: 'rate_quote', ...quote })),
  };
  assert.deepEqual(plain(envelope.market_data), GOLDEN.envelope.market_data);
  assert.equal(calibration.calibrate(envelope).result.report.success, true);
});

test('validateCalibration returns the validated envelope object', () => {
  const validated = calibration.validateCalibration(GOLDEN.envelope);
  assert.equal(validated.plan.id, 'parity');
  assert.deepEqual(
    validated.plan.steps.map((step) => step.id),
    ['USD-OIS']
  );
  assert.equal(calibration.validateCalibration(JSON.stringify(GOLDEN.envelope)).plan.id, 'parity');
  assert.deepEqual(calibration.dryRun(GOLDEN.envelope).errors, []);
  const broken = structuredClone(GOLDEN.envelope);
  broken.plan.steps[0].quote_set = 'missing';
  assert.throws(
    () => calibration.validateCalibration(broken),
    (error) => error.name === 'CalibrationEnvelopeError' && error.kind === 'undefined_quote_set'
  );
});

test('calibrationResultStepReport, its JSON twin and residuals read one step', () => {
  const result = calibration.calibrate(GOLDEN.envelope);
  const expected = GOLDEN.step_report;
  const report = calibration.calibrationResultStepReport(result, 'USD-OIS');
  assert.equal(report.success, expected.success);
  assert.equal(report.iterations, expected.iterations);
  assert.deepEqual(Object.keys(report.residuals).sort(), expected.residual_ids);
  assert.equal(
    JSON.parse(calibration.calibrationResultStepReportJson(result, 'USD-OIS')).iterations,
    expected.iterations
  );
  assert.equal(
    calibration.calibrationResultStepReport(JSON.stringify(result), 'USD-OIS').iterations,
    expected.iterations
  );
  const rows = calibration.calibrationResultResiduals(result, 'USD-OIS');
  assert.deepEqual(
    rows.map((row) => row.quote_label),
    expected.residual_ids
  );
  for (const row of rows) {
    assert.ok(Math.abs(row.residual) < 1e-8, `${row.quote_label}: ${row.residual}`);
    assert.ok(Number.isNaN(row.target_value));
  }
  for (const read of [
    calibration.calibrationResultStepReport,
    calibration.calibrationResultStepReportJson,
    calibration.calibrationResultResiduals,
  ]) {
    assert.throws(
      () => read(result, 'nope'),
      (error) =>
        error.kind === 'not_found' &&
        error.message.includes(`calibration step 'nope'; available steps: ["USD-OIS"]`)
    );
  }
  assert.throws(
    () => calibration.calibrationResultStepReport('{ not json', 'USD-OIS'),
    (error) => error.name === 'CalibrationEnvelopeError'
  );
});

test('bootstrapHullWhiteSigmaScheduleToCapFloors matches the Python bootstrap', () => {
  const curve = discountCurve();
  const [params, report] = calibration.bootstrapHullWhiteSigmaScheduleToCapFloors(
    curve,
    HW.cap_floor_quotes,
    HW.piecewise_config
  );
  const expected = HW.piecewise;
  assert.equal(report.success, expected.success);
  assert.equal(params.kappa, expected.params.kappa);
  assert.deepEqual(Array.from(params.volatility.times), expected.params.volatility.times);
  params.volatility.values.forEach((value, index) =>
    close(value, expected.params.volatility.values[index], 1e-8, `sigma[${index}]`)
  );
  const [explicit] = calibration.bootstrapHullWhiteSigmaScheduleToCapFloors(
    curve,
    JSON.stringify(HW.cap_floor_quotes),
    JSON.stringify(HW.piecewise_config),
    curve
  );
  assert.deepEqual(plain(explicit), plain(params));
  assert.throws(
    () =>
      calibration.bootstrapHullWhiteSigmaScheduleToCapFloors(curve, HW.cap_floor_quotes, {
        ...HW.piecewise_config,
        frequency: 'monthly',
      }),
    validation
  );
});

test('hullWhiteParamsSigmaAt reads the piecewise sigma like Python sigma_at', () => {
  const expected = HW.piecewise;
  assert.equal(calibration.hullWhiteParamsSigmaAt(expected.params, 1.5), expected.sigma_at_1_5);
  assert.equal(
    calibration.hullWhiteParamsSigmaAt(JSON.stringify(expected.params), 0.5),
    expected.sigma_at_0_5
  );
  assert.throws(() => calibration.hullWhiteParamsSigmaAt({ kappa: 0.03 }, 1.0), validation);
});

test('calibrateHullWhiteToCapFloors matches the Python fixed-kappa fit', () => {
  const curve = discountCurve();
  const quotes = HW.cap_floor_quotes.slice(1, 2);
  const [params, report] = calibration.calibrateHullWhiteToCapFloors(
    curve,
    quotes,
    HW.scalar_config
  );
  assert.equal(report.success, HW.scalar.success);
  assert.equal(params.kappa, HW.scalar.params.kappa);
  close(params.sigma, HW.scalar.params.sigma, 1e-8, 'sigma');
  const [explicit] = calibration.calibrateHullWhiteToCapFloors(
    curve,
    quotes,
    HW.scalar_config,
    curve
  );
  assert.deepEqual(plain(explicit), plain(params));
  assert.throws(
    () => calibration.calibrateHullWhiteToCapFloors(curve, quotes, { fit_tolerance: 1e-6 }),
    validation
  );
  assert.throws(
    () =>
      calibration.calibrateHullWhiteToCapFloors(curve, quotes, {
        ...HW.scalar_config,
        bogus: 1,
      }),
    validation
  );
});

test('calibrateHullWhiteToSwaptions matches the Python swaption fit', () => {
  const curve = discountCurve();
  const [params, report] = calibration.calibrateHullWhiteToSwaptions(
    curve,
    HW.swaption_quotes,
    HW.swaption_fit_tolerance
  );
  assert.equal(report.success, HW.swaptions.success);
  close(params.kappa, HW.swaptions.params.kappa, 1e-6, 'kappa');
  close(params.sigma, HW.swaptions.params.sigma, 1e-6, 'sigma');
  const [explicit] = calibration.calibrateHullWhiteToSwaptions(
    curve,
    HW.swaption_quotes,
    HW.swaption_fit_tolerance,
    'semi_annual',
    null
  );
  assert.deepEqual(plain(explicit), plain(params));
  assert.throws(
    () =>
      calibration.calibrateHullWhiteToSwaptions(
        curve,
        HW.swaption_quotes,
        HW.swaption_fit_tolerance,
        'monthly'
      ),
    (error) => validation(error) && /invalid swap frequency 'monthly'/.test(error.message)
  );
  assert.throws(
    () =>
      calibration.calibrateHullWhiteToSwaptions(
        curve,
        HW.swaption_quotes.slice(0, 1),
        HW.swaption_fit_tolerance
      ),
    validation
  );
});

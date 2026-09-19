/** Valuation audit contracts shared with Python and exercised through the facade. */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { core, valuations } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

test('structured credit recovery overrides match canonical model terms', () => {
  const fixture = JSON.parse(
    readFileSync(
      new URL(
        '../../../finstack-quant/valuations/tests/fixtures/production_structured_credit.json',
        import.meta.url
      )
    )
  );
  const value = () =>
    valuations.instruments.priceInstrument(
      JSON.stringify(fixture.instrument),
      JSON.stringify(fixture.market),
      fixture.as_of,
      'discounting',
      []
    ).value.amount;
  const expected = value();
  const spec = fixture.instrument.instrument.spec;
  spec.recovery_spec = { rate: 0.1, recovery_lag: 0 };
  spec.behavior_overrides.recovery_rate = 0.7;
  spec.behavior_overrides.recovery_lag_months = 9;
  assert.ok(Math.abs(value() - expected) < 1e-6);
});

test('convertible clean exercise pays accrued interest exactly once', () => {
  const fixture = JSON.parse(
    readFileSync(
      new URL(
        '../../../finstack-quant/valuations/tests/fixtures/production_convertible.json',
        import.meta.url
      )
    )
  );
  const result = valuations.instruments.priceInstrument(
    JSON.stringify(fixture.instrument),
    JSON.stringify(fixture.market),
    fixture.as_of,
    'tree',
    []
  );
  assert.ok(Math.abs(result.value.amount - (1000 + (50 * 90) / 365)) < 1e-8);
});

test('convertible cross gamma follows scalar and override volatility', () => {
  const fixture = JSON.parse(
    readFileSync(
      new URL(
        '../../../finstack-quant/valuations/tests/fixtures/production_convertible.json',
        import.meta.url
      )
    )
  );
  const spec = fixture.instrument.instrument.spec;
  spec.call_put = null;
  spec.fixed_coupon = null;
  spec.conversion.ratio = 10;
  const price = (spot, vol, metrics = []) => {
    fixture.market.prices.AAPL.unitless = spot;
    fixture.market.prices['AAPL-VOL'].unitless = vol;
    return valuations.instruments.priceInstrument(
      JSON.stringify(fixture.instrument),
      JSON.stringify(fixture.market),
      fixture.as_of,
      'tree',
      metrics
    );
  };
  const expected =
    (Number(price(90.9, 0.41).value.amount) -
      Number(price(90.9, 0.39).value.amount) -
      Number(price(89.1, 0.41).value.amount) +
      Number(price(89.1, 0.39).value.amount)) /
    4;
  assert.ok(Math.abs(expected) > 1e-8);
  assert.ok(
    Math.abs(price(90, 0.4, ['cross_gamma_spot_vol']).measures.cross_gamma_spot_vol - expected) <
      1e-8
  );
  spec.instrument_pricing_overrides = { market_quotes: { implied_volatility: 0.4 } };
  assert.ok(
    Math.abs(price(90, 0.1, ['cross_gamma_spot_vol']).measures.cross_gamma_spot_vol - expected) <
      1e-8
  );
});

test('active volatility overrides separate scalar Greeks from source-node vega', () => {
  const fixture = JSON.parse(
    readFileSync(
      new URL(
        '../../../finstack-quant/valuations/tests/fixtures/production_equity.json',
        import.meta.url
      )
    )
  );
  const spec = fixture.instrument.instrument.spec;
  spec.exercise_style = 'european';
  spec.strike = 120;
  const price = () =>
    valuations.instruments.priceInstrument(
      JSON.stringify(fixture.instrument),
      JSON.stringify(fixture.market),
      fixture.as_of,
      'black76',
      ['vega', 'vanna', 'volga', 'bucketed_vega']
    );
  const reference = price();
  spec.instrument_pricing_overrides = { market_quotes: { implied_volatility: 0.2 } };
  for (const removeSurface of [false, true]) {
    if (removeSurface) fixture.market.surfaces = [];
    const result = price();
    for (const metric of ['vega', 'vanna', 'volga']) {
      assert.ok(Math.abs(reference.measures[metric]) > 1e-8);
      assert.ok(Math.abs(result.measures[metric] - reference.measures[metric]) < 1e-10, metric);
    }
    assert.equal(result.measures.bucketed_vega, 0);
    assert.equal(result.measures.bucketed_vega_residual, result.measures.vega);
  }
});

test('Hull-White exercise retains partial coupons and floating spread', () => {
  const fixture = JSON.parse(
    readFileSync(
      new URL(
        '../../../finstack-quant/valuations/tests/fixtures/production_hw.json',
        import.meta.url
      )
    )
  );
  const result = valuations.instruments.priceInstrument(
    JSON.stringify(fixture.instrument),
    JSON.stringify(fixture.market),
    fixture.as_of,
    'hull_white_1f',
    []
  );
  const modelTime = (start, end) => (Date.parse(end) - Date.parse(start)) / 86400000 / 365;
  let start = '2026-04-01';
  let expected = 0;
  for (const end of ['2027-01-01', '2028-01-01']) {
    const tau = modelTime(start, end);
    expected +=
      (Math.expm1(0.04 * tau) + (0.01 - 0.02) * tau) *
      Math.exp(-0.04 * modelTime(fixture.as_of, end));
    start = end;
  }
  assert.ok(Math.abs(Number(result.value.amount) - expected * 10000000) < 0.01);
});

test('FX global source precedes the pinned quote in both orientations', () => {
  const fx = new core.FxMatrix();
  fx.setQuote('USD', 'EUR', 0.8);
  fx.setQuoteOn('EUR', 'USD', '2025-01-02', core.FxConversionPolicy.cashflowDate(), 1.3);
  assert.equal(fx.rateDefault('EUR', 'USD', '2025-01-02').rate, 1.25);
  assert.equal(fx.rateDefault('USD', 'EUR', '2025-01-02').rate, 0.8);
  fx.setQuote('USD', 'GBP', 0.8);
  assert.equal(fx.rateDefault('EUR', 'GBP', '2025-01-02').rate, 1);
});

test('zero-rate bond DV01 permits negative-rate stress', () => {
  const fixture = JSON.parse(
    readFileSync(
      new URL(
        '../../../finstack-quant/valuations/tests/fixtures/production_icma.json',
        import.meta.url
      )
    )
  );
  const spec = fixture.instrument.instrument.spec;
  spec.issue_date = '2025-01-02';
  spec.maturity = '2026-01-02';
  spec.instrument_pricing_overrides = {};
  spec.cashflow_spec.fixed.rate = '0';
  spec.cashflow_spec.fixed.day_count = 'act_365f';
  spec.cashflow_spec.fixed.end_of_month = false;
  fixture.market.curves[0].base = spec.issue_date;
  const result = valuations.instruments.priceInstrument(
    JSON.stringify(fixture.instrument),
    JSON.stringify(fixture.market),
    spec.issue_date,
    'discounting',
    ['dv01']
  );
  assert.ok(Math.abs(result.measures.dv01 + 100 * Math.sinh(0.0001)) < 1e-9);
});

test('ICMA mid-coupon duration and convexity retain reference periods', () => {
  const fixture = JSON.parse(
    readFileSync(
      new URL(
        '../../../finstack-quant/valuations/tests/fixtures/production_icma.json',
        import.meta.url
      )
    )
  );
  const result = valuations.instruments.priceInstrument(
    JSON.stringify(fixture.instrument),
    JSON.stringify(fixture.market),
    '2024-05-31',
    'discounting',
    ['ytm', 'duration_mac', 'convexity', 'z_spread']
  );
  const base = 1 + result.measures.ytm / 2;
  const first = 2 / base ** 0.5;
  const last = 102 / base ** 1.5;
  assert.ok(Math.abs(result.measures.duration_mac - (0.25 * first + 0.75 * last) / 101) < 1e-9);
  assert.ok(
    Math.abs(
      result.measures.convexity - (0.25 * 0.75 * first + 0.75 * 1.25 * last) / base ** 2 / 10100
    ) < 1e-10
  );
  assert.ok(result.measures.z_spread > 0);
});
const fixtures = JSON.parse(
  readFileSync(
    new URL(
      '../../../finstack-quant/valuations/tests/fixtures/valuation_binding_regressions.json',
      import.meta.url
    )
  )
);
const price = (instrument, market, model, metrics = []) =>
  valuations.instruments.priceInstrument(
    JSON.stringify(instrument),
    JSON.stringify(market),
    '2025-01-02',
    model,
    metrics
  );

for (const model of ['black76', 'static_replication']) {
  for (const missing of ['curves', 'surfaces']) {
    test(`${model}: missing ${missing} preserves not_found`, () => {
      const market = structuredClone(fixtures.market);
      market[missing] = [];
      assert.throws(
        () => price(fixtures.cms, market, model),
        (error) => {
          assert.equal(error.kind, 'not_found');
          assert.match(error.message, /USD-/);
          return true;
        }
      );
    });
  }
  test(`${model}: invalid CMS contract preserves validation`, () => {
    const instrument = structuredClone(fixtures.cms);
    instrument.instrument.spec.accrual_fractions = [];
    assert.throws(
      () => price(instrument, fixtures.market, model),
      (error) => {
        assert.equal(error.kind, 'validation');
        return true;
      }
    );
  });
}

test('metric fixing requirement preserves validation', () => {
  const instrument = structuredClone(fixtures.cms);
  Object.assign(instrument.instrument.spec, {
    fixing_dates: ['2025-01-03'],
    payment_dates: ['2025-04-03'],
    metric_pricing_overrides: { theta_period: '2D' },
  });
  assert.throws(
    () => price(instrument, fixtures.market, 'static_replication', ['theta']),
    (error) => {
      assert.equal(error.kind, 'validation');
      assert.match(error.message, /fixing/i);
      return true;
    }
  );
});

test('commodity MC diagnostics survive serialization and position scaling', () => {
  const instrument = structuredClone(fixtures.commodity);
  const run = () => price(instrument, fixtures.market, 'monte_carlo_schwartz_smith');
  const base = run();
  const replay = run();
  assert.deepEqual(base.value, replay.value);
  assert.deepEqual(base.details, replay.details);
  assert.equal(base.details.type, 'monte_carlo');
  const details = base.details.data;
  assert.ok(details.standard_error > 0);
  assert.equal(Number(details.estimator_paths), 2000);
  assert.equal(Number(details.simulated_paths), 2000);
  assert.equal(details.seed, 42n);
  assert.equal(details.time_grid.length, 253);
  assert.equal(details.time_grid[0], 0);
  assert.equal(details.antithetic || details.sobol || details.brownian_bridge, false);
  instrument.instrument.spec.quantity *= 10;
  const scaled = run();
  assert.ok(Math.abs(Number(scaled.value.amount) / Number(base.value.amount) - 10) < 1e-10);
  assert.ok(Math.abs(scaled.details.data.standard_error / details.standard_error - 10) < 1e-10);
});

test('metric missing volatility preserves not_found', () => {
  assert.throws(
    () => price(fixtures.commodity, fixtures.market, 'monte_carlo_schwartz_smith', ['vega']),
    (error) => {
      assert.equal(error.kind, 'not_found');
      assert.match(error.message, /WTI-VOL/);
      return true;
    }
  );
});

for (const strike of ['0.00001', '0.02', '0.03', '0.04']) {
  test(`CMS zero-vol replication matches discounted intrinsic at strike ${strike}`, () => {
    const instrument = structuredClone(fixtures.cms);
    instrument.instrument.spec.strike = strike;
    const market = structuredClone(fixtures.market);
    market.surfaces[0].vols_row_major.fill(0);
    const analytic = price(instrument, market, 'black76');
    const replicated = price(instrument, market, 'static_replication');
    assert.ok(Math.abs(Number(analytic.value.amount) - Number(replicated.value.amount)) < 1e-7);
  });
}

test('equity LR prices and implied volatility match independent QuantLib cases', () => {
  const fixture = JSON.parse(
    readFileSync(
      new URL(
        '../../../finstack-quant/valuations/tests/fixtures/production_equity.json',
        import.meta.url
      )
    )
  );
  const oracle = JSON.parse(
    readFileSync(
      new URL(
        '../../../finstack-quant/models/tests/fixtures/production_lr_quantlib.json',
        import.meta.url
      )
    )
  );
  const spec = fixture.instrument.instrument.spec;
  for (const row of oracle.cases.filter((row) => row.style !== 'european')) {
    spec.attributes = { meta: { market_price: String(row.pv) } };
    spec.option_type = row.side;
    spec.exercise_style = row.style;
    spec.exercise_schedule = oracle.exercise_days.map((days) =>
      new Date(Date.parse(fixture.as_of) + days * 86400000).toISOString().slice(0, 10)
    );
    spec.instrument_pricing_overrides = {
      market_quotes: { implied_volatility: row.volatility },
      model_config: { tree_steps: row.steps },
    };
    const result = valuations.instruments.priceInstrument(
      JSON.stringify(fixture.instrument),
      JSON.stringify(fixture.market),
      fixture.as_of,
      'black76',
      ['implied_vol']
    );
    assert.ok(
      Math.abs(Number(result.value.amount) - row.pv) < 1e-9,
      `${row.style}/${row.side}/${row.steps}`
    );
    assert.ok(Math.abs(result.measures.implied_vol - row.volatility) < 1e-7);
  }
});

test('inflation fixed leg compounds annual 1/1 and requires historical CPI', () => {
  const fixture = JSON.parse(
    readFileSync(
      new URL(
        '../../../finstack-quant/valuations/tests/fixtures/production_inflation.json',
        import.meta.url
      )
    )
  );
  const price = () =>
    valuations.instruments.priceInstrument(
      JSON.stringify(fixture.instrument),
      JSON.stringify(fixture.market),
      fixture.as_of,
      'discounting',
      []
    );
  const expected = 1_000_000 * (0.1 - (1.02 ** 5 - 1));
  assert.ok(Math.abs(Number(price().value.amount) - expected) < 1e-7);
  assert.equal(core.DayCount.oneOne().toString(), 'one_one');
  delete fixture.instrument.instrument.spec.base_cpi;
  assert.throws(price, /US-CPI/);
});

test('mortgage settlement, seasoned IO and financing preserve economic units', () => {
  const fixture = JSON.parse(
    readFileSync(
      new URL(
        '../../../finstack-quant/valuations/tests/fixtures/production_mortgage.json',
        import.meta.url
      )
    )
  );
  const price = (name, metrics = []) =>
    valuations.instruments.priceInstrument(
      JSON.stringify(fixture[name]),
      JSON.stringify(fixture.market),
      fixture.as_of,
      'discounting',
      metrics
    );
  assert.ok(Math.abs(Number(price('tba').value.amount) - 1000 * 0.04 * (1 / 12 - 10 / 360)) < 1e-9);
  assert.ok(Math.abs(Number(price('cmo').value.amount) - (60000 * 0.01) / 12) < 1e-9);
  const mbs = price('mbs', ['duration_mod', 'dv01']);
  assert.ok(mbs.measures.duration_mod > 0);
  assert.ok(mbs.measures.dv01 < 0);
  const roll = price('roll', ['implied_financing_rate', 'roll_specialness']);
  assert.ok(
    Math.abs(roll.measures.roll_specialness + roll.measures.implied_financing_rate * 10000) < 1e-9
  );
});

test('TBA delivery excludes prior receivables and adjusts the agency payment date', () => {
  const fixture = JSON.parse(
    readFileSync(
      new URL(
        '../../../finstack-quant/valuations/tests/fixtures/production_mortgage.json',
        import.meta.url
      )
    )
  );
  fixture.tba.instrument.spec.assumed_pool.issue_date = '2025-01-01';
  fixture.market.curves[0].knot_points = [
    [0, 1],
    [40, Math.exp(-0.04 * 40)],
  ];
  const result = valuations.instruments.priceInstrument(
    JSON.stringify(fixture.tba),
    JSON.stringify(fixture.market),
    fixture.as_of,
    'discounting',
    []
  );
  const expected =
    (1000 + (1000 * 0.04) / 12) * Math.exp((-0.04 * 57) / 365) -
    (1000 + (1000 * 0.04 * 10) / 360) * Math.exp((-0.04 * 10) / 365);
  assert.ok(Math.abs(Number(result.value.amount) - expected) < 1e-9);
});

test('large homogeneous credit-pool loss meets the independent LHP reference', () => {
  const f = JSON.parse(
    readFileSync(
      new URL(
        '../../../finstack-quant/valuations/tests/fixtures/production_credit_tranche.json',
        import.meta.url
      )
    )
  );
  const reference = JSON.parse(
    readFileSync(
      new URL(
        '../../../finstack-quant/valuations/tests/fixtures/production_tranche_loss_reference.json',
        import.meta.url
      )
    )
  );
  const result = valuations.instruments.priceInstrument(
    JSON.stringify(f.instrument),
    JSON.stringify(f.market),
    f.as_of,
    'hazard_rate',
    ['expected_loss']
  );
  // Index-sized homogeneous pools use the exact conditional binomial, so the
  // independent SciPy binomial-CDF case matching this fixture's pool is the
  // pin. Mirrors the Python twin in tests/test_production_audit.py.
  const expectedCase = reference.cases.find(
    (c) => c.n === 125 && c.pd === 0.01 && c.correlation === 0.3 && c.cap === 0.03
  );
  assert.ok(expectedCase, 'reference fixture must carry the n=125 pd=0.01 rho=0.3 cap=0.03 case');
  assert.ok(
    Math.abs(result.measures.expected_loss - 1e6 * expectedCase.expected_loss) < 2e-4,
    `expected_loss ${result.measures.expected_loss} vs reference ${1e6 * expectedCase.expected_loss}`
  );
});

test('credit-index snapshots require complete issuer coverage', () => {
  const f = JSON.parse(
    readFileSync(
      new URL(
        '../../../finstack-quant/valuations/tests/fixtures/production_credit_tranche.json',
        import.meta.url
      )
    )
  );
  f.market.credit_indices[0].num_constituents = 2;
  f.market.credit_indices[0].issuer_credit_curve_ids = { A: 'HZ' };
  assert.throws(
    () =>
      valuations.instruments.priceInstrument(
        JSON.stringify(f.instrument),
        JSON.stringify(f.market),
        f.as_of,
        'hazard_rate',
        []
      ),
    /complete coverage|num_constituents/
  );
});

test('CDS option premium settlement does not change option variance', () => {
  const f = JSON.parse(
    readFileSync(
      new URL(
        '../../../finstack-quant/valuations/tests/fixtures/production_cds_option.json',
        import.meta.url
      ),
      'utf8'
    )
  );
  const values = ['2025-01-06', '2025-07-01'].map((settlement) => {
    f.instrument.instrument.spec.cash_settlement_date = settlement;
    return valuations.instruments.priceInstrument(
      JSON.stringify(f.instrument),
      JSON.stringify(f.market),
      f.as_of,
      'bloomberg_cdso',
      []
    ).value.amount;
  });
  assert.ok(Math.abs(values[0] - values[1]) < 1e-8, `${values}`);
});

test('CDS bespoke frequency and stub determine premium cashflow PV', () => {
  const f = JSON.parse(
    readFileSync(
      new URL(
        '../../../finstack-quant/valuations/tests/fixtures/production_cds_premium.json',
        import.meta.url
      ),
      'utf8'
    )
  );
  const start = Date.parse('2025-01-01');
  for (const [stub, dates] of [
    ['short_front', ['2025-04-01', '2025-10-01', '2026-04-01']],
    ['long_front', ['2025-10-01', '2026-04-01']],
  ]) {
    f.instrument.instrument.spec.premium.stub = stub;
    const actual = valuations.instruments.priceInstrument(
      JSON.stringify(f.instrument),
      JSON.stringify(f.market),
      f.as_of,
      'hazard_rate',
      []
    ).value.amount;
    let previous = start;
    let expected = 0;
    for (const date of dates) {
      const payment = Date.parse(date);
      expected -=
        ((10 * (payment - previous)) / 86400000 / 360) *
        Math.exp((-0.03 * (payment - start)) / 86400000 / 365);
      previous = payment;
    }
    assert.ok(Math.abs(actual - expected) < 1e-10, `${stub}: ${actual} vs ${expected}`);
  }
});

test('CDS option exercise payment cannot precede legal expiry', () => {
  const f = JSON.parse(
    readFileSync(
      new URL(
        '../../../finstack-quant/valuations/tests/fixtures/production_cds_option.json',
        import.meta.url
      ),
      'utf8'
    )
  );
  f.instrument.instrument.spec.exercise_settlement_date = '2025-12-31';
  assert.throws(
    () =>
      valuations.instruments.priceInstrument(
        JSON.stringify(f.instrument),
        JSON.stringify(f.market),
        f.as_of,
        'bloomberg_cdso',
        []
      ),
    /exercise_settlement_date.*expiry/
  );
});

test('representative structured collateral preserves PV and rejects duplicates', () => {
  const fixture = JSON.parse(
    readFileSync(
      new URL(
        '../../../finstack-quant/valuations/tests/fixtures/production_structured_credit.json',
        import.meta.url
      )
    )
  );
  const pool = fixture.instrument.instrument.spec.pool;
  const value = () =>
    valuations.instruments.priceInstrument(
      JSON.stringify(fixture.instrument),
      JSON.stringify(fixture.market),
      fixture.as_of,
      'discounting',
      []
    ).value.amount;
  const expected = value();
  const asset = pool.assets.pop();
  pool.rep_lines = [
    {
      id: 'REP',
      asset_type: asset.asset_type,
      balance: asset.balance,
      rate: asset.rate,
      spread_bp: asset.spread_bp,
      index_id: asset.index_id,
      maturity: asset.maturity,
      seasoning_months: 0,
      day_count: asset.day_count,
      cpr: null,
      cdr: null,
      recovery_rate: null,
    },
  ];
  assert.ok(Math.abs(value() - expected) < 1e-6);
  pool.assets.push(asset);
  assert.throws(value, /exactly one of assets, representative lines or instruments/);
});

for (const [endDate, periodDays] of [
  ['2024-04-02', [91]],
  ['2025-01-02', [91, 91, 92, 92]],
]) {
  for (const [maxPrice, minYield] of [
    [99, 0],
    [100, 0.5],
  ]) {
    test(`ineligible reinvestment retains capital through ${endDate}: max price ${maxPrice}, minimum current yield ${minYield}`, () => {
      const fixture = JSON.parse(
        readFileSync(
          new URL(
            '../../../finstack-quant/valuations/tests/fixtures/production_structured_credit.json',
            import.meta.url
          )
        )
      );
      const spec = fixture.instrument.instrument.spec;
      fixture.as_of = spec.closing_date = '2024-01-02';
      spec.first_payment_date = '2024-04-02';
      spec.maturity = spec.pool.assets[0].maturity = endDate;
      spec.prepayment_spec = { cpr: 0.36, curve: null };
      spec.default_spec = { cdr: 0, curve: null };
      Object.assign(spec.tranches.tranches[0], {
        seniority: 'equity',
        coupon: { fixed: { rate: 0 } },
        maturity: endDate,
      });
      spec.pool.reinvestment_period = {
        end_date: endDate,
        is_active: true,
        criteria: {
          max_price: maxPrice,
          min_yield: minYield,
        },
      };
      fixture.market.curves[0].base = fixture.as_of;
      fixture.market.curves[0].knot_points = [
        [0, 1],
        [20, 1],
      ];
      const result = valuations.instruments.priceInstrument(
        JSON.stringify(fixture.instrument),
        JSON.stringify(fixture.market),
        fixture.as_of,
        'discounting',
        []
      );
      const q = 0.64 ** 0.25;
      const expected =
        100_000_000 *
        (1 + 0.07 * periodDays.reduce((sum, days, i) => sum + (q ** i * days) / 360, 0));
      assert.ok(Math.abs(result.value.amount - expected) < 1e-6);
    });
  }
}

test('predefaulted collateral pays its outstanding recovery once', () => {
  const fixture = JSON.parse(
    readFileSync(
      new URL(
        '../../../finstack-quant/valuations/tests/fixtures/production_structured_credit.json',
        import.meta.url
      )
    )
  );
  Object.assign(fixture.instrument.instrument.spec.pool.assets[0], {
    is_defaulted: true,
    default_date: fixture.as_of,
    recovery_amount: { amount: '70000000', currency: 'USD' },
  });
  fixture.market.curves[0].knot_points = [
    [0, 1],
    [20, 1],
  ];
  const result = valuations.instruments.priceInstrument(
    JSON.stringify(fixture.instrument),
    JSON.stringify(fixture.market),
    fixture.as_of,
    'discounting',
    []
  );
  assert.ok(Math.abs(result.value.amount - 70_000_000) < 1e-6);
});

test('credit enhancement uses current debt, collateral and funded reserves', () => {
  const fixture = JSON.parse(
    readFileSync(
      new URL(
        '../../../finstack-quant/valuations/tests/fixtures/production_structured_credit.json',
        import.meta.url
      )
    )
  );
  const spec = fixture.instrument.instrument.spec;
  spec.tranches.tranches[0].current_balance.amount = '80000000';
  spec.pool.reserve_account.amount = '20000000';
  const result = valuations.instruments.priceInstrument(
    JSON.stringify(fixture.instrument),
    JSON.stringify(fixture.market),
    fixture.as_of,
    'discounting',
    ['abs_ce_level']
  );
  assert.ok(Math.abs(result.measures.abs_ce_level - 100 / 3) < 1e-12);
});

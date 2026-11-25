import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { core, valuations } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

const { instruments } = valuations;

test('B5: quoteCompounding accepts structured wire forms across Bond, TermLoan, and RevolvingCredit', () => {
  const zc = instruments.Bond.zeroCoupon(
    'ZC-WIRE',
    new core.Money(1_000_000, core.Currency.usd()),
    '2025-01-15',
    '2030-01-15',
    { periodic: 2 },
    'USD-OIS'
  );
  assert.deepEqual(JSON.parse(zc.toJson()).instrument.spec.quote_compounding, { periodic: 2 });

  const tl = instruments.TermLoan.example()
    .toBuilder()
    .quoteCompounding({ continuous: null })
    .build();
  assert.equal(tl.id, 'TERM-LOAN-USD-5Y');

  const rc = instruments.RevolvingCredit.example()
    .toBuilder()
    .quoteCompounding({ periodic: 4 })
    .build();
  assert.equal(rc.id, 'RCF-USD-3Y');
  zc.free();
  tl.free();
  rc.free();
});

test('H25: RevolvingCredit optional setters accept null without panicking', () => {
  const rc = instruments.RevolvingCredit.example()
    .toBuilder()
    .oidEir(null)
    .lc(null)
    .creditCurveId(null)
    .calendarId(null)
    .attributes(null)
    .build();
  assert.equal(rc.creditCurveId, null);
  assert.equal(rc.calendarId, null);
  rc.free();
});

test('H26: AgencyCmo.dealName and AgencyMbsPassthrough.poolId use plain strings', () => {
  const cmo = instruments.AgencyCmo.example().toBuilder().dealName('FNMA-TEST').build();
  assert.equal(cmo.dealName, 'FNMA-TEST');

  const mbs = instruments.AgencyMbsPassthrough.example().toBuilder().poolId('POOL-TEST').build();
  assert.equal(mbs.poolId, 'POOL-TEST');
  cmo.free();
  mbs.free();
});

test('H28: bondBreakevenLadderTable returns numeric columns via to_js_table_numeric', () => {
  const bond = instruments.Bond.example();
  const spec = JSON.parse(bond.toJson()).instrument.spec;
  const market = new core.MarketContext();
  const curve = new core.DiscountCurve({
    id: spec.discount_curve_id,
    baseDate: spec.issue_date,
    knots: [0.0, 1.0, 10.0, 0.7],
    interp: 'log_linear',
    extrapolation: 'extend',
    dayCount: 'act_365f',
  });
  try {
    market.insert(curve);
    const ladder = instruments.bondBreakevenLadder(bond, market, spec.issue_date, ['3M', '1Y'], {
      target: 'z_spread',
      mode: 'linear',
    });
    const table = instruments.bondBreakevenLadderTable(ladder);
    assert.equal(table.row_count, 2);
    const carryCol = table.columns.find((col) => col.name === 'carry');
    assert.ok(carryCol, 'carry column present');
    assert.ok(Array.isArray(carryCol.data.values));
    assert.equal(typeof carryCol.data.values[0], 'number');
  } finally {
    curve.free();
    market.free();
    bond.free();
  }
});

test('H29: TermLoanBuilder.currency validates ISO-4217 codes case-insensitively', () => {
  const loanLower = instruments.TermLoan.example().toBuilder().currency('usd').build();
  assert.equal(loanLower.currency, 'USD');
  assert.throws(() => instruments.TermLoan.example().toBuilder().currency('NOPE'), {
    kind: 'validation',
  });
  loanLower.free();
});

test('H30: BondFuture and InterestRateFuture expose full contract getters', () => {
  const bf = instruments.BondFuture.example();
  assert.equal(typeof bf.deliveryStart, 'string');
  assert.equal(typeof bf.contractSpecs, 'object');
  assert.ok(Array.isArray(bf.deliverableBasket) && bf.deliverableBasket.length > 0);
  assert.ok(bf.ctdBondId === null || typeof bf.ctdBondId === 'string');
  assert.ok(bf.ctdBond === null || typeof bf.ctdBond === 'object');
  assert.equal(typeof bf.discountCurveId, 'string');
  assert.ok(bf.repoCurveId === null || typeof bf.repoCurveId === 'string');

  const irf = instruments.InterestRateFuture.example();
  assert.ok(irf.fixingDate === null || typeof irf.fixingDate === 'string');
  assert.ok(irf.periodStart === null || typeof irf.periodStart === 'string');
  assert.ok(irf.periodEnd === null || typeof irf.periodEnd === 'string');
  assert.equal(typeof irf.dayCount, 'object');
  assert.equal(typeof irf.dayCount.toString(), 'string');
  assert.equal(typeof irf.contractSpecs, 'object');
  assert.equal(typeof irf.discountCurveId, 'string');
  assert.equal(typeof irf.forwardCurveId, 'string');
  assert.notEqual(irf.compounding, undefined);
  assert.ok(irf.indexId === null || typeof irf.indexId === 'string');
  assert.ok(irf.fixingCalendarId === null || typeof irf.fixingCalendarId === 'string');
  assert.ok(irf.volSurfaceId === null || typeof irf.volSurfaceId === 'string');
  bf.free();
  irf.free();
});

test('M1-A: core instruments support toBuilder() id round-trip', () => {
  const cases = [
    [instruments.Bond, 'BOND-EDIT'],
    [instruments.TermLoan, 'TL-EDIT'],
    [instruments.RevolvingCredit, 'RC-EDIT'],
    [instruments.ConvertibleBond, 'CB-EDIT'],
    [instruments.InterestRateSwap, 'IRS-EDIT'],
    [instruments.Swaption, 'SW-EDIT'],
    [instruments.CapFloor, 'CF-EDIT'],
  ];
  for (const [Cls, nextId] of cases) {
    const inst = Cls.example();
    const rebuilt = inst.toBuilder().id(nextId).build();
    assert.equal(rebuilt.id, nextId);
    inst.free();
    rebuilt.free();
  }
});

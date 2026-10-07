/**
 * `priceInstrument` stamps `provenance` on every result and, with `explain`,
 * attaches a per-cashflow trace that sums to the price.
 */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { core, valuations } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

function golden(name) {
  return JSON.parse(
    readFileSync(
      new URL(
        `../../../finstack-quant/valuations/tests/golden/data/pricing/quantlib/bond/${name}.json`,
        import.meta.url
      )
    )
  );
}

const { priceInstrument, priceInstrumentWithMarket } = valuations.instruments;

test('every priced result records how it was produced', () => {
  const g = golden('usd_fixed_10y_risk_free_quantlib');
  const asOf = g.metadata.valuation_date;
  const result = priceInstrument(g.instrument, g.market.data, asOf, 'discounting');

  assert.equal(result.explanation, undefined);
  assert.equal(result.provenance.model, 'discounting');
  assert.equal(result.provenance.requested_as_of, asOf);
  assert.ok(result.provenance.market_dependencies.curves.discount_curves.length > 0);
  assert.equal(result.provenance.sensitivity_bumps, undefined);

  const withRisk = priceInstrument(g.instrument, g.market.data, asOf, 'discounting', ['dv01'], {
    bump_config: { rate_bump_bp: 5 },
  });
  assert.equal(withRisk.provenance.sensitivity_bumps.rate_bump_bp, 5);
  assert.equal(withRisk.provenance.sensitivity_bumps.vol_bump_decimal, 0.01);
});

test('explain attaches a cashflow trace that sums to the price', () => {
  for (const [name, model] of [
    ['usd_fixed_10y_risk_free_quantlib', 'discounting'],
    ['usd_fixed_5y_hazard_quantlib', 'hazard_rate'],
  ]) {
    const g = golden(name);
    const asOf = g.metadata.valuation_date;
    const result = priceInstrument(
      g.instrument,
      g.market.data,
      asOf,
      model,
      null,
      null,
      null,
      true
    );
    const { type, entries } = result.explanation;

    assert.equal(type, 'pricing');
    assert.ok(entries.length > 0);
    assert.ok(entries.every((entry) => entry.kind === 'cashflow_pv'));
    assert.equal(
      entries.every((entry) => typeof entry.survival_probability === 'number'),
      model === 'hazard_rate'
    );
    const total = entries.reduce((sum, entry) => sum + entry.pv_amount, 0);
    assert.ok(Math.abs(total - result.value.amount) < 0.01, `${name}: ${total}`);

    // The trace is part of the wire format.
    assert.doesNotThrow(() => valuations.valuationResultToJson(result));
  }
});

test('a model without a per-flow decomposition says so in the trace', () => {
  const g = golden('usd_fixed_callable_8y_oas_quantlib');
  const market = core.MarketContext.fromJson(g.market.data);
  const result = priceInstrumentWithMarket(
    g.instrument,
    market,
    g.metadata.valuation_date,
    g.model,
    null,
    null,
    null,
    true
  );
  const { entries } = result.explanation;

  assert.equal(entries.length, 1);
  assert.equal(entries[0].kind, 'computation_step');
  assert.equal(entries[0].name, 'cashflow_trace_unavailable');
});

test('explain rejects a non-boolean', () => {
  const g = golden('usd_fixed_10y_risk_free_quantlib');
  assert.throws(
    () =>
      priceInstrument(
        g.instrument,
        g.market.data,
        g.metadata.valuation_date,
        'discounting',
        null,
        null,
        null,
        'yes'
      ),
    (error) => error instanceof TypeError && error.kind === 'invalid_type'
  );
});

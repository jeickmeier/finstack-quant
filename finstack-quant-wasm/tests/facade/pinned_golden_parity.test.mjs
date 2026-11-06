/**
 * Cross-host goldens: Rust-owned tables both hosts compare to one pinned fixture.
 *
 * The Python twin is `finstack-quant-py/tests/test_pinned_golden_parity.py`:
 * it reads the same fixtures through the same table.
 */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

import { facade, plain } from './statements_parity_support.mjs';

const instruments = facade.valuations.instruments;

// fixture -> [value computed from the pinned fixture, fixture key holding the
// expected value; null when the whole fixture is the expected value]
const GOLDENS = {
  'tranche_metric_field_units.json': [() => instruments.trancheMetricFieldUnits('EUR'), null],
  'tranche_solve_field_units.json': [() => instruments.trancheSolveFieldUnits(), null],
  'bond_quote_set_field_units.json': [() => instruments.bondQuoteSetFieldUnits('EUR'), null],
  'pool_report_field_units.json': [() => instruments.poolReportFieldUnits('EUR'), null],
  'scenario_table_field_units.json': [() => instruments.scenarioTableFieldUnits('EUR'), null],
  'simulation_field_units.json': [() => instruments.simulationFieldUnits('EUR'), null],
  'equity_metrics_field_units.json': [() => instruments.equityMetricsFieldUnits('EUR'), null],
  'price_driver_fields.json': [() => instruments.priceDriverFields(), null],
  'metric_coordinates.json': [
    (pinned) => plain(instruments.metricMetadata(pinned.keys)).map((entry) => entry.coordinates),
    'coordinates',
  ],
};

for (const [fixture, [compute, key]] of Object.entries(GOLDENS)) {
  test(`${fixture} matches the value pinned for both hosts`, () => {
    const pinned = JSON.parse(
      readFileSync(new URL(`../../../finstack-quant-py/tests/fixtures/${fixture}`, import.meta.url))
    );
    assert.deepEqual(plain(compute(pinned)), key === null ? pinned : pinned[key]);
  });
}

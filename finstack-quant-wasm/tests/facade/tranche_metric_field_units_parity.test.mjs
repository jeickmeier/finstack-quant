/**
 * Cross-host golden for `valuations.instruments.trancheMetricFieldUnits`.
 *
 * The Python twin is `finstack-quant-py/tests/test_tranche_metric_field_units_parity.py`:
 * both hosts compare to the same pinned records.
 */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

import { facade, plain } from './statements_parity_support.mjs';

const PINNED = JSON.parse(
  readFileSync(
    new URL(
      '../../../finstack-quant-py/tests/fixtures/tranche_metric_field_units.json',
      import.meta.url
    )
  )
);

test('trancheMetricFieldUnits matches the records pinned for both hosts', () => {
  assert.deepEqual(plain(facade.valuations.instruments.trancheMetricFieldUnits('EUR')), PINNED);
});

const SOLVE_PINNED = JSON.parse(
  readFileSync(
    new URL(
      '../../../finstack-quant-py/tests/fixtures/tranche_solve_field_units.json',
      import.meta.url
    )
  )
);

test('trancheSolveFieldUnits matches the records pinned for both hosts', () => {
  assert.deepEqual(plain(facade.valuations.instruments.trancheSolveFieldUnits()), SOLVE_PINNED);
});

const POOL_REPORT_PINNED = JSON.parse(
  readFileSync(
    new URL(
      '../../../finstack-quant-py/tests/fixtures/pool_report_field_units.json',
      import.meta.url
    )
  )
);

test('poolReportFieldUnits matches the records pinned for both hosts', () => {
  assert.deepEqual(
    plain(facade.valuations.instruments.poolReportFieldUnits('EUR')),
    POOL_REPORT_PINNED
  );
});

const SCENARIO_PINNED = JSON.parse(
  readFileSync(
    new URL(
      '../../../finstack-quant-py/tests/fixtures/scenario_table_field_units.json',
      import.meta.url
    )
  )
);

test('scenarioTableFieldUnits matches the records pinned for both hosts', () => {
  assert.deepEqual(
    plain(facade.valuations.instruments.scenarioTableFieldUnits('EUR')),
    SCENARIO_PINNED
  );
});

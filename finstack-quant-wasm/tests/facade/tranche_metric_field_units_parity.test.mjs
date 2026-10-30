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

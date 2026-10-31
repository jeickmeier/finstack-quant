/**
 * Cross-host golden for `MetricMetadata.coordinates`.
 *
 * The Python twin is `finstack-quant-py/tests/test_metric_coordinates_parity.py`:
 * both hosts compare to the same pinned coordinates.
 */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

import { facade, plain } from './statements_parity_support.mjs';

const PINNED = JSON.parse(
  readFileSync(
    new URL('../../../finstack-quant-py/tests/fixtures/metric_coordinates.json', import.meta.url)
  )
);

test('metricMetadata coordinates match the values pinned for both hosts', () => {
  const metadata = facade.valuations.instruments.metricMetadata(PINNED.keys);
  assert.deepEqual(
    plain(metadata).map((entry) => entry.coordinates),
    PINNED.coordinates
  );
});

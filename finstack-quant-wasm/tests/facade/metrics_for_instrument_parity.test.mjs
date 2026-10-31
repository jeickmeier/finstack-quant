/**
 * Cross-host golden for the ungrouped ids of `metricsForInstrument`.
 *
 * The Python twin is `finstack-quant-py/tests/test_metrics_for_instrument_parity.py`:
 * both hosts compare to the same pinned ids.
 */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

import { facade, plain } from './statements_parity_support.mjs';

const PINNED = JSON.parse(
  readFileSync(
    new URL(
      '../../../finstack-quant-py/tests/fixtures/metrics_for_instrument.json',
      import.meta.url
    )
  )
);

test('metricsForInstrument ungrouped ids match the values pinned for both hosts', () => {
  const instruments = facade.valuations.instruments;
  for (const [type, ids] of Object.entries(PINNED.ids)) {
    assert.deepEqual(plain(instruments.metricsForInstrument(type))[PINNED.group] ?? [], ids, type);
  }
});

test('metricMetadata reports the same group', () => {
  const metadata = facade.valuations.instruments.metricMetadata(['all_in_rate', 'no_such_metric']);
  assert.deepEqual(
    plain(metadata).map((entry) => entry.group ?? null),
    [PINNED.group, null]
  );
});

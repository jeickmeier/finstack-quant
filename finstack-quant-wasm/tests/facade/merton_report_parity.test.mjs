/**
 * Cross-host goldens for `models.credit.mertonReport`.
 *
 * The Python twin is `finstack-quant-py/tests/test_merton_report_parity.py`:
 * both hosts read the same inputs and compare to the same pinned reports.
 */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

import { assertMatches, facade, plain } from './statements_parity_support.mjs';

const { models } = facade;

const fixture = (name) =>
  JSON.parse(
    readFileSync(new URL(`../../../finstack-quant-py/tests/fixtures/${name}`, import.meta.url))
  );
const INPUTS = fixture('merton_report_parity_inputs.json');
const EXPECTED = fixture('merton_report_parity_expected.json');

test('mertonReport returns the reports pinned for both hosts', () => {
  for (const [name, source] of Object.entries(INPUTS.sources)) {
    const report = models.credit.mertonReport({ source, ...INPUTS.terms });
    assertMatches(plain(report), EXPECTED[name], name);
  }
});

test('mertonReport rejects a non-positive horizon and an unknown field', () => {
  const request = { source: INPUTS.sources.model, ...INPUTS.terms };
  assert.throws(() => models.credit.mertonReport({ ...request, horizons: [0] }), {
    kind: 'validation',
  });
  assert.throws(() => models.credit.mertonReport({ ...request, extra: 1 }), {
    kind: 'validation',
  });
});

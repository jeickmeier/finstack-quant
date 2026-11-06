/**
 * Cross-host golden for `scenarios.applicationReportEffectiveAsOf`.
 *
 * The Python twin is
 * `finstack-quant-py/tests/test_application_report_effective_as_of_parity.py`:
 * both hosts read the same pinned reports and compare to the same dates.
 */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

import { facade } from './statements_parity_support.mjs';

const CASES = JSON.parse(
  readFileSync(
    new URL(
      '../../../finstack-quant-py/tests/fixtures/application_report_effective_as_of.json',
      import.meta.url
    )
  )
);

for (const { name, report, as_of: asOf, effective } of CASES) {
  test(`applicationReportEffectiveAsOf matches the date pinned for both hosts: ${name}`, () => {
    assert.equal(facade.scenarios.applicationReportEffectiveAsOf(report, asOf), effective);
  });
}

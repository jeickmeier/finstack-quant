/**
 * The dated LBO evaluator on the WASM facade: the accepted worked example, its
 * refusals and its required-check gate.
 *
 * The model and request are the shared cross-host inputs `dated_lbo_model` and
 * `dated_lbo_request`; the Python twin is
 * `finstack-quant-py/tests/test_statements_dated_lbo.py`. Entry at 8x on 100 of
 * EBITDA is 800; fees of 20 make uses 820; debt proceeds of 500 leave 320 of
 * sponsor cash. Exit at 9x on 120 is 1,080 against 400 of net debt.
 *
 * Requires the wasm-pack web build: npm run build (mise run wasm-build).
 */

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { INPUTS, facade } from './statements_parity_support.mjs';

const { statements_analytics: sa } = facade;
const usd = (amount) => ({ amount: String(amount), currency: 'USD' });

function model(edit) {
  const document = structuredClone(INPUTS.dated_lbo_model);
  edit?.(document);
  return document;
}

function request(edit) {
  const document = structuredClone(INPUTS.dated_lbo_request);
  edit?.(document);
  return document;
}

const commit = (amount) => (document) => {
  document.funding.sponsor_commitment = usd(amount);
};

const amount = (money) => Number(money.amount);

test('evaluateDatedLbo sizes the deal and withholds the funded return', () => {
  const result = sa.evaluateDatedLbo(model(), request());
  assert.equal(result.entry_date, '2025-12-31');
  assert.equal(result.exit_valuation_date, '2028-12-31');
  assert.equal(amount(result.entry_enterprise_value), 800);
  assert.equal(amount(result.funding.cash_uses), 820);
  assert.equal(amount(result.funding.debt_cash_proceeds), 500);
  assert.equal(amount(result.funding.opening_debt_face), 520);
  assert.equal(amount(result.funding.required_sponsor_cash), 320);
  assert.equal(result.funding.funding_gap, null);
  assert.equal(amount(result.exit.enterprise_value), 1080);
  assert.equal(amount(result.exit.net_debt), 400);
  assert.equal(amount(result.exit.sponsor_proceeds), 680);
  assert.equal(result.exit.sized_moic, 2.125);
  assert.equal(result.exit.funded_moic, null);
  assert.equal(result.checks, null);
  assert.equal(result.run.source_snapshot, 'training-borrower-2025-close');
  assert.deepEqual(result.run.observation_cutoff, { through: '2025-12-31' });
});

test('evaluateDatedLbo reports a funding gap, a funded return and a floored exit', () => {
  const short = sa.evaluateDatedLbo(model(), request(commit(300)));
  assert.equal(amount(short.funding.funding_gap), 20);
  assert.equal(short.exit.funded_moic, null);

  const funded = sa.evaluateDatedLbo(model(), request(commit(320)));
  assert.equal(amount(funded.funding.funding_gap), 0);
  assert.equal(funded.exit.funded_moic, 2.125);

  const distressed = sa.evaluateDatedLbo(
    model(),
    request((document) => {
      commit(320)(document);
      document.exit.multiple = '3';
    })
  );
  assert.equal(amount(distressed.exit.common_equity_residual), -40);
  assert.equal(amount(distressed.exit.sponsor_proceeds), 0);
  assert.equal(distressed.exit.funded_moic, 0);
});

test('evaluateDatedLbo takes the model and the request as objects or JSON text', () => {
  const fromObjects = sa.evaluateDatedLbo(model(), request(commit(320)));
  const fromText = sa.evaluateDatedLbo(
    JSON.stringify(model()),
    JSON.stringify(request(commit(320)))
  );
  assert.deepEqual(fromText, fromObjects);
});

test('evaluateDatedLbo refuses a request or model outside the dated contract', () => {
  const refusals = [
    [
      'funding date is not the entry date',
      undefined,
      (document) => {
        document.funding.date = '2025-12-30';
      },
      /matching entry and funding dates before the exit date/,
    ],
    [
      'exit date is not the period end',
      undefined,
      (document) => {
        document.exit_valuation_date = '2028-12-30';
      },
      /must close on the matching actual entry or projected exit date/,
    ],
    [
      'no source snapshot',
      (document) => {
        delete document.source_snapshot;
      },
      undefined,
      /needs an identified source snapshot/,
    ],
    [
      'no availability dates',
      (document) => {
        for (const node of Object.values(document.nodes)) {
          delete node.availability_dates;
        }
      },
      undefined,
      /has no availability date for its 2025 observation/,
    ],
    [
      'zero multiple',
      undefined,
      (document) => {
        document.entry.multiple = '0';
      },
      /multiples must be positive exact decimals/,
    ],
    [
      'unknown request field',
      undefined,
      (document) => {
        document.zzz = 1;
      },
      /unknown field `zzz`/,
    ],
  ];
  for (const [label, editModel, editRequest, message] of refusals) {
    assert.throws(
      () => sa.evaluateDatedLbo(model(editModel), request(editRequest)),
      (error) => error instanceof Error && message.test(error.message),
      label
    );
  }
});

test('evaluateDatedLbo blocks on a required check suite and reports an optional one', () => {
  const checked = (formula, required) =>
    request((document) => {
      commit(320)(document);
      const suite = structuredClone(INPUTS.dated_lbo_check_suite);
      suite.formula_checks[0].formula = formula;
      document.checks = { suite: { kind: 'spec', spec: suite }, required };
    });

  assert.throws(
    () => sa.evaluateDatedLbo(model(), checked('term_amortization > 1000', true)),
    (error) => /Required LBO check suite returned Fail/.test(error.message)
  );
  const reported = sa.evaluateDatedLbo(model(), checked('term_amortization > 1000', false));
  assert.equal(reported.exit.funded_moic, 2.125);
  assert.equal(reported.checks.outcome, 'fail');
  const passed = sa.evaluateDatedLbo(model(), checked('term_amortization >= 0', true));
  assert.equal(passed.checks.outcome, 'pass');
});

test('evaluateDatedLbo runs the LBO model check suite from node mappings', () => {
  const result = sa.evaluateDatedLbo(
    INPUTS.dated_lbo_checked_model,
    INPUTS.dated_lbo_checked_request
  );
  assert.equal(result.checks.outcome, 'pass');
  assert.equal(result.checks.summary.total_checks, 9);
  assert.equal(result.checks.summary.warnings, 5);

  const untagged = request((document) => {
    document.checks = { suite: INPUTS.dated_lbo_check_suite, required: false };
  });
  assert.throws(
    () => sa.evaluateDatedLbo(model(), untagged),
    (error) => /missing field `kind`/.test(error.message)
  );
});

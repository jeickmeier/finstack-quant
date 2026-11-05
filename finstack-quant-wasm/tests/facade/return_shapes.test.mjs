/**
 * Return-shape checks on the published TypeScript declarations.
 *
 * The mirror of `finstack-quant-py/tests/parity/test_return_shapes.py`, in the
 * same order with the same entry names, so a cross-language divergence reads
 * as a one-screen diff. The declarations are wasm-bindgen's
 * `pkg/finstack_quant_wasm.d.ts` (typed by the `#[wasm_bindgen]` attributes in
 * `src/api/**`) plus the members the facade adds in `index.d.ts`. A member is
 * named `function` for a free export and `Class#method` (or
 * `Interface#method`) for a method.
 */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';

const flat = (text) => text.replace(/\s+/g, ' ').trim();

/** Every declared function and method with its declared return type. */
function declaredMembers() {
  const members = new Map();
  const add = (key, ret) => {
    if (!members.has(key)) members.set(key, flat(ret));
  };
  const visit = (sf, statements) => {
    for (const node of statements) {
      if (ts.isModuleDeclaration(node) && node.body && ts.isModuleBlock(node.body))
        visit(sf, node.body.statements);
      if (ts.isFunctionDeclaration(node) && node.name && node.type)
        add(node.name.text, node.type.getText(sf));
      if ((ts.isClassDeclaration(node) || ts.isInterfaceDeclaration(node)) && node.name)
        for (const member of node.members)
          if (
            (ts.isMethodDeclaration(member) || ts.isMethodSignature(member)) &&
            member.name &&
            ts.isIdentifier(member.name) &&
            member.type
          )
            add(`${node.name.text}#${member.name.text}`, member.type.getText(sf));
    }
  };
  // The facade first: its augmentations widen package methods.
  for (const file of ['../../index.d.ts', '../../pkg/finstack_quant_wasm.d.ts']) {
    const path = new URL(file, import.meta.url).pathname;
    const sf = ts.createSourceFile(path, readFileSync(path, 'utf8'), ts.ScriptTarget.Latest, true);
    visit(sf, sf.statements);
  }
  return members;
}

const MEMBERS = declaredMembers();

/** The declared return type of `key`; a pinned export that disappears fails. */
function declared(key) {
  assert.ok(MEMBERS.has(key), `${key} is not declared`);
  return MEMBERS.get(key);
}

const memberName = (key) => key.split('#').at(-1);

// A `Json`-suffixed export returns a JSON string; an unsuffixed one returns a
// structured value. The suffix is the only signal a JS caller has, so it must
// not lie (`accruedInterestJson` used to return a number).
test('Json-suffixed exports return strings', () => {
  for (const key of [
    'validateCovenantSpecJson',
    'validateCovenantReportJson',
    'validateCovenantEngineJson',
    'valuationResultToJson',
    'attributePnlEnvelopeJson',
  ])
    assert.equal(declared(key), 'string', `${key} is Json-suffixed but returns ${declared(key)}`);
  // The rule holds for every Json-suffixed member (`fromJson` builds a handle,
  // not a wire surface). A lookup that can miss returns the JSON string or
  // `undefined` (Rust `Option<String>`).
  for (const [key, ret] of MEMBERS) {
    const name = memberName(key);
    if (name.endsWith('Json') && !name.startsWith('from'))
      assert.ok(
        ['string', 'string | undefined'].includes(ret),
        `${key} is Json-suffixed but returns ${ret}`
      );
  }
});

// Computation results are structured values, never bare strings.
test('computation results are structured, not strings', () => {
  for (const key of [
    'priceInstrument',
    'priceInstrumentWithMarket',
    'calibrate',
    'dryRun',
    'attributePnl',
    'attributePnlEnvelope',
    'trancheOas',
    'trancheMetrics',
    'trancheScenarioTable',
    'runChecks',
    'runThreeStatementChecks',
    'runCreditUnderwritingChecks',
    'generateTornadoEntries',
    'FactorCovarianceForecast#covarianceAt',
    'FactorCovarianceForecast#factorModelAt',
    'FxHandle#price',
    'instrumentCashflows',
    'instrumentCashflowsWithMarket',
    'attributePnlMany',
    'attributeReturnContribution',
    'collapseToBaseByDateKind',
    'portfolioMetricsSeries',
    'nodeToDatedSchedule',
    'statementResultToTableLong',
    'statementResultToTableWide',
    'scenarioComparisonTable',
    'plSummaryReport',
    'normalize',
    'Evaluator#evaluate',
    'Evaluator#evaluateMonteCarlo',
    'DependencyTracer#dependencyTree',
    'AssetBackedFacility#project',
    'RevolvingCredit#expectedCashflows',
  ])
    assert.notEqual(
      declared(key),
      'string',
      `${key} is a computation result but returns a bare string; return a structured object ` +
        '(or add a Json suffix if it really is a wire surface)'
    );
});

// A member that returns a bare `string` says what the string is: a `Json`
// document, `Text` prose or `Html`. The allowlist names the strings that are
// the value itself.
test('bare string returns are named or allowlisted', () => {
  const VALUE_STRINGS = new Set([
    'Currency#toString',
    'Money#toString',
    'DayCount#toString',
    'Tenor#toString',
    'FxConversionPolicy#toString',
    'FxQuoteConvention#toString',
    // Canonical labels of the core enum, identifier and calendar handles.
    'HolidayCalendar#toString',
    'PeriodKind#toString',
    'PeriodId#toString',
    'SifmaSettlementClass#toString',
    'TenorUnit#toString',
    'RoundingMode#toString',
    'UnknownScalePolicy#toString',
    'CreditRating#toString',
    'CurveId#toString',
    'InstrumentId#toString',
    // Identifier text and rating spellings: the value itself.
    'CurveId#asStr',
    'InstrumentId#asStr',
    'CreditRating#toMoodysString',
    'RatingScaleRegistry#defaultScaleId',
    'ratingScalesExtensionKey',
    // Exact decimal text of the amount.
    'Money#amountDecimal',
    // Display formatting of the amount.
    'Money#formatWith',
    // Canonical return-frequency label.
    'Performance#getFrequency',
    // Canonical liquidity-tier label.
    'liquidityTier',
    // Canonical formula text (re-parseable), the Python `parse_formula` twin.
    'parseFormula',
    // Canonical "sha256:<hex>" content hashes.
    'calibrationEnvelopeContentHash',
    'calibrationResultContentHash',
    'financialModelContentHash',
    // Canonical horizon descriptor ("one_step", "unconditional", "years:<n>").
    'VolHorizon#toString',
    // Rating label of a simulated path at a time / of a WARF factor.
    'RatingPath#labelAt',
    'RatingScale#ratingFromWarf',
    // ISO-8601 dates of the underlying swap.
    'Swaption#getUnderlyingStartDate',
    'Swaption#getUnderlyingMaturity',
    // ISO-8601 valuation date after a scenario's time roll.
    'applicationReportEffectiveAsOf',
  ]);
  const offenders = [...MEMBERS]
    .filter(([key, ret]) => ret === 'string' && !/(Json|Text|Html)$/.test(memberName(key)))
    .map(([key]) => key)
    .filter((key) => !VALUE_STRINGS.has(key));
  assert.deepEqual(
    offenders,
    [],
    'these members return a bare string without a Json/Text/Html suffix; return a structured object or name the string'
  );
});

// Text-returning exports say so in their names: they share the string return
// with the JSON exports, so the name is the only way to tell prose from a
// parseable document.
test('prose-returning exports are named Text', () => {
  for (const key of ['plSummaryReportText', 'creditAssessmentReportText', 'explainFormulaText'])
    assert.equal(declared(key), 'string');
  // The pre-refactor names are gone, not aliased (`plSummaryReport` is the
  // structured table twin of Python `pl_summary_report`, pinned above).
  for (const stale of ['parseFormulaText', 'creditAssessmentReport'])
    assert.ok(!MEMBERS.has(stale), `the pre-refactor export ${stale} is still declared`);
});

// Numeric vectors cross the boundary as `Float64Array`, not boxed-number arrays.
test('numeric vector exports declare Float64Array', () => {
  for (const key of [
    'correlationBounds',
    'CorrelatedBernoulli#jointProbabilities',
    'nearestCorrelation',
    'SabrSmile#generateSmile',
  ])
    assert.ok(
      declared(key).includes('Float64Array'),
      `${key} returns a numeric vector but declares ${declared(key)}`
    );
});

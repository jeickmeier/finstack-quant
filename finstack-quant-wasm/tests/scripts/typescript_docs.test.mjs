import assert from 'node:assert/strict';
import { copyFileSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const root = dirname(dirname(dirname(fileURLToPath(import.meta.url))));
const scripts = join(root, 'scripts');
const fixtures = join(root, 'tests', 'scripts', 'fixtures', 'typescript-docs');

function run(script, ...args) {
  return spawnSync(process.execPath, [join(scripts, script), ...args], {
    cwd: root,
    encoding: 'utf8',
  });
}

function temporaryFixture(t, fixture) {
  const directory = mkdtempSync(join(tmpdir(), 'finstack-typescript-docs-'));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const target = join(directory, fixture);
  copyFileSync(join(fixtures, fixture), target);
  return target;
}

test('synchronizer replaces contract tags while preserving prose and examples', (t) => {
  const facade = temporaryFixture(t, 'facade.stale.d.ts');
  const result = run(
    'sync-facade-jsdoc.mjs',
    `--facade=${facade}`,
    `--raw=${join(fixtures, 'raw.d.ts')}`,
    '--write'
  );
  assert.equal(result.status, 0, result.stderr);
  assert.equal(
    readFileSync(facade, 'utf8'),
    readFileSync(join(fixtures, 'facade.expected.d.ts'), 'utf8')
  );
});

test('synchronizer check mode detects stale declarations', () => {
  const raw = `--raw=${join(fixtures, 'raw.d.ts')}`;
  const clean = run(
    'sync-facade-jsdoc.mjs',
    `--facade=${join(fixtures, 'facade.expected.d.ts')}`,
    raw,
    '--check'
  );
  assert.equal(clean.status, 0, clean.stderr);

  const stale = run(
    'sync-facade-jsdoc.mjs',
    `--facade=${join(fixtures, 'facade.stale.d.ts')}`,
    raw,
    '--check'
  );
  assert.equal(stale.status, 1);
  assert.match(stale.stderr, /facade JSDoc is not synchronized/);
});

test('namespace properties do not inherit same-named function contracts', (t) => {
  const directory = mkdtempSync(join(tmpdir(), 'finstack-typescript-docs-'));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const facade = join(directory, 'facade.d.ts');
  const raw = join(directory, 'raw.d.ts');
  writeFileSync(
    facade,
    `export interface ModelsNamespace {
  /** Credit correlation models. */
  correlation: CorrelationNamespace;
}
export interface MathNamespace {
  /** Compute sample correlation. */
  correlation(values: number[]): number;
}
`
  );
  writeFileSync(
    raw,
    `/**
 * Compute sample correlation.
 * @returns Sample correlation in [-1, 1].
 */
export function correlation(values: number[]): number;
`
  );
  const result = run('sync-facade-jsdoc.mjs', `--facade=${facade}`, `--raw=${raw}`, '--write');
  assert.equal(result.status, 0, result.stderr);
  const updated = readFileSync(facade, 'utf8');
  const [models, math] = updated.split('export interface MathNamespace');
  assert.doesNotMatch(models, /@returns/);
  assert.match(math, /@returns Sample correlation in \[-1, 1\]/);
});

test('completer removes only legacy fabricated documentation', (t) => {
  const declaration = temporaryFixture(t, 'checker.legacy.d.ts');
  const result = run('complete-facade-jsdoc.mjs', `--declaration=${declaration}`, '--write');
  assert.equal(result.status, 0, result.stderr);
  const completed = readFileSync(declaration, 'utf8');
  assert.doesNotMatch(completed, /supplied values are malformed, violate/);
  assert.doesNotMatch(completed, /void (?:api|factory);/);
  assert.doesNotMatch(completed, /Supply the documented arguments/);
  assert.match(completed, /@returns Returns a `Calculator` handle\./);
});

test('completer does not emit residual generic boilerplate', (t) => {
  const directory = mkdtempSync(join(tmpdir(), 'finstack-typescript-docs-'));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const target = join(directory, 'thin.d.ts');
  writeFileSync(
    target,
    `export interface SampleResult {
  intercept: number;
}
export interface Sample {
  delta(spot: number): number;
  items(): Float64Array;
}
`
  );
  const result = run('complete-facade-jsdoc.mjs', `--declaration=${target}`, '--write');
  assert.equal(result.status, 0, result.stderr);
  const completed = readFileSync(target, 'utf8');
  assert.doesNotMatch(completed, /exposed by this/);
  assert.doesNotMatch(completed, /units described above/);
  assert.doesNotMatch(completed, /in the documented order/);
  assert.doesNotMatch(completed, /Perform delta for this/);
  assert.doesNotMatch(completed, /declared TypeScript shape/);
  assert.doesNotMatch(completed, /or WebAssembly handle/);
  assert.doesNotMatch(completed, /requested string representation or JSON payload/);
  assert.doesNotMatch(completed, /TypeScript view of the/);
  assert.doesNotMatch(completed, /consumed by this/);
  assert.doesNotMatch(completed, /documented condition/);
  assert.doesNotMatch(completed, /Create a new `/);
  assert.doesNotMatch(completed, /Create the object from its inputs/);
  assert.doesNotMatch(completed, /Whether to enable/);
  assert.doesNotMatch(completed, /accepted by this operation/);
  assert.doesNotMatch(completed, /Construction and factory entry points/);
  assert.doesNotMatch(completed, /Compute delta for this/);
  // No unit is derived from the method name: the missing `@returns` repeats the summary.
  assert.doesNotMatch(completed, /change in value per|per 1\.0 absolute|per year of calendar/);
  assert.match(completed, /@returns Delta for this `Sample`\./);
  // An undocumented parameter stays undocumented so the checker reports it.
  assert.doesNotMatch(completed, /@param spot/);
});

function temporaryDeclarations(t, files) {
  const directory = mkdtempSync(join(tmpdir(), 'finstack-typescript-docs-'));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  return Object.fromEntries(
    Object.entries(files).map(([name, text]) => {
      const path = join(directory, name);
      writeFileSync(path, text);
      return [name, path];
    })
  );
}

test('synchronizer maps renamed JSON parameters and keeps facade-only parameters', (t) => {
  const paths = temporaryDeclarations(t, {
    'facade.d.ts': `export interface CalibrationNamespace {
  /**
   * Fit the loading scale from the market surface.
   * @param instrument - Stale facade text.
   * @param extra - Facade-only option with its own description.
   * @returns Stale return text.
   */
  fit(instrument: object | string, asOf: string, extra?: boolean): number;
}
`,
    'raw.d.ts': `/**
 * Fit the loading scale from the market surface.
 * @param instrument_json - Bermudan swaption instrument envelope.
 * @param as_of - ISO-8601 valuation date.
 * @returns Positive finite base volatility.
 */
export function fit(instrument_json: any, as_of: any): number;
`,
  });
  const args = [`--facade=${paths['facade.d.ts']}`, `--raw=${paths['raw.d.ts']}`];
  const result = run('sync-facade-jsdoc.mjs', ...args, '--write');
  assert.equal(result.status, 0, result.stderr);
  const updated = readFileSync(paths['facade.d.ts'], 'utf8');
  assert.match(updated, /@param instrument - Bermudan swaption instrument envelope\./);
  assert.match(updated, /@param asOf - ISO-8601 valuation date\./);
  assert.match(updated, /@param extra - Facade-only option with its own description\./);
  assert.match(updated, /@returns Positive finite base volatility\./);
  assert.doesNotMatch(updated, /Stale/);
  // The written file is a fixed point: the check gate passes on it.
  assert.equal(run('sync-facade-jsdoc.mjs', ...args, '--check').status, 0);
});

test('synchronizer documents `<Name>Instrument` interfaces from the raw class', (t) => {
  const paths = temporaryDeclarations(t, {
    'facade.d.ts': `export interface FxOptionInstrument {
  /**
   * Vega of the option.
   * @returns Vega: change in value per 1.0 absolute move in implied volatility.
   */
  vega(): number;
}
`,
    'raw.d.ts': `export class FxOption {
  /**
   * Vega of the option.
   * @returns Cash vega: PV change for a 1 vol-point (0.01 absolute) move in implied volatility.
   */
  vega(): number;
}
`,
  });
  const result = run(
    'sync-facade-jsdoc.mjs',
    `--facade=${paths['facade.d.ts']}`,
    `--raw=${paths['raw.d.ts']}`,
    '--write'
  );
  assert.equal(result.status, 0, result.stderr);
  const updated = readFileSync(paths['facade.d.ts'], 'utf8');
  assert.match(updated, /@returns Cash vega: PV change for a 1 vol-point/);
  assert.doesNotMatch(updated, /per 1\.0 absolute move/);
});

test('checker rejects placeholders, template units, internal names, repeats and orphan blocks', (t) => {
  const paths = temporaryDeclarations(t, {
    'bad.d.ts': `/**
 * Facade interface used to exercise the documentation checker.
 */
export interface Sample {
  /**
   * Stale description left above the real one.
   */
  /**
   * Calibrate against the supplied instrument.
   * @param instrument - Instrument used by this call.
   * @returns Calibrated base volatility as a decimal.
   */
  calibrate(instrument: string): number;
  /**
   * Vega of the option under the selected model.
   * @returns Vega: change in value per 1.0 absolute move in implied volatility.
   */
  vega(): number;
  /**
   * Theta of the option under the selected model.
   * @returns Theta: change in value per year of calendar time.
   */
  theta(): number;
  /**
   * Currency of this amount, a [\`JsCurrency\`] wrapper.
   * @returns The currency this amount is tagged with.
   */
  currency(): string;
  /**
   * Lookback returns ending on the reference date.
   * @param refDate - ISO-8601 date on which the windows end.
   * @param refDate - ISO-8601 reference date.
   * @returns Lookback returns as decimal fractions.
   */
  lookback(refDate: string): number;
}
`,
  });
  const result = run('check-typescript-docs.mjs', `--declaration=${paths['bad.d.ts']}`);
  assert.equal(result.status, 1);
  assert.match(result.stderr, /Sample\.calibrate: contains placeholder @param/);
  assert.match(result.stderr, /Sample\.vega: contains template Greek unit/);
  assert.match(result.stderr, /Sample\.theta: contains template Greek unit/);
  assert.match(result.stderr, /Sample\.currency: names the internal Rust wrapper \[`JsCurrency`\]/);
  assert.match(result.stderr, /Sample\.lookback: documents @param `refDate` more than once/);
  assert.match(result.stderr, /orphan JSDoc block before another JSDoc block/);
  assert.match(result.stderr, /6 error\(s\)/);
});

test('checker accepts concrete contracts and rejects exact legacy shapes', () => {
  const valid = run(
    'check-typescript-docs.mjs',
    `--declaration=${join(fixtures, 'checker.valid.d.ts')}`
  );
  assert.equal(valid.status, 0, valid.stderr);

  const legacy = run(
    'check-typescript-docs.mjs',
    `--declaration=${join(fixtures, 'checker.legacy.d.ts')}`
  );
  assert.equal(legacy.status, 1);
  assert.match(legacy.stderr, /fabricated catch-all @throws boilerplate/);
  assert.match(legacy.stderr, /non-executable placeholder @example/);
});

test('write and check modes are mutually exclusive', () => {
  const completed = run(
    'complete-facade-jsdoc.mjs',
    `--declaration=${join(fixtures, 'checker.valid.d.ts')}`,
    '--write',
    '--check'
  );
  assert.equal(completed.status, 2);
  assert.match(completed.stderr, /mutually exclusive/);

  const synchronized = run(
    'sync-facade-jsdoc.mjs',
    `--facade=${join(fixtures, 'facade.expected.d.ts')}`,
    `--raw=${join(fixtures, 'raw.d.ts')}`,
    '--write',
    '--check'
  );
  assert.equal(synchronized.status, 2);
  assert.match(synchronized.stderr, /mutually exclusive/);
});

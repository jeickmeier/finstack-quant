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
  assert.match(completed, /Spot delta: change in value per unit spot/);
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

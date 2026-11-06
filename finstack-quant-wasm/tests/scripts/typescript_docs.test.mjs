import assert from 'node:assert/strict';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
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

test('checker requires class examples and skips wasm-bindgen glue', (t) => {
  const paths = temporaryDeclarations(t, {
    'pkg.d.ts': `/**
 * Handle without a runnable sample on the class or its constructor.
 */
export class Bare {
  private constructor();
  free(): void;
  [Symbol.dispose](): void;
}
/**
 * Handle whose constructor carries the runnable sample.
 */
export class Sampled {
  free(): void;
  /**
   * Build a handle from its finite seed value.
   * @param seed - Finite seed value.
   * @example
   * \`\`\`typescript
   * new Sampled(1);
   * \`\`\`
   */
  constructor(seed: number);
}
export type InitInput = RequestInfo | URL;
export function initSync(module: unknown): unknown;
`,
  });
  const result = run('check-typescript-docs.mjs', `--declaration=${paths['pkg.d.ts']}`);
  assert.equal(result.status, 1);
  assert.match(result.stderr, /class Bare: missing @example/);
  assert.doesNotMatch(result.stderr, /Sampled|free|Symbol\.dispose|InitInput|initSync/);
  assert.match(result.stderr, /1 error\(s\)/);
});

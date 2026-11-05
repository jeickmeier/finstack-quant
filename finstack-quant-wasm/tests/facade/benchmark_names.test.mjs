/**
 * `benchmarks/bench.mjs` calls the raw package (`pkg-node/`) by flat export
 * name through a dynamic import, which no type check sees. This test fails when
 * the benchmark calls a name the package no longer exports, so a rename cannot
 * leave the benchmark broken until its next manual run.
 */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';

const bench = readFileSync(new URL('../../benchmarks/bench.mjs', import.meta.url), 'utf8');
const dtsPath = new URL('../../pkg/finstack_quant_wasm.d.ts', import.meta.url).pathname;
const dts = ts.createSourceFile(
  dtsPath,
  readFileSync(dtsPath, 'utf8'),
  ts.ScriptTarget.Latest,
  true
);
const exported = new Set(
  dts.statements
    .filter((st) => st.name && st.modifiers?.some((m) => m.kind === ts.SyntaxKind.ExportKeyword))
    .map((st) => st.name.text)
);

test('the benchmark calls only names the package exports', () => {
  // The benchmark binds the loaded module to `w`.
  const used = new Set([...bench.matchAll(/\bw\.([A-Za-z_]\w*)/g)].map(([, name]) => name));
  assert.ok(used.size > 0, 'no `w.<name>` calls found; update this test with the benchmark');
  assert.deepEqual(
    [...used].filter((name) => !exported.has(name)).sort(),
    [],
    'benchmarks/bench.mjs calls names the package does not export'
  );
});

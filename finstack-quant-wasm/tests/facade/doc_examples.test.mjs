// Execute every documented code sample against the built package.
//
// The samples are the `@example` blocks of `index.d.ts` and the `js`/`ts`
// blocks of the package README and the WASM usage rules (see
// `scripts/doc-examples.mjs`). `scripts/check-typescript-examples.mjs`
// type-checks the same blocks; this file proves they run.
//
// Each sample is written unmodified to a module inside the package, so its
// `import ... from "finstack-quant-wasm"` resolves to `index.js` through the
// package's own `exports` map. The module is instantiated once, here, with the
// `.wasm` bytes; a sample's own `await init()` then returns that instance
// instead of fetching a URL, which Node cannot do for a file.
import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import ts from 'typescript';
import { PACKAGE_DIR, collectExamples } from '../../scripts/doc-examples.mjs';

const WASM_BG = join(PACKAGE_DIR, 'pkg', 'finstack_quant_wasm_bg.wasm');
if (!existsSync(WASM_BG)) {
  throw new Error(
    `finstack-quant-wasm web build not found at ${WASM_BG}. Generate it with: npm run build`
  );
}
const { default: init } = await import('../../index.js');
await init({ module_or_path: readFileSync(WASM_BG) });

// `target/` is gitignored and, unlike a temp directory, inside the package.
const SCRATCH = join(PACKAGE_DIR, 'target', 'doc-examples');
rmSync(SCRATCH, { recursive: true, force: true });
mkdirSync(SCRATCH, { recursive: true });
test.after(() => rmSync(SCRATCH, { recursive: true, force: true }));

const examples = collectExamples();

test('the documentation has samples to run', () => {
  assert.ok(examples.length > 200, `only ${examples.length} samples found`);
});

for (const example of examples) {
  test(`${example.source}:${example.line} runs`, async () => {
    const code =
      example.language === 'ts'
        ? ts.transpileModule(example.code, {
            compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
          }).outputText
        : example.code;
    const file = join(SCRATCH, `${example.name}.mjs`);
    writeFileSync(file, code);
    // Samples print their results (and the errors they catch on purpose);
    // keep the test output readable. A failing sample still throws.
    const saved = { ...console };
    for (const method of ['log', 'info', 'warn', 'error']) console[method] = () => {};
    try {
      await import(pathToFileURL(file).href);
    } finally {
      Object.assign(console, saved);
    }
  });
}

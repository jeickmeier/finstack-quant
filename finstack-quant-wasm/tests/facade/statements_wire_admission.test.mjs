/**
 * The WASM facade loads the shared model documents to the outcomes Rust
 * records.
 *
 * The cases file is written by the Rust test
 * `wire_admission::every_host_document_loads_to_its_recorded_outcome`; the
 * Python test reads the same file.
 *
 * Requires the wasm-pack web build: npm run build
 */

import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const WASM_BG = join(__dirname, '..', '..', 'pkg', 'finstack_quant_wasm_bg.wasm');
const CASES_FILE = join(
  __dirname,
  '..',
  '..',
  '..',
  'finstack-quant',
  'statements',
  'tests',
  'data',
  'wire_admission.json'
);

if (!existsSync(WASM_BG)) {
  throw new Error(
    `finstack-quant-wasm web build not found at ${WASM_BG}. Generate it with: npm run build`
  );
}

const { default: init, statements } = await import('../../index.js');
await init({ module_or_path: readFileSync(WASM_BG) });

const { cases } = JSON.parse(readFileSync(CASES_FILE, 'utf8'));

/** The content hash of a loaded model, or the error's kind, code and pointer. */
function outcome(load, document) {
  try {
    load(document);
  } catch (error) {
    const first = error.report?.diagnostics?.[0];
    const contract = error.name === 'ContractValidationError';
    return {
      kind: error.kind,
      code: first?.code ?? (contract ? error.code : null),
      pointer: first?.pointer ?? null,
    };
  }
  return { content_hash: statements.financialModelContentHash(document) };
}

for (const { name, document, expect } of cases) {
  test(`model document '${name}' loads to the recorded outcome`, () => {
    assert.deepEqual(outcome(statements.validateFinancialModelJson, document), expect);
  });
}

test('every model-taking entry point uses the same loader', () => {
  const duplicate = cases.find(({ name }) => name === 'duplicate_key');
  for (const load of [
    statements.financialModelContentHash,
    (document) => statements.ModelBuilder.fromSpec(document),
    (document) => new statements.Evaluator().evaluate(document),
  ]) {
    assert.deepEqual(outcome(load, duplicate.document), duplicate.expect);
  }
});

test('a bigint seed is the same seed as its wire text', () => {
  const model = JSON.parse(cases.find(({ name }) => name === 'valid').document);
  const seeded = (seed) => ({
    ...model,
    nodes: {
      ...model.nodes,
      shock: {
        node_id: 'shock',
        node_type: 'mixed',
        value_type: { type: 'scalar' },
        forecast: { method: 'normal', params: { mean: 0, std_dev: 1, start: 0, seed } },
      },
    },
  });
  const text = statements.financialModelContentHash(seeded('9007199254740993'));
  assert.equal(statements.financialModelContentHash(seeded(9007199254740993n)), text);
  const run = (seed) => {
    const evaluator = new statements.Evaluator();
    try {
      return evaluator.evaluateMonteCarlo(model, { n_paths: 2, seed, metrics: ['revenue'] });
    } finally {
      evaluator.free();
    }
  };
  assert.equal(run(18446744073709551615n).seed, '18446744073709551615');
});

test('the Monte Carlo seed is a decimal string on the wire', () => {
  const valid = cases.find(({ name }) => name === 'valid').document;
  const run = (seed) => {
    const evaluator = new statements.Evaluator();
    try {
      return evaluator.evaluateMonteCarlo(valid, { n_paths: 2, seed, metrics: ['revenue'] });
    } finally {
      evaluator.free();
    }
  };
  assert.equal(run('18446744073709551615').seed, '18446744073709551615');
  assert.throws(
    () => run(7),
    (error) => error.kind === 'validation' && /decimal string/.test(error.message)
  );
});

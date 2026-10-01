/**
 * Facade tests for the `*.schema` namespaces bound by parity slice PX1: the
 * per-crate `index` / `get` / `validate` registries, the whole-workspace
 * `schema` namespace, the named schema accessors and the instrument validators.
 *
 * Expected values come from `schema_parity.golden.json`, which
 * `finstack-quant-py/tests/test_schema_wasm_parity.py` generates and asserts
 * through the Python binding, so each case is a cross-host golden. Python
 * returns JSON text where WASM returns plain objects.
 *
 * Requires the wasm-pack web build: npm run build (mise run wasm-build).
 */

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, * as facade from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

const golden = JSON.parse(
  readFileSync(new URL('./schema_parity.golden.json', import.meta.url), 'utf8')
);

/** The `schema` namespace at a facade path (`""` is the workspace registry). */
const schemaAt = (path) =>
  path.split('.').reduce((node, key) => (key ? node[key] : node), facade).schema;

const camel = (name) => name.replace(/_([a-z])/g, (_, letter) => letter.toUpperCase());

const summary = (document) => ({
  id: document.$id ?? null,
  title: document.title ?? null,
  keys: Object.keys(document).sort(),
  definitions: Object.keys(document.$defs ?? {}).sort(),
});

const INDEX_COLUMNS = ['domain', 'path', '$id', 'type_name', 'kind', 'bytes'];
const indexSummary = (index) => ({
  schema_index_version: index.schema_index_version,
  artifacts: index.artifacts.map((row) =>
    Object.fromEntries(INDEX_COLUMNS.filter((key) => key in row).map((key) => [key, row[key]]))
  ),
});

for (const [path, expected] of Object.entries(golden.registries)) {
  const label = path ? `${path}.schema` : 'schema';
  test(`${label} index/get/validate return the Python documents`, () => {
    const schema = schemaAt(path);
    const { selector } = expected;
    const index = schema.index();
    assert.equal(Object.getPrototypeOf(index), Object.prototype);
    assert.deepEqual(indexSummary(index), expected.index);
    assert.deepEqual(summary(schema.get(selector)), expected.canonical);
    assert.deepEqual(summary(schema.get(selector, 'canonical')), expected.canonical);
    assert.deepEqual(summary(schema.get(selector, 'llm')), expected.llm);
    assert.deepEqual(schema.validate(selector, golden.invalid_payload), expected.report);
    assert.deepEqual(
      schema.validate(selector, JSON.parse(golden.invalid_payload)),
      expected.report
    );
  });

  test(`${label} reports bad selectors, profiles and payloads by kind`, () => {
    const schema = schemaAt(path);
    const { selector } = expected;
    assert.throws(
      () => schema.get('no_such_schema.json'),
      (error) => error.kind === 'not_found'
    );
    assert.throws(
      () => schema.validate('no_such_schema.json', {}),
      (error) => error.kind === 'not_found'
    );
    assert.throws(
      () => schema.get(selector, 'nope'),
      (error) => error.kind === 'validation' && /unknown schema profile/.test(error.message)
    );
    assert.throws(
      () => schema.validate(selector, '{"unterminated":'),
      (error) => error.kind === 'validation'
    );
    assert.throws(
      () => schema.get(42),
      (error) => error instanceof TypeError && error.kind === 'invalid_type'
    );
    assert.throws(
      () => schema.validate(selector, 42),
      (error) => error instanceof TypeError && error.kind === 'invalid_type'
    );
  });
}

test('a schema example validates against its own schema', () => {
  const document = facade.valuations.schema.get('bond.schema.json');
  assert.deepEqual(facade.valuations.schema.validate('bond.schema.json', document.examples[0]), []);
  assert.deepEqual(facade.schema.validate('bond.schema.json', document.examples[0]), []);
});

test('schema.domains lists the domains of the workspace index', () => {
  assert.deepEqual(facade.schema.domains(), golden.domains);
  const domains = new Set(facade.schema.index().artifacts.map((row) => row.domain));
  assert.deepEqual([...domains].sort(), golden.domains);
});

test('named schema accessors return the Python documents', () => {
  for (const [path, accessors] of Object.entries(golden.named)) {
    const schema = schemaAt(path);
    for (const [name, expected] of Object.entries(accessors)) {
      assert.deepEqual(summary(schema[camel(name)]()), expected, `${path}.schema.${name}`);
    }
  }
});

test('cashflows.schema.resources maps every schema id to its document', () => {
  const resources = facade.cashflows.schema.resources();
  assert.equal(Object.getPrototypeOf(resources), Object.prototype);
  assert.deepEqual(Object.keys(resources), Object.keys(golden.cashflows_resources));
  for (const [uri, expected] of Object.entries(golden.cashflows_resources)) {
    assert.deepEqual(summary(resources[uri]), expected, uri);
  }
});

test('instrument validators return the canonical JSON Python returns', () => {
  const { schema } = facade.valuations;
  const { json, envelope, typed } = golden.instrument;
  assert.equal(schema.validateInstrumentEnvelopeJson(json), envelope);
  assert.equal(schema.validateInstrumentTypeJson('bond', json), typed);
  assert.equal(schema.validateInstrumentTypeJson('bond', JSON.parse(typed)), typed);

  const broken = JSON.parse(json);
  delete broken.instrument.spec.maturity;
  for (const call of [
    () => schema.validateInstrumentEnvelopeJson(broken),
    () => schema.validateInstrumentTypeJson('bond', broken),
    () => schema.validateInstrumentEnvelopeJson('{"schema":'),
  ]) {
    assert.throws(call, (error) => error.kind === 'validation');
  }
  assert.throws(
    () => schema.validateInstrumentTypeJson('not_a_type', json),
    (error) => error.kind === 'not_found'
  );
  assert.throws(
    () => schema.validateInstrumentEnvelopeJson(42),
    (error) => error instanceof TypeError && error.kind === 'invalid_type'
  );
});

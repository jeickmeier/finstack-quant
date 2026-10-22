import assert from 'node:assert/strict';
import { test } from 'node:test';
import { compile } from 'json-schema-to-typescript';
import { prepareSchema } from '../../scripts/prepare-schema.mjs';

const tuple = {
  type: 'array',
  minItems: 2,
  maxItems: 2,
  prefixItems: [{ type: 'string' }, { type: 'number' }],
};

test('fixed tuple preparation preserves references, nullability and source schemas', () => {
  const source = {
    $defs: { Amount: { type: 'string' } },
    anyOf: [
      { ...tuple, prefixItems: [{ type: 'string', format: 'date' }, { $ref: '#/$defs/Amount' }] },
      { type: 'null' },
    ],
  };
  const original = structuredClone(source);
  const prepared = prepareSchema(source);
  assert.deepEqual(source, original);
  assert.deepEqual(prepared.anyOf[0].items, source.anyOf[0].prefixItems);
  assert.equal(prepared.anyOf[0].additionalItems, false);
  assert.equal(prepared.anyOf[0].minItems, 2);
  assert.equal(prepared.anyOf[0].maxItems, 2);
  assert.equal(Object.hasOwn(prepared.anyOf[0], 'prefixItems'), false);
  assert.deepEqual(prepared.$defs, source.$defs);
  assert.deepEqual(prepared.anyOf[1], { type: 'null' });
  assert.deepEqual(prepareSchema(prepared), prepared);
});

test('the installed compiler emits nested typed tuples with no unknown rest', async () => {
  const schema = { type: 'array', items: { anyOf: [tuple, { type: 'null' }] } };
  const output = await compile(prepareSchema(schema), 'Schedule', { bannerComment: '' });
  assert.match(output, /\[string, number\]/);
  assert.match(output, /null/);
  assert.doesNotMatch(output, /unknown|\.\.\./);
});

test('unsupported prefixItems shapes fail explicitly', () => {
  for (const invalid of [
    { ...tuple, minItems: 1 },
    { ...tuple, maxItems: 3 },
    { ...tuple, prefixItems: [] },
    { ...tuple, prefixItems: true },
    { ...tuple, type: 'object' },
    { ...tuple, items: { type: 'number' } },
  ])
    assert.throws(() => prepareSchema(invalid), /Unsupported prefixItems/);
});

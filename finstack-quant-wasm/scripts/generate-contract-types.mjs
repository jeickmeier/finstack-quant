import { readFile, writeFile } from 'node:fs/promises';
import { compile } from 'json-schema-to-typescript';

const schema = JSON.parse(
  await readFile(new URL('../schemas/host/1/valuation_result.schema.json', import.meta.url), 'utf8')
);
// This metadata is generated from Rust for to_js_value_with_bigints. Preserve the
// JSON schema for validation and project only the declaration's numeric types.
function hostTypes(value) {
  if (!value || typeof value !== 'object') return value;
  if (Array.isArray(value)) return value.map(hostTypes);
  const result = Object.fromEntries(
    Object.entries(value).map(([key, child]) => [key, hostTypes(child)])
  );
  if (value.type === 'integer' && ['uint64', 'int64'].includes(value.format))
    result.tsType = 'bigint';
  return result;
}
const output = await compile(hostTypes(schema), 'ValuationResult', {
  bannerComment:
    '// Generated from the Rust facade host schema by scripts/generate-contract-types.mjs. Do not edit.',
  unknownAny: true,
  unreachableDefinitions: true,
  $refOptions: { resolve: { file: false, http: false } },
});
const target = new URL('../types/valuation-result.d.ts', import.meta.url);
if (process.argv.includes('--check')) {
  if ((await readFile(target, 'utf8')) !== output)
    throw new Error('Host declaration drift; run mise run wasm-gen-contracts');
} else await writeFile(target, output);

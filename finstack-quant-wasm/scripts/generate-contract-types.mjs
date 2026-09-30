// Generate the package's TypeScript contract types from the Rust JSON Schemas.
//
// One mechanism (json-schema-to-typescript over Rust-generated schemas), two
// outputs:
//
// 1. `types/valuation-result.d.ts`: the host valuation-result contract
//    (`schemas/host/1/valuation_result.schema.json`). It crosses the boundary
//    through `to_js_value_with_bigints`, so its 64-bit integers are `bigint`.
// 2. `types/generated/<crate>/<crate>.ts`: every crate's checked-in schema
//    artifacts (listed by `<crate>/schemas/index.json`), bundled into one
//    module per crate. These are JSON envelopes, so integers are `number`.
//
// A crate bundle merges the `$defs` of all of its artifacts and adds every root
// under its Rust type name (`type_name` in the schema index). A `$ref` to
// another crate's artifact becomes a type import of that crate's module. A
// name defined twice with different assertions is a generator error. The
// barrels (`<crate>/index.ts`, `index.ts`) are written by
// `scripts/sync_generated_ts_index.py`.
//
// `--check` compares every output byte-for-byte and fails on stale files.
import { mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { isDeepStrictEqual } from 'node:util';
import { compile } from 'json-schema-to-typescript';

const PACKAGE = new URL('../', import.meta.url);
const WORKSPACE = new URL('../finstack-quant/', PACKAGE);
const GENERATED = new URL('types/generated/', PACKAGE);
const CHECK = process.argv.includes('--check');

// Per-instrument envelopes are the branches of `instrument.schema.json`'s
// `InstrumentJson` union, and their roots share one generic Rust wrapper type.
const SKIPPED_ARTIFACTS = [/^schemas\/instruments\/1\/[^/]+\/[^/]+\.schema\.json$/];
// Keywords that annotate without constraining; ignored when comparing duplicates.
const ANNOTATIONS = new Set([
  '$comment',
  'default',
  'deprecated',
  'description',
  'examples',
  'readOnly',
  'title',
  'writeOnly',
]);

const stale = [];
const expected = new Set();

async function emit(url, text) {
  expected.add(url.href);
  if (CHECK) {
    const current = await readFile(url, 'utf8').catch(() => null);
    if (current !== text) stale.push(url.pathname);
    return;
  }
  await mkdir(new URL('./', url), { recursive: true });
  await writeFile(url, text);
}

function assertions(value) {
  if (Array.isArray(value)) return value.map(assertions);
  if (!value || typeof value !== 'object') return value;
  return Object.fromEntries(
    Object.entries(value)
      .filter(([key]) => !ANNOTATIONS.has(key))
      .map(([key, child]) => [key, assertions(child)])
  );
}

// --- 1. host valuation-result contract --------------------------------------

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

const hostSchema = JSON.parse(
  await readFile(new URL('schemas/host/1/valuation_result.schema.json', PACKAGE), 'utf8')
);
await emit(
  new URL('types/valuation-result.d.ts', PACKAGE),
  await compile(hostTypes(hostSchema), 'ValuationResult', {
    bannerComment:
      '// Generated from the Rust facade host schema by scripts/generate-contract-types.mjs. Do not edit.',
    unknownAny: true,
    unreachableDefinitions: true,
    $refOptions: { resolve: { file: false, http: false } },
  })
);

// --- 2. crate contract bundles ----------------------------------------------

const crates = [];
const entries = await readdir(WORKSPACE, { withFileTypes: true });
for (const entry of entries.sort((a, b) => a.name.localeCompare(b.name))) {
  if (!entry.isDirectory()) continue;
  const index = await readFile(
    new URL(`${entry.name}/schemas/index.json`, WORKSPACE),
    'utf8'
  ).catch(() => null);
  if (!index) continue;
  const artifacts = [];
  for (const row of JSON.parse(index).artifacts) {
    const text = await readFile(new URL(`${entry.name}/${row.path}`, WORKSPACE), 'utf8');
    const skipped = SKIPPED_ARTIFACTS.some((pattern) => pattern.test(row.path));
    artifacts.push({ ...row, schema: JSON.parse(text), skipped });
  }
  crates.push({ name: entry.name, module: entry.name.replaceAll('-', '_'), artifacts });
}

// `$id` of every bundled root -> owning crate module and Rust type name.
const owners = new Map();
for (const crate of crates)
  for (const artifact of crate.artifacts)
    if (!artifact.skipped)
      owners.set(artifact.$id, { module: crate.module, name: artifact.type_name });

// A struct that flattens a tagged enum is an object with `properties` and a
// `oneOf` of variants. Emit it as a union of complete objects (shared fields
// copied into every variant); `unevaluatedProperties: false` on the shared part
// means exactly "no keys beyond shared + variant" in each merged branch.
function distribute(node) {
  if (!node.properties || !Array.isArray(node.oneOf) || node.anyOf || node.allOf) return node;
  const { properties, required = [], oneOf, unevaluatedProperties, ...rest } = node;
  delete rest.type;
  const closed = unevaluatedProperties === false ? { additionalProperties: false } : {};
  return {
    ...rest,
    oneOf: oneOf.map((variant) => ({
      ...variant,
      ...closed,
      type: 'object',
      properties: { ...properties, ...variant.properties },
      required: [...new Set([...required, ...(variant.required ?? [])])],
    })),
  };
}

function bundle(crate) {
  const defs = new Map();
  const imports = new Set();
  const define = (name, schema, origin) => {
    const existing = defs.get(name);
    if (!existing) defs.set(name, { schema, origin });
    else if (!isDeepStrictEqual(assertions(existing.schema), assertions(schema)))
      throw new Error(`${crate.name}: '${name}' differs between ${existing.origin} and ${origin}`);
  };
  const rewrite = (value, rootName) => {
    if (Array.isArray(value)) return value.map((child) => rewrite(child, rootName));
    if (!value || typeof value !== 'object') return value;
    const result = {};
    for (const [key, child] of Object.entries(value)) {
      // TypeScript has no use for these; json-schema-to-typescript would
      // otherwise infer `X & string` from a string `default` beside a `$ref`.
      if (key === 'title' || key === 'examples' || key === 'default') continue;
      if (key !== '$ref') result[key] = rewrite(child, rootName);
      else if (child === '#') result.$ref = `#/$defs/${rootName}`;
      else if (/^#\/\$defs\/[^/]+$/.test(child)) result.$ref = child;
      else {
        const owner = owners.get(child);
        if (!owner) throw new Error(`${crate.name}: unresolved $ref ${child}`);
        if (owner.module === crate.module) result.$ref = `#/$defs/${owner.name}`;
        else {
          imports.add(owner.module);
          result.tsType = `${owner.module}.${owner.name}`;
        }
      }
    }
    // A `$ref` with sibling annotations makes json-schema-to-typescript
    // dereference into a fresh copy (named `Money1`, ...). Keep the reference
    // bare and hang the annotations on a one-branch `allOf`.
    const { $ref, ...rest } = result;
    if ($ref !== undefined && Object.keys(rest).length) return { ...rest, allOf: [{ $ref }] };
    return distribute(result);
  };
  for (const artifact of crate.artifacts) {
    if (artifact.skipped) continue;
    const { $defs = {}, ...root } = artifact.schema;
    delete root.$id;
    delete root.$schema;
    for (const [name, schema] of Object.entries($defs))
      define(name, rewrite(schema, artifact.type_name), artifact.path);
    define(artifact.type_name, rewrite(root, artifact.type_name), artifact.path);
  }
  const names = [...defs.keys()].sort();
  const schema = { $defs: Object.fromEntries(names.map((name) => [name, defs.get(name).schema])) };
  return { schema, imports: [...imports].sort() };
}

const ROOT = '__FinstackContractBundle';
for (const crate of crates) {
  const { schema, imports } = bundle(crate);
  const compiled = await compile(schema, ROOT, {
    bannerComment: '',
    unknownAny: true,
    unreachableDefinitions: true,
    customName: (_schema, key) => key,
    $refOptions: { resolve: { file: false, http: false } },
  });
  const rootDeclaration = new RegExp(
    `^export interface ${ROOT} \\{\\n  \\[k: string\\]: unknown;\\n\\}\\n`,
    'm'
  );
  if (!rootDeclaration.test(compiled)) throw new Error(`${crate.name}: bundle root not emitted`);
  const declared = [...compiled.matchAll(/^export (?:type|interface) (\w+)/gm)].map((m) => m[1]);
  const invented = declared.filter((name) => name !== ROOT && !(name in schema.$defs));
  if (invented.length)
    throw new Error(`${crate.name}: names not in the Rust schemas: ${invented.join(', ')}`);
  const header = [
    `// Generated from the finstack-quant-${crate.name} JSON schemas by scripts/generate-contract-types.mjs. Do not edit.`,
    ...imports.map((module) => `import type * as ${module} from '../${module}/index.js';`),
  ].join('\n');
  const body = compiled
    .replace(rootDeclaration, '')
    .replace(
      /(?: \*\n)? \* This (?:interface|type) was referenced by `\w+`'s JSON-Schema\n \* via the `definition` "\w+"\.\n/g,
      ''
    )
    .replace(/\/\*\*\n \*\/\n/g, '')
    .replace(/\n{3,}/g, '\n\n')
    .trim();
  if (body.includes(ROOT)) throw new Error(`${crate.name}: bundle root leaked into the output`);
  await emit(new URL(`${crate.module}/${crate.module}.ts`, GENERATED), `${header}\n\n${body}\n`);
}

// Barrels belong to sync_generated_ts_index.py; every other file under
// types/generated must be an output of this run.
const present = [];
async function walk(dir) {
  for (const entry of await readdir(dir, { withFileTypes: true }).catch(() => [])) {
    const url = new URL(entry.name + (entry.isDirectory() ? '/' : ''), dir);
    if (entry.isDirectory()) await walk(url);
    else if (entry.name !== 'index.ts') present.push(url);
  }
}
await walk(GENERATED);
for (const url of present.filter((candidate) => !expected.has(candidate.href))) {
  if (CHECK) stale.push(url.pathname);
  else await rm(url);
}
if (stale.length)
  throw new Error(
    `Generated contract types drift; run mise run wasm-gen-bindings:\n${stale.join('\n')}`
  );

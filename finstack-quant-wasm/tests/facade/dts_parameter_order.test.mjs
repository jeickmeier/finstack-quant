/**
 * A declaration in `index.d.ts` must list its parameters in the order of the
 * `#[wasm_bindgen]` signature it declares, read from the raw wasm-bindgen
 * declarations in `pkg/finstack_quant_wasm.d.ts`. `tsc` cannot see a swap
 * when the swapped parameters share a type (Black-76 `vol` and `expiry` are
 * both `number`).
 *
 * Each raw export is compared with the same-named declarations on its class,
 * the interfaces that class extends and its `*Constructor` interface, else
 * with every same-named declaration: free functions, facade renames and
 * members declared through a generic such as `FxConstructor<T>` have no
 * class-named home. The check reads the parameters the two lists share: it
 * fails when two or more of them appear in another order. JS facade wrappers
 * may add, drop or rename parameters by design, so this gate covers order
 * only, not names or arity, and an export sharing fewer than two names with
 * its declaration is not compared.
 */
import assert from 'node:assert/strict';
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import ts from 'typescript';

const root = fileURLToPath(new URL('../../', import.meta.url));
const rawPath = join(root, 'pkg', 'finstack_quant_wasm.d.ts');

const parse = (path) =>
  ts.createSourceFile(path, readFileSync(path, 'utf8'), ts.ScriptTarget.Latest, true);

const camelCase = (name) =>
  name.replace(/^_+/, '').replace(/_([a-z0-9])/gi, (_, ch) => ch.toUpperCase());

/** A `Json` suffix is a Rust naming detail several declarations drop (`specJson` is declared `spec`). */
const comparable = (name) => name.replace(/Json$/, '');

const paramNames = (node, file) =>
  node.parameters.map((param) => param.name.getText(file).replace(/^\.\.\./, ''));

/** Name of a callable member; `constructor(...)` and `new (...)` are both `constructor`. */
function callableName(member, file) {
  if (ts.isConstructorDeclaration(member) || ts.isConstructSignatureDeclaration(member))
    return 'constructor';
  if (ts.isMethodDeclaration(member) || ts.isMethodSignature(member))
    return member.name.getText(file);
  return null;
}

/** Every raw export: free functions and the non-accessor members of each class. */
function rawExports() {
  const file = parse(rawPath);
  const exports = [];
  for (const statement of file.statements) {
    if (ts.isFunctionDeclaration(statement) && statement.name) {
      exports.push({ name: statement.name.text, cls: null, params: paramNames(statement, file) });
    } else if (ts.isClassDeclaration(statement) && statement.name) {
      for (const member of statement.members) {
        const name = callableName(member, file);
        if (name && !name.startsWith('[')) {
          exports.push({ name, cls: statement.name.text, params: paramNames(member, file) });
        }
      }
    }
  }
  return exports;
}

/** Every callable member of a top-level `index.d.ts` interface or class, plus each one's `extends` list. */
function declaredMembers() {
  const file = parse(join(root, 'index.d.ts'));
  const declared = [];
  const parents = new Map();
  for (const statement of file.statements) {
    if (!(ts.isInterfaceDeclaration(statement) || ts.isClassDeclaration(statement))) continue;
    if (!statement.name) continue;
    const container = statement.name.text;
    for (const clause of statement.heritageClauses ?? []) {
      if (clause.token !== ts.SyntaxKind.ExtendsKeyword) continue;
      const list = parents.get(container) ?? [];
      list.push(...clause.types.map((type) => type.expression.getText(file)));
      parents.set(container, list);
    }
    for (const member of statement.members) {
      const name = callableName(member, file);
      if (name) declared.push({ container, name, params: paramNames(member, file) });
    }
  }
  return { declared, parents };
}

/** Facade keys of the renamed exports, by raw wasm-bindgen name, from the `key: wasm.rawName` entries in `exports/**`. */
function facadeRenames() {
  const renames = new Map();
  const visit = (dir) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const path = join(dir, entry.name);
      if (entry.isDirectory()) visit(path);
      else if (entry.name.endsWith('.js')) {
        for (const match of readFileSync(path, 'utf8').matchAll(/^\s*(\w+): wasm\.(\w+),?$/gm)) {
          const keys = renames.get(match[2]) ?? new Set();
          keys.add(match[1]);
          renames.set(match[2], keys);
        }
      }
    }
  };
  visit(join(root, 'exports'));
  return renames;
}

test('index.d.ts lists parameters in the order of the wasm-bindgen signatures', () => {
  assert.ok(existsSync(rawPath), 'build the web package first: mise run wasm-build');
  const { declared, parents } = declaredMembers();
  const renames = facadeRenames();
  const byName = new Map();
  for (const decl of declared) {
    const list = byName.get(decl.name) ?? [];
    list.push(decl);
    byName.set(decl.name, list);
  }

  let compared = 0;
  const offenders = [];
  for (const exported of rawExports()) {
    const expected = exported.params.map((name) => comparable(camelCase(name)));
    const names = new Set([exported.name]);
    if (exported.cls === null) for (const key of renames.get(exported.name) ?? []) names.add(key);
    let candidates = [...names].flatMap((name) => byName.get(name) ?? []);

    if (exported.cls !== null) {
      const scope = new Set();
      const pending = [exported.cls, `${exported.cls}Constructor`];
      while (pending.length > 0) {
        const container = pending.pop();
        pending.push(...(parents.get(container) ?? []));
        scope.add(container);
      }
      const scoped = candidates.filter((decl) => scope.has(decl.container));
      if (scoped.length > 0) candidates = scoped;
    }

    const shared = candidates
      .map((decl) => ({
        decl,
        order: decl.params.map(comparable).filter((name) => expected.includes(name)),
      }))
      .filter(({ order }) => order.length >= 2);
    if (shared.length === 0) continue;
    compared += 1;
    const inOrder = shared.some(({ order }) => {
      const rust = expected.filter((name) => order.includes(name));
      return rust.join(',') === order.join(',');
    });
    if (!inOrder) {
      const { decl } = shared[0];
      const label = exported.cls === null ? exported.name : `${exported.cls}.${exported.name}`;
      offenders.push(
        `${label}: declared on ${decl.container} as (${decl.params.join(', ')}), Rust order is (${expected.join(', ')})`
      );
    }
  }
  assert.ok(compared > 600, `compared only ${compared} declarations`);
  assert.deepEqual(offenders, [], 'index.d.ts parameter order differs from the Rust binding');
});

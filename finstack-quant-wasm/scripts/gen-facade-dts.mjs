// Generates `index.d.ts`, the TypeScript declaration of the namespaced facade
// (`index.js` and `exports/**/*.js`), over wasm-bindgen's
// `pkg/finstack_quant_wasm.d.ts`. Run by `mise run gen-write`; `--check` (run
// by `mise run wasm-test`) fails when the committed file differs.
//
// Every value the facade re-exports from `wasm` is declared `typeof wasm.X`, so
// its type is whatever the Rust export's `#[wasm_bindgen]` attributes say. The
// facade adds JavaScript only where wasm-bindgen cannot express a Rust
// signature, and only that code carries a hand-written declaration, beside it:
//
// - a wrapper function: its JSDoc, ending in a
//   `@facadeType <TypeScript function type>` tag, on its statement or property;
// - a method the facade adds to a class prototype, or whose signature it
//   widens: one JSDoc block per method, ending in `@facadeClass Name` and
//   `@member name(params): Return`;
// - `chain(Class, [names])`: those methods return the receiver, so they are
//   declared from the call itself, with the parameters and JSDoc wasm-bindgen
//   declared.
//
// The JSDoc above the tags is the member's published documentation; a wrapper
// or added method without one fails generation.
//
// Prototype members are declared by augmenting the package module, so the class
// stays the one wasm-bindgen declares and every value of it, wherever it comes
// from, has the facade's members. A raw method the facade removes is marked
// `skip_typescript` in Rust and is therefore never declared.
//
// Usage: node scripts/gen-facade-dts.mjs [--check]
import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import ts from 'typescript';

const PACKAGE = fileURLToPath(new URL('../', import.meta.url));
const OUTPUT = path.join(PACKAGE, 'index.d.ts');
const PKG_DTS = path.join(PACKAGE, 'pkg', 'finstack_quant_wasm.d.ts');
const PKG_IMPORT = './pkg/finstack_quant_wasm.js';

const fail = (message) => {
  throw new Error(`gen-facade-dts: ${message}`);
};

// ── The package's exported names, JSDoc and class methods ────────────────────
const pkgText = readFileSync(PKG_DTS, 'utf8');
const pkg = ts.createSourceFile(PKG_DTS, pkgText, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
const pkgNames = new Set();
const pkgDocs = new Map(); // exported function name -> JSDoc text
const pkgMethods = new Map(); // class name -> Map(method name -> { params, doc })
const isExported = (node) => node.modifiers?.some((m) => m.kind === ts.SyntaxKind.ExportKeyword);
for (const st of pkg.statements) {
  if (ts.isExportDeclaration(st) && st.exportClause && ts.isNamedExports(st.exportClause))
    for (const el of st.exportClause.elements) pkgNames.add(el.name.text);
  if (!isExported(st)) continue;
  if (ts.isVariableStatement(st))
    for (const d of st.declarationList.declarations) pkgNames.add(d.name.text);
  else if (st.name) {
    pkgNames.add(st.name.text);
    if (ts.isFunctionDeclaration(st)) {
      const docs = ts.getJSDocCommentsAndTags(st).filter(ts.isJSDoc);
      if (docs.length > 0) pkgDocs.set(st.name.text, docs.at(-1).getText(pkg));
    }
    if (ts.isClassDeclaration(st)) {
      const methods = new Map();
      for (const member of st.members)
        if (ts.isMethodDeclaration(member) && member.name && ts.isIdentifier(member.name)) {
          const docs = ts.getJSDocCommentsAndTags(member).filter(ts.isJSDoc);
          methods.set(member.name.text, {
            params: member.parameters.map((p) => p.getText(pkg)).join(', '),
            doc: docs.length > 0 ? docs.at(-1).getText(pkg) : null,
          });
        }
      pkgMethods.set(st.name.text, methods);
    }
  }
}

// ── The facade modules ────────────────────────────────────────────────────────
const patches = new Map(); // class name -> { members: string[], chained: Set, declaredIn: Set }
function patchOf(name) {
  if (!patches.has(name))
    patches.set(name, { name, members: [], chained: new Set(), declaredIn: new Set() });
  return patches.get(name);
}

// The JSDoc `block` without its facade tags (each tag runs to the next `@` line
// or the end), or null when no description is left.
function docWithout(block, tags) {
  const lines = block
    .replace(/^\/\*\*|\*\/$/g, '')
    .split('\n')
    .map((l) => l.replace(/^\s*\*\s?/, ''));
  const kept = [];
  let inTag = false;
  for (const line of lines) {
    const tag = /^@(\w+)/.exec(line.trim());
    if (tag) inTag = tags.includes(tag[1]);
    if (!inTag) kept.push(line);
  }
  while (kept.length > 0 && kept.at(-1).trim() === '') kept.pop();
  while (kept.length > 0 && kept[0].trim() === '') kept.shift();
  if (kept.length === 0 || /^@/.test(kept[0].trim())) return null;
  return ['/**', ...kept.map((l) => (l ? ` * ${l}` : ' *')), ' */'].join('\n');
}

const parsed = new Map();
function parseModule(file) {
  if (parsed.has(file)) return parsed.get(file);
  const text = readFileSync(file, 'utf8');
  const relative = path.relative(PACKAGE, file);
  const source = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true, ts.ScriptKind.JS);
  // `{ type, doc }` from the wrapper's `@facadeType` JSDoc, or null.
  const facadeType = (node) => {
    const block = (ts.getLeadingCommentRanges(text, node.getFullStart()) ?? [])
      .map(({ pos, end }) => text.slice(pos, end))
      .filter((c) => c.startsWith('/**') && c.includes('@facadeType'))
      .at(-1);
    if (!block) return null;
    const type = /@facadeType\s+([\s\S]*?)\s*\*\//
      .exec(block)[1]
      .replace(/\n\s*\*\s?/g, ' ')
      .replace(/\s+/g, ' ')
      .trim();
    return { type, doc: docWithout(block, ['facadeType']) };
  };
  const module = {
    file,
    text,
    source,
    imports: new Map(),
    locals: new Map(),
    factories: new Map(),
    exportedFactories: new Map(),
    objects: new Map(),
    exportsByName: new Map(),
    facadeType,
  };
  const visitChains = (node) => {
    // chain(wasm.Class, ['a', 'b'])
    if (
      ts.isCallExpression(node) &&
      ts.isIdentifier(node.expression) &&
      node.expression.text === 'chain'
    ) {
      const [cls, names] = node.arguments;
      if (
        !cls ||
        !ts.isPropertyAccessExpression(cls) ||
        cls.expression.getText(source) !== 'wasm' ||
        !names ||
        !ts.isArrayLiteralExpression(names)
      )
        fail(`${relative}: chain() takes wasm.<Class> and an array literal of names`);
      const patch = patchOf(cls.name.text);
      patch.declaredIn.add(relative);
      for (const el of names.elements) {
        if (!ts.isStringLiteral(el)) fail(`${relative}: chain() names must be string literals`);
        patch.chained.add(el.text);
      }
    }
    ts.forEachChild(node, visitChains);
  };
  visitChains(source);
  for (const range of text.matchAll(/\/\*\*[\s\S]*?\*\//g)) {
    const block = range[0];
    const cls = /@facadeClass\s+(\w+)/.exec(block);
    if (!cls) continue;
    const patch = patchOf(cls[1]);
    patch.declaredIn.add(relative);
    const where = `${relative}: @facadeClass ${cls[1]}`;
    for (const [, tag] of block.matchAll(/@(\w+)/g))
      if (tag === 'omit' || tag === 'static')
        fail(
          `${where}: @${tag} is not supported; mark a removed raw method skip_typescript in Rust`
        );
    const signatures = [...block.matchAll(/@member\s+([\s\S]*?)(?=\n\s*\*\s*@|\s*\*\/$)/g)].map(
      ([, signature]) =>
        signature
          .replace(/\n\s*\*\s?/g, ' ')
          .replace(/\s+/g, ' ')
          .trim()
    );
    if (signatures.length !== 1) fail(`${where}: declare exactly one @member per block`);
    const doc = docWithout(block, ['facadeClass', 'member']);
    if (!doc) fail(`${where}: @member ${signatures[0]} needs a JSDoc description`);
    patch.members.push({ signature: signatures[0], doc });
  }
  for (const st of source.statements) {
    if (
      ts.isImportDeclaration(st) &&
      st.importClause?.namedBindings &&
      ts.isNamedImports(st.importClause.namedBindings)
    ) {
      const from = path.resolve(path.dirname(file), st.moduleSpecifier.text);
      for (const el of st.importClause.namedBindings.elements)
        module.imports.set(el.name.text, { file: from, name: (el.propertyName ?? el.name).text });
    }
    if (ts.isFunctionDeclaration(st) && st.name) {
      module.locals.set(st.name.text, facadeType(st));
      const ret = st.body?.statements.length === 1 ? st.body.statements[0] : null;
      if (
        ret &&
        ts.isReturnStatement(ret) &&
        ret.expression &&
        ts.isObjectLiteralExpression(ret.expression)
      ) {
        module.factories.set(st.name.text, ret.expression);
        if (isExported(st)) module.exportedFactories.set(st.name.text, ret.expression);
      }
    }
    if (ts.isVariableStatement(st))
      for (const d of st.declarationList.declarations) {
        if (!ts.isIdentifier(d.name) || !d.initializer) continue;
        if (ts.isObjectLiteralExpression(d.initializer)) {
          const entry = { module, object: d.initializer, docModule: module, docNode: st };
          module.objects.set(d.name.text, entry);
          if (isExported(st)) module.exportsByName.set(d.name.text, entry);
        } else {
          const factory = resolveFactoryCall(module, d.initializer);
          if (factory) {
            const entry = {
              module: factory.module,
              object: factory.object,
              docModule: module,
              docNode: st,
            };
            module.objects.set(d.name.text, entry);
            if (isExported(st)) module.exportsByName.set(d.name.text, entry);
          } else module.locals.set(d.name.text, facadeType(st));
        }
      }
  }
  parsed.set(file, module);
  return module;
}

// Resolve `factory(...)` when the callee is a helper function returning an
// object literal (such as `domainSchema('core')`).
function resolveFactoryCall(module, expr) {
  if (!ts.isCallExpression(expr) || !ts.isIdentifier(expr.expression)) return null;
  const callee = expr.expression.text;
  if (module.imports.has(callee)) {
    const target = module.imports.get(callee);
    const child = parseModule(target.file);
    const object = child.exportedFactories.get(target.name);
    return object ? { module: child, object } : null;
  }
  if (module.factories.has(callee)) return { module, object: module.factories.get(callee) };
  return null;
}

// ── Namespace tree ────────────────────────────────────────────────────────────
const pascal = (segments) =>
  segments.map((s) => s.replace(/(^|_)([a-z])/g, (_, __, c) => c.toUpperCase())).join('');
const interfaces = []; // { name, doc, members: [{ key, type, doc }] }
// The doc comment on the statement that declares a namespace object (not a
// `@facadeType` / `@facadeClass` block), or null.
function namespaceDoc(module, node) {
  let statement = node;
  while (statement && !ts.isVariableStatement(statement)) statement = statement.parent;
  if (!statement) return null;
  const blocks = (ts.getLeadingCommentRanges(module.text, statement.getFullStart()) ?? [])
    .map(({ pos, end }) => module.text.slice(pos, end))
    .filter((c) => c.startsWith('/**') && !/@facade(Type|Class)/.test(c));
  return blocks.at(-1) ?? null;
}
function collectNamespaceMembers(module, object, segments, members) {
  for (const property of object.properties) {
    if (ts.isSpreadAssignment(property)) {
      const spread = resolveFactoryCall(module, property.expression);
      if (!spread)
        fail(
          `${segments.join('.')}: cannot declare spread \`${property.getText(module.source).slice(0, 60)}\``
        );
      collectNamespaceMembers(spread.module, spread.object, segments, members);
      continue;
    }
    const key = property.name.getText(module.source);
    const where = `${segments.join('.')}.${key}`;
    if (
      ts.isPropertyAssignment(property) &&
      ts.isPropertyAccessExpression(property.initializer) &&
      property.initializer.expression.getText(module.source) === 'wasm'
    ) {
      const raw = property.initializer.name.text;
      if (!pkgNames.has(raw)) fail(`${where}: wasm.${raw} is not exported by the package`);
      members.push({ key, type: `typeof wasm.${raw}`, doc: pkgDocs.get(raw) });
      continue;
    }
    if (
      ts.isPropertyAssignment(property) &&
      (ts.isArrowFunction(property.initializer) || ts.isFunctionExpression(property.initializer))
    ) {
      const wrapper = module.facadeType(property);
      if (!wrapper) fail(`${where}: a wrapper function needs a /** ... @facadeType ... */ comment`);
      if (!wrapper.doc)
        fail(`${where}: the wrapper's @facadeType comment needs a JSDoc description`);
      members.push({ key, type: wrapper.type, doc: wrapper.doc, wrapper: true });
      continue;
    }
    const local = ts.isShorthandPropertyAssignment(property)
      ? key
      : ts.isIdentifier(property.initializer)
        ? property.initializer.text
        : null;
    if (local && module.imports.has(local)) {
      const target = module.imports.get(local);
      const child = parseModule(target.file);
      const childEntry =
        child.exportsByName.get(target.name) ??
        fail(`${where}: ${target.name} is not an exported object in ${target.file}`);
      members.push({ key, type: buildNamespace(childEntry, [...segments, key]) });
    } else if (local && module.objects.has(local)) {
      members.push({
        key,
        type: buildNamespace(module.objects.get(local), [...segments, key]),
      });
    } else if (local && module.locals.has(local)) {
      const wrapper = module.locals.get(local);
      if (!wrapper)
        fail(`${where}: wrapper \`${local}\` needs a /** ... @facadeType ... */ comment`);
      if (!wrapper.doc)
        fail(`${where}: the wrapper's @facadeType comment needs a JSDoc description`);
      members.push({ key, type: wrapper.type, doc: wrapper.doc, wrapper: true });
    } else fail(`${where}: cannot declare \`${property.getText(module.source).slice(0, 60)}\``);
  }
}
function buildNamespace(entry, segments) {
  const name = `${pascal(segments)}Namespace`;
  const members = [];
  collectNamespaceMembers(entry.module, entry.object, segments, members);
  interfaces.push({ name, doc: namespaceDoc(entry.docModule, entry.docNode), members });
  return name;
}

const index = ts.createSourceFile(
  'index.js',
  readFileSync(path.join(PACKAGE, 'index.js'), 'utf8'),
  ts.ScriptTarget.Latest,
  true,
  ts.ScriptKind.JS
);
const roots = [];
for (const st of index.statements) {
  if (!ts.isExportDeclaration(st) || !st.exportClause || !ts.isNamedExports(st.exportClause))
    continue;
  const specifier = st.moduleSpecifier.text;
  if (!specifier.startsWith('./exports/')) continue;
  const module = parseModule(path.resolve(PACKAGE, specifier));
  for (const el of st.exportClause.elements) {
    const exported = el.name.text;
    const entry =
      module.exportsByName.get((el.propertyName ?? el.name).text) ??
      fail(`index.js: ${exported} is not an exported object`);
    roots.push({ exported, iface: buildNamespace(entry, [exported]) });
  }
}

// ── Emit ──────────────────────────────────────────────────────────────────────
// Prototype members, as an augmentation of the package module: names inside it
// resolve against the package's own exports, so they need no import.
const augmentation = [];
for (const patch of [...patches.values()].sort((a, b) => a.name.localeCompare(b.name))) {
  const methods =
    pkgMethods.get(patch.name) ??
    fail(`patched class ${patch.name} is not a class exported by the package`);
  const where = [...patch.declaredIn].sort().join(', ');
  augmentation.push(
    `  /** Members the facade adds to \`${patch.name}\` (${where}). */`,
    `  interface ${patch.name} {`
  );
  const indent = (doc) =>
    doc.split('\n').map((l) => `    ${l.trim().startsWith('*') ? ` ${l.trim()}` : l.trim()}`);
  for (const { signature, doc } of patch.members)
    augmentation.push(...indent(doc), `    ${signature.replace(/;?$/, ';')}`);
  for (const name of [...patch.chained].sort()) {
    const method =
      methods.get(name) ??
      fail(
        `chain(wasm.${patch.name}, ['${name}']): the package declares no method ${patch.name}.${name}`
      );
    if (method.doc) augmentation.push(...indent(method.doc));
    augmentation.push(`    ${name}(${method.params}): this;`);
  }
  augmentation.push('  }');
}

// Hand-written and copied signatures name package types. An augmentation block
// sees only names the package declares itself, not ones it re-exports, so every
// package name a signature uses is imported at the top level, where both the
// wrapper types and the augmentation resolve it.
const used = new Set();
const noteNames = (text) => {
  for (const id of text.match(/(?<![\w.])[A-Za-z_]\w*/g) ?? []) used.add(id);
};
for (const { members } of interfaces) for (const m of members) if (m.wrapper) noteNames(m.type);
for (const line of augmentation) if (!/^\s*(\/\*\*|\*)/.test(line)) noteNames(line);
const imports = [...used].filter((n) => pkgNames.has(n)).sort();

const out = [
  '// Generated by scripts/gen-facade-dts.mjs from index.js and exports/**/*.js; do not edit.',
  '// Every facade member is the wasm-bindgen export it re-exports (`typeof wasm.X`), typed',
  '// by the `#[wasm_bindgen]` attributes in src/api/**; src/prelude.d.ts holds the shared',
  '// host-only types. Hand-written declarations exist only beside the JavaScript the',
  '// facade adds (`@facadeType`, `@facadeClass` in exports/**).',
  '',
  `import type * as wasm from '${PKG_IMPORT}';`,
];
if (used.has('generated'))
  out.push("import type * as generated from './types/generated/index.js';");
if (imports.length > 0) out.push(`import type { ${imports.join(', ')} } from '${PKG_IMPORT}';`);
out.push('', `export { default } from '${PKG_IMPORT}';`, `export type * from '${PKG_IMPORT}';`, '');
if (augmentation.length > 0) out.push(`declare module '${PKG_IMPORT}' {`, ...augmentation, '}', '');
for (const { name, doc, members } of interfaces) {
  if (doc) out.push(...doc.split('\n').map((l) => l.replace(/^\s+\*/, ' *')));
  out.push(`export interface ${name} {`);
  for (const m of members) {
    if (m.doc)
      out.push(
        ...m.doc.split('\n').map((l) => `  ${l.trim().startsWith('*') ? ` ${l.trim()}` : l.trim()}`)
      );
    out.push(`  readonly ${m.key}: ${m.type};`);
  }
  out.push('}', '');
}
for (const { exported, iface } of roots) out.push(`export declare const ${exported}: ${iface};`);
const result = `${out.join('\n')}\n`;

if (process.argv.includes('--check')) {
  let current = '';
  try {
    current = readFileSync(OUTPUT, 'utf8');
  } catch {
    /* missing */
  }
  if (current !== result) {
    console.error('index.d.ts is out of date; run `mise run gen-write`.');
    process.exit(1);
  }
  console.log('index.d.ts is current.');
} else {
  writeFileSync(OUTPUT, result);
  console.log(
    `Wrote index.d.ts: ${interfaces.length} namespaces, ${patches.size} patched classes.`
  );
}

/**
 * The live facade is the surface of record.
 *
 * 1. The runtime surface (namespaces, functions, classes with their static and
 *    prototype members, getters vs methods) is snapshotted in
 *    `facade-surface.json`; any change must be regenerated deliberately with
 *    `UPDATE_FACADE_SURFACE=1 node --test tests/facade/surface.test.mjs`.
 * 2. `index.d.ts`, read through the TypeScript checker, must declare exactly
 *    that surface: the same top-level value exports, namespace keys, class
 *    statics and instance members, with getters declared as properties and
 *    methods as methods.
 */
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';

const facade = await import('../../index.js');
await facade.default({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

const SNAPSHOT = new URL('../../facade-surface.json', import.meta.url);
const DTS = new URL('../../index.d.ts', import.meta.url).pathname;

// wasm-bindgen plumbing and ownership members that are not part of the API.
const skipMember = (name) => name.startsWith('__') || name === 'constructor';

function isClass(value) {
  return typeof value === 'function' && /^class\b/.test(Function.prototype.toString.call(value));
}

function classSurface(cls) {
  const statics = Object.getOwnPropertyNames(cls)
    .filter((name) => !['length', 'name', 'prototype'].includes(name) && !skipMember(name))
    .sort();
  const methods = [];
  const getters = [];
  for (const name of Object.getOwnPropertyNames(cls.prototype)) {
    if (skipMember(name)) continue;
    const descriptor = Object.getOwnPropertyDescriptor(cls.prototype, name);
    (descriptor.get ? getters : methods).push(name);
  }
  return { kind: 'class', statics, methods: methods.sort(), getters: getters.sort() };
}

function runtimeSurface() {
  const out = {};
  const visit = (path, value) => {
    if (isClass(value)) {
      out[path] = classSurface(value);
    } else if (typeof value === 'function') {
      out[path] = { kind: 'function' };
    } else if (value && typeof value === 'object') {
      out[path] = { kind: 'namespace', keys: Object.keys(value).sort() };
      for (const key of Object.keys(value)) visit(`${path}.${key}`, value[key]);
    }
  };
  for (const key of Object.keys(facade).sort()) {
    if (key !== 'default') visit(key, facade[key]);
  }
  return out;
}

function declaredSurface() {
  const program = ts.createProgram([DTS], {
    target: ts.ScriptTarget.ES2020,
    module: ts.ModuleKind.ESNext,
    moduleResolution: ts.ModuleResolutionKind.Bundler,
    strict: true,
    noEmit: true,
  });
  const checker = program.getTypeChecker();
  const source = program.getSourceFile(DTS);
  const moduleSymbol = checker.getSymbolAtLocation(source);
  const out = {};
  const topLevel = [];

  const memberKind = (symbol) => {
    const decl = symbol.valueDeclaration ?? symbol.declarations?.[0];
    if (!decl) return 'method';
    if (ts.isMethodSignature(decl) || ts.isMethodDeclaration(decl)) return 'method';
    return 'getter';
  };

  // A class is declared either as `declare class X` (reached as `typeof X`)
  // or as a constructor interface with `new (...)` signatures or a
  // `prototype` member plus its statics.
  const instanceTypeOf = (type) => {
    const symbol = type.getSymbol();
    if (symbol && symbol.flags & ts.SymbolFlags.Class)
      return checker.getDeclaredTypeOfSymbol(symbol);
    const constructs = type.getConstructSignatures();
    if (constructs.length > 0) return constructs[0].getReturnType();
    if (type.getCallSignatures().length > 0) return null;
    const prototype = checker.getPropertiesOfType(type).find((p) => p.getName() === 'prototype');
    return prototype ? checker.getTypeOfSymbolAtLocation(prototype, source) : null;
  };

  const visitClass = (path, constructorType, instance) => {
    const statics = checker
      .getPropertiesOfType(constructorType)
      .map((p) => p.getName())
      .filter((name) => name !== 'prototype' && !skipMember(name))
      .sort();
    const methods = [];
    const getters = [];
    for (const member of checker.getPropertiesOfType(instance)) {
      const name = member.getName();
      if (skipMember(name) || name.startsWith('__@')) continue;
      (memberKind(member) === 'method' ? methods : getters).push(name);
    }
    out[path] = { kind: 'class', statics, methods: methods.sort(), getters: getters.sort() };
  };

  const visit = (path, type) => {
    const instance = instanceTypeOf(type);
    if (instance) {
      visitClass(path, type, instance);
    } else if (type.getCallSignatures().length > 0) {
      out[path] = { kind: 'function' };
    } else {
      const props = checker.getPropertiesOfType(type);
      out[path] = { kind: 'namespace', keys: props.map((p) => p.getName()).sort() };
      for (const prop of props) {
        visit(`${path}.${prop.getName()}`, checker.getTypeOfSymbolAtLocation(prop, source));
      }
    }
  };

  const typeOnly = (symbol) =>
    (symbol.declarations ?? []).some(
      (decl) => ts.isExportSpecifier(decl) && (decl.isTypeOnly || decl.parent.parent.isTypeOnly)
    );
  for (const exported of checker.getExportsOfModule(moduleSymbol)) {
    const name = exported.getName();
    if (typeOnly(exported)) continue;
    const target =
      exported.flags & ts.SymbolFlags.Alias ? checker.getAliasedSymbol(exported) : exported;
    if (name === 'default' || !(target.flags & ts.SymbolFlags.Value)) continue;
    topLevel.push(name);
    visit(name, checker.getTypeOfSymbolAtLocation(target, source));
  }
  return { surface: out, topLevel: topLevel.sort() };
}

test('the runtime facade matches the checked-in facade-surface.json snapshot', () => {
  const actual = runtimeSurface();
  const text = `${JSON.stringify(actual, null, 2)}\n`;
  if (process.env.UPDATE_FACADE_SURFACE === '1') {
    writeFileSync(SNAPSHOT, text);
  }
  const expected = JSON.parse(readFileSync(SNAPSHOT, 'utf8'));
  assert.deepEqual(
    actual,
    expected,
    'facade surface changed: review the diff and regenerate with ' +
      'UPDATE_FACADE_SURFACE=1 node --test tests/facade/surface.test.mjs'
  );
});

test('index.d.ts declares exactly the runtime facade surface', () => {
  const runtime = runtimeSurface();
  const { surface: declared, topLevel } = declaredSurface();
  assert.deepEqual(
    topLevel,
    Object.keys(facade)
      .filter((key) => key !== 'default')
      .sort(),
    'top-level value exports in index.d.ts must equal index.js exports'
  );
  const problems = [];
  for (const path of new Set([...Object.keys(runtime), ...Object.keys(declared)])) {
    const r = runtime[path];
    const d = declared[path];
    if (!r) problems.push(`${path}: declared in index.d.ts but absent at runtime`);
    else if (!d) problems.push(`${path}: present at runtime but not declared`);
    else if (JSON.stringify(r) !== JSON.stringify(d)) {
      problems.push(`${path}: runtime ${JSON.stringify(r)} vs declared ${JSON.stringify(d)}`);
    }
  }
  assert.deepEqual(problems, []);
});

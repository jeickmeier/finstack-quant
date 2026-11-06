/**
 * The published declarations carry real types.
 *
 * wasm-bindgen declares every `JsValue` parameter and return as `any` unless
 * the Rust export names its TypeScript type with `unchecked_param_type`,
 * `unchecked_optional_param_type` or `unchecked_return_type`. This test fails
 * on any `any` in the public part of `pkg/finstack_quant_wasm.d.ts` (everything
 * before wasm-bindgen's raw `InitOutput` glue) or in the generated facade
 * `index.d.ts`, naming the declaration to fix in `src/api/**`.
 */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import ts from 'typescript';

function anyTypes(file, stopAt) {
  const path = new URL(file, import.meta.url).pathname;
  const sf = ts.createSourceFile(path, readFileSync(path, 'utf8'), ts.ScriptTarget.Latest, true);
  const found = [];
  const owner = (node) => {
    for (let n = node; n; n = n.parent)
      if (
        (ts.isFunctionDeclaration(n) ||
          ts.isMethodDeclaration(n) ||
          ts.isMethodSignature(n) ||
          ts.isPropertyDeclaration(n) ||
          ts.isPropertySignature(n) ||
          ts.isGetAccessor(n) ||
          ts.isConstructorDeclaration(n)) &&
        n.name
      )
        return `${n.parent?.name?.text ? `${n.parent.name.text}.` : ''}${n.name.getText(sf)}`;
    return '?';
  };
  for (const statement of sf.statements) {
    if (stopAt && statement.name?.text === stopAt) break;
    const visit = (node) => {
      if (node.kind === ts.SyntaxKind.AnyKeyword) {
        const { line } = sf.getLineAndCharacterOfPosition(node.getStart(sf));
        found.push(`${file.split('/').at(-1)}:${line + 1} ${owner(node)}`);
      }
      ts.forEachChild(node, visit);
    };
    visit(statement);
  }
  return found;
}

test('the published declarations contain no `any`', () => {
  assert.deepEqual(
    [
      ...anyTypes('../../pkg/finstack_quant_wasm.d.ts', 'InitOutput'),
      ...anyTypes('../../index.d.ts'),
    ],
    [],
    'add `unchecked_param_type` / `unchecked_return_type` to these exports in src/api/**'
  );
});

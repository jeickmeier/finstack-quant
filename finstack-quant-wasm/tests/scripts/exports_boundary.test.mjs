import assert from 'node:assert/strict';
import { test } from 'node:test';
import { checkExportsSource } from '../../scripts/check-exports-boundary.mjs';

const check = (text) => checkExportsSource(text, 'exports/a.js');
const why = '// wasm-bindgen cannot borrow an optional handle.\n';

for (const [source, kind] of [
  ['const x = a * b;', 'operator *'],
  ['const x = a + b;', 'operator +'],
  ['x += a;', 'operator +='],
  ['const x = -a;', 'prefix minus'],
  ['const x = Math.max(a, b);', 'Math.max'],
  ['const x = list.reduce(f, 0);', '.reduce'],
]) {
  test(`flags ${kind} and names the line`, () => {
    const [error, ...rest] = check(`\n${source}\n`);
    assert.deepEqual(rest, []);
    assert.match(error, new RegExp(`^exports/a\\.js:2: ${kind.replace(/[*+.]/g, '\\$&')}`));
  });
}

test('arithmetic inside a commented function still fails', () => {
  assert.match(check(`${why}const f = (a, b) => a * b;`)[0], /operator \*/);
});

test('string concatenation and a plain re-export pass', () => {
  assert.deepEqual(check('const x = "a" + b;\nexport const ns = { f: wasm.f };'), []);
});

for (const [name, source] of [
  ['declaration', 'function f() {}'],
  ['arrow', 'const f = () => 1;'],
  ['function expression', 'wasm.A.prototype.f = function f() {};'],
  ['property arrow', 'export const ns = {\n  f: () => 1,\n};'],
]) {
  test(`${name} needs a comment`, () => {
    assert.match(check(source)[0], /function with no comment saying what wasm-bindgen cannot/);
  });
}

test('a vague comment does not explain a function', () => {
  assert.equal(check('// Makes the argument optional.\nconst f = () => 1;').length, 1);
  assert.equal(check('// wasm-bindgen glue.\nconst f = () => 1;').length, 1);
});

test('a comment explains its statement, its property and nested functions only', () => {
  assert.deepEqual(check(`${why}const f = (a) => (b) => [a].map((c) => c ?? b);`), []);
  assert.deepEqual(check(`export const ns = {\n  ${why}  f: () => 1,\n};`), []);
  assert.equal(check(`${why}const f = () => 1;\nconst g = () => 2;`).length, 1);
});

import { runInNewContext } from "node:vm";
import { expect, it } from "vitest";
import { sourceString } from "../../scripts/source-string.mjs";

it.each([
  "ordinary-item",
  'quotes" and backslash\\',
  "</script><script>alert(1)</script>",
  "line\u2028paragraph\u2029separator",
  "\b\f\n\r\t\0",
  '"}; throw new Error("injected"); //',
])("preserves %j as a script-safe string literal", (value) => {
  const literal = sourceString(value);
  expect(literal).not.toMatch(/[<>\u2028\u2029]/);
  expect(JSON.parse(literal)).toBe(value);
  expect(runInNewContext(literal)).toBe(value);
});

it("rejects non-string inputs", () => {
  expect(() => sourceString(undefined)).toThrow(TypeError);
});

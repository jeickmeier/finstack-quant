import { describe, expect, it } from "vitest";
import {
  checkSources,
  globToRegExp,
  scanSource,
} from "../../scripts/check-no-arithmetic.mjs";

const errorsIn = (text, file = "registry/a.ts") =>
  scanSource(text, file).errors;

describe("check-no-arithmetic", () => {
  it.each([
    ["const x = a * b;", "operator *"],
    ["const x = a / b;", "operator /"],
    ["const x = a - b;", "operator -"],
    ["const x = a + b;", "operator +"],
    ["const x = a % b;", "operator %"],
    ["const x = a ** b;", "operator **"],
    ["x += a;", "operator +="],
    ["x *= a;", "operator *="],
    ["const x = -a;", "prefix minus"],
    ["const x = Math.max(a, b);", "Math.max"],
    ["const x = list.reduce(sum, 0);", ".reduce"],
    ["const x = new Big(a);", "new Big"],
    ["const x = a.times(b);", "Big .times"],
    ["const x = a.abs();", "Big .abs"],
  ])("flags %s and says how to resolve it", (source, kind) => {
    const [error, ...rest] = errorsIn(`\n${source}\n`);
    expect(rest).toEqual([]);
    expect(error).toContain(`registry/a.ts:2: ${kind}`);
    expect(error).toContain("// finstack-arith: <reason>");
    expect(error).toContain("scripts/no-arithmetic-allow.json");
  });

  it.each([
    ['const x = "a" + b;'],
    ["const x = a + `b${c}`;"],
    ['const x = a + "b" + c;'],
    ['const x = a + (b ? "c" : "d");'],
    ['x += "a";'],
    ["const x = list.length - 1;"],
    ["const x = seen.size + 1;"],
    ["const x = -1;"],
    ["const x = -1n;"],
    ["const x = list.map(f);"],
    ["seen.add(a);"],
  ])("does not flag %s", (source) => {
    expect(errorsIn(source)).toEqual([]);
  });

  it("flags a nested count expression on its outer operator", () => {
    expect(errorsIn("const x = row * list.length + column;")[0]).toContain(
      "operator +",
    );
  });

  it("allows a comment with a reason on or directly above the line", () => {
    expect(errorsIn("const x = a * b; // finstack-arith: pixel width")).toEqual(
      [],
    );
    expect(
      errorsIn("// finstack-arith: pixel width\nconst x = a * b - c;"),
    ).toEqual([]);
    expect(
      errorsIn(
        "const x = <p>\n{/* finstack-arith: ordinal */}\n{i + 1}\n</p>;",
        "registry/a.tsx",
      ),
    ).toEqual([]);
  });

  it("rejects a comment two lines above", () => {
    const errors = errorsIn(
      "// finstack-arith: pixel width\n\nconst x = a * b;",
    );
    expect(errors).toHaveLength(2);
    expect(errors[0]).toContain("registry/a.ts:1: finstack-arith comment");
    expect(errors[1]).toContain("registry/a.ts:3: operator *");
  });

  it.each([
    ["const x = a * b; // finstack-arith:"],
    ["// finstack-arith:   \nconst x = a * b;"],
    ["{/* finstack-arith: */}\nconst x = a * b;"],
  ])("fails a comment with no reason: %j", (source) => {
    const errors = errorsIn(source);
    expect(errors[0]).toContain("finstack-arith comment has no reason");
    expect(errors[1]).toContain("operator *");
  });

  it("fails a comment that covers nothing", () => {
    expect(errorsIn("// finstack-arith: stale\nconst x = a;")[0]).toContain(
      "registry/a.ts:1: finstack-arith comment covers no flagged arithmetic",
    );
  });

  it("excludes files matched by an allow-list glob", () => {
    const files = [
      { file: "registry/shared/chart/deep/a.ts", text: "const x = a * b;" },
      { file: "registry/core/b.ts", text: "const x = a * b;" },
    ];
    const errors = checkSources(files, {
      "registry/shared/chart/**": "layout",
    });
    expect(errors).toHaveLength(1);
    expect(errors[0]).toContain("registry/core/b.ts:1");
  });

  it("fails a glob that excludes no flagged arithmetic or has no reason", () => {
    const files = [{ file: "src/a.ts", text: "const x = a;" }];
    expect(checkSources(files, { "src/a.ts": "mechanics" })[0]).toContain(
      'scripts/no-arithmetic-allow.json: "src/a.ts" excludes no flagged arithmetic',
    );
    expect(
      checkSources([{ file: "src/a.ts", text: "const x = a * b;" }], {
        "src/*.ts": " ",
      })[0],
    ).toContain('"src/*.ts" has no reason');
  });

  it("compiles globs literally except for the stars", () => {
    expect(globToRegExp("src/codec.mjs").test("src/codec.mjs")).toBe(true);
    expect(globToRegExp("src/codec.mjs").test("src/codecxmjs")).toBe(false);
    expect(globToRegExp("a/*.ts").test("a/b/c.ts")).toBe(false);
    expect(globToRegExp("a/**").test("a/b/c.ts")).toBe(true);
  });
});

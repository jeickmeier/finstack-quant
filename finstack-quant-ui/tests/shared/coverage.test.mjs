import { describe, it, expect } from "vitest";
import ts from "typescript";
import {
  checkLedger,
  collectPicks,
  importsWasmValue,
  nextLedger,
  ruleExclusion,
  tally,
  unitOf,
} from "../../scripts/coverage.mjs";

const surface = {
  ns: { kind: "namespace" },
  "ns.sub": { kind: "namespace" },
  "ns.sub.f": { kind: "function" },
  "ns.f": { kind: "function" },
  "ns.g": { kind: "function" },
  "ns.h": { kind: "function" },
  "ns.d": { kind: "function" },
  "ns.fJson": { kind: "function" },
  "ns.FooBuilder": { kind: "class" },
  "ns.schema.x": { kind: "function" },
  "ns.C": {
    kind: "class",
    constructor: true,
    statics: ["fromJson"],
    methods: ["free", "price"],
    getters: ["id"],
  },
};
const source = (text) =>
  ts.createSourceFile("t.ts", text, ts.ScriptTarget.Latest, true);
const pick = (key) => ({ file: "s.ts", key });
// A clean baseline the cases perturb: two picked units, one planned, one excluded.
const base = () => ({
  surface,
  ledger: {
    covered: ["ns.C", "ns.f"],
    planned: { "ns.g": "slice-a" },
    excluded: { "ns.d": "schema-form" },
  },
  picks: [pick("ns.f"), pick("ns.C")],
  used: new Set(["ns.f", "ns.C", "ns.fJson", "ns.sub", "ns.FooBuilder"]),
  valueImports: [],
});
const run = (change) => {
  const input = base();
  change(input);
  return checkLedger(input).join("\n");
};

describe("facade coverage units", () => {
  it("merges a wire twin into its typed twin and excludes by rule", () => {
    expect(unitOf(surface, "ns.fJson")).toBe("ns.f");
    expect(unitOf(surface, "ns.f")).toBe("ns.f");
    expect(ruleExclusion(surface, "ns.fJson")).toBe("wire-twin");
    expect(ruleExclusion(surface, "ns.FooBuilder")).toBe("builder");
    expect(ruleExclusion(surface, "ns.schema.x")).toBe("schema-registry");
    expect(ruleExclusion(surface, "ns.f")).toBeNull();
  });
  it("collects Pick leaves over facade namespaces only", () => {
    const picks = collectPicks(
      source(`type N = {
        ns: Pick<typeof ns, "f" | "C"> & { sub: Pick<typeof ns.sub, "f"> };
        other: Pick<typeof local, "x">;
      };`),
      surface,
    );
    expect(picks.map(({ key }) => key)).toEqual(["ns.f", "ns.C", "ns.sub.f"]);
  });
  it.each([
    [`import type { core } from "finstack-quant-wasm";`, false],
    [`import { type core } from "finstack-quant-wasm";`, false],
    [`import schema from "finstack-quant-wasm/contracts/x.json";`, false],
    [`export type { core } from "finstack-quant-wasm";`, false],
    [`import { core } from "finstack-quant-wasm";`, true],
    [`import init from "finstack-quant-wasm";`, true],
    [`import * as wasm from "finstack-quant-wasm";`, true],
    [`import "finstack-quant-wasm";`, true],
    [`export { core } from "finstack-quant-wasm";`, true],
    [`const m = import("finstack-quant-wasm");`, true],
  ])("treats %s as a value import: %s", (text, expected) => {
    expect(importsWasmValue(source(text))).toBe(expected);
  });
  it("moves newly picked planned keys to covered, sorted", () => {
    expect(
      nextLedger(
        {
          covered: ["ns.f"],
          planned: { "ns.h": "b", "ns.g": "a" },
          excluded: { "ns.d": "schema-form" },
        },
        ["ns.g", "ns.C", "ns.f", "ns.f"],
      ),
    ).toEqual({
      covered: ["ns.C", "ns.f", "ns.g"],
      planned: { "ns.h": "b" },
      excluded: { "ns.d": "schema-form" },
    });
  });
  it("counts in-scope units and class members", () => {
    const { rows, classMembers, usedMembers } = tally(
      surface,
      base().ledger,
      new Map([["ns.C", new Set(["fromJson", "free"])]]),
    );
    // ns.f, ns.g, ns.h, ns.C, ns.sub.f; ns.d is listed, the rest are rule exclusions.
    expect(rows).toEqual({ ns: { covered: 2, scope: 5 } });
    expect([usedMembers, classMembers]).toEqual([1, 4]);
  });
});

describe("facade coverage gate", () => {
  it("accepts a consistent ledger", () => {
    expect(checkLedger(base())).toEqual([]);
  });
  it("counts a picked wire twin as its typed twin", () => {
    expect(
      run((input) => {
        input.picks = [pick("ns.fJson"), pick("ns.C")];
      }),
    ).toBe("");
  });
  it.each([
    [
      "namespace-level Pick",
      (input) => input.picks.push(pick("ns.sub")),
      /picks the namespace `ns.sub`/,
    ],
    [
      "picked key absent from the facade",
      (input) => input.picks.push(pick("ns.gone")),
      /`ns.gone`, which is not in .*facade-surface/,
    ],
    [
      "picked key never referenced",
      (input) => input.picks.push(pick("ns.h")),
      /`ns.h` but the worker closure never references it/,
    ],
    [
      "picked key not yet covered",
      (input) => (input.ledger.covered = ["ns.f"]),
      /Picked `ns.C` is not in `covered`/,
    ],
    [
      "covered key no longer picked",
      (input) => (input.picks = [pick("ns.f")]),
      /Covered `ns.C` is no longer picked/,
    ],
    [
      "unsorted covered",
      (input) => (input.ledger.covered = ["ns.f", "ns.C"]),
      /not sorted/,
    ],
    [
      "planned key now picked",
      (input) => (input.ledger.planned = { "ns.f": "slice-a" }),
      /Planned `ns.f` \(slice-a\) is now picked/,
    ],
    [
      "stale planned key",
      (input) => (input.ledger.planned = { "ns.gone": "slice-a" }),
      /Planned `ns.gone` .* not a counted facade function or class/,
    ],
    [
      "planned without a slice id",
      (input) => (input.ledger.planned = { "ns.g": "" }),
      /needs a slice id/,
    ],
    [
      "stale excluded key",
      (input) => (input.ledger.excluded = { "ns.gone": "schema-form" }),
      /Excluded `ns.gone` is not a facade function or class/,
    ],
    [
      "listed key already excluded by rule",
      (input) => (input.ledger.excluded = { "ns.FooBuilder": "schema-form" }),
      /already excluded by the builder rule/,
    ],
    [
      "unknown reason",
      (input) => (input.ledger.excluded = { "ns.d": "too-hard" }),
      /unknown reason `too-hard`/,
    ],
    [
      "picked and listed-excluded",
      (input) => {
        input.picks.push(pick("ns.d"));
        input.used.add("ns.d");
      },
      /picks `ns.d`, listed as excluded \(schema-form\)/,
    ],
    [
      "picked rule-excluded key",
      (input) => input.picks.push(pick("ns.FooBuilder")),
      /excluded by the builder rule/,
    ],
    [
      "value import outside the worker entry",
      (input) => (input.valueImports = ["registry/shared/file/x.ts"]),
      /x.ts value-imports `finstack-quant-wasm`/,
    ],
  ])("fails on a %s", (_, change, message) => {
    expect(run(change)).toMatch(message);
  });
});

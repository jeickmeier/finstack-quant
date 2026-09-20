import { describe, expect, it } from "vitest";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { z } from "zod";
import { instruments } from "../src/generated/instruments";
import fixtures from "../src/generated/fixtures.json";
import roots from "../src/generated/roots.json";
import { converterSchema } from "../src/schema.mjs";

const repo = resolve(import.meta.dirname, "../..");

describe("discovered canonical fixtures", () => {
  for (const entry of instruments) {
    it(`${entry.type}: lazy module is cached and accepts its complete wire example unchanged`, async () => {
      const first = await entry.loader();
      const second = await entry.loader();
      expect(first).toBe(second);
      expect(first.validator).toBe(second.validator);
      expect(
        JSON.parse(first.codec.stringify(first.validator.parse(first.example))),
      ).toEqual(first.example);
      const broken = structuredClone(first.example);
      (broken.instrument as { type: string }).type = "__invalid__";
      expect(first.validator.safeParse(broken).success).toBe(false);
      expect(first.validator.safeParse({}).success).toBe(false);
    });
  }
  it("has one discovered input per instrument and no eager schema or example imports", async () => {
    expect(
      fixtures.filter((fixture) => fixture.kind === "instrument"),
    ).toHaveLength(instruments.length);
    const source = await readFile(
      resolve(repo, "finstack-quant-ui/src/generated/instruments.ts"),
      "utf8",
    );
    expect(source).not.toMatch(/^import /m);
    expect(source).not.toContain("zod");
    expect(source).not.toContain("/schemas/");
    expect(source).not.toContain("/examples/");
    for (const entry of instruments)
      expect(Object.keys(entry).sort()).toEqual([
        "group",
        "loader",
        "title",
        "type",
      ]);
  });
  it("validates calibration inputs separately from any returned final_market", async () => {
    const validators = new Map();
    for (const fixture of fixtures.filter(
      (fixture) => fixture.kind !== "instrument",
    )) {
      if (!validators.has(fixture.schema)) {
        const root = roots.find((root) => root.uri === fixture.schema)!;
        const schema = JSON.parse(
          await readFile(
            resolve(repo, "finstack-quant-ui/src/generated", root.schema),
            "utf8",
          ),
        );
        validators.set(
          fixture.schema,
          z.fromJSONSchema(converterSchema(schema)),
        );
      }
      let value = JSON.parse(
        await readFile(resolve(repo, fixture.source), "utf8"),
      );
      if (typeof fixture.pointer === "string")
        value = fixture.pointer
          .slice(1)
          .split("/")
          .reduce(
            (node, key) =>
              node[key.replaceAll("~1", "/").replaceAll("~0", "~")],
            value,
          );
      else {
        delete value.$schema;
        delete value.final_market;
      }
      expect(validators.get(fixture.schema).parse(value)).toEqual(value);
    }
  });
});

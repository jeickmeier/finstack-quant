import { describe, expect, it } from "vitest";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { z } from "zod";
import { instruments } from "../../src/generated/instruments";
import fixtures from "../../src/generated/fixtures.json";
import roots from "../../src/generated/roots.json";
import checkSuiteSchema from "../../src/generated/schemas/check_suite_spec.json";
import { createWireCodec } from "../../src/codec.mjs";
import {
  converterSchema,
  linkSchema,
  sharedDefsId,
} from "../../src/schema.mjs";

const repo = resolve(import.meta.dirname, "../../..");

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
  it("accepts every embedded Rust example of each non-instrument root unchanged", async () => {
    const generated = resolve(repo, "finstack-quant-ui/src/generated");
    const shared = JSON.parse(
      await readFile(resolve(generated, "defs/shared.json"), "utf8"),
    );
    const withoutExamples: string[] = [];
    for (const root of roots.filter(
      (root) => !root.uri.includes("/instrument/"),
    )) {
      const text = await readFile(resolve(generated, root.schema), "utf8");
      const bundle = JSON.parse(text);
      const schema = text.includes(sharedDefsId)
        ? linkSchema(bundle, shared)
        : bundle;
      if (!schema.examples?.length) {
        withoutExamples.push(root.schema);
        continue;
      }
      const validator = z.fromJSONSchema(converterSchema(schema));
      for (const example of schema.examples)
        expect(validator.parse(example)).toEqual(example);
    }
    // Rust publishes no example for CovenantReport, DatedCovenantReports,
    // the five credit-scoring inputs, ArbitrageReport, MertonModel, CreditAssessment,
    // DcfOptions, DcfSensitivityResult, LboConfig, LboResult, ScenarioSet,
    // ScenarioDiff, SensitivityResult, BridgeChart, ForecastMetrics or
    // VarianceConfig; every other root carries one.
    expect(withoutExamples).toEqual([
      "schemas/covenant_report.json",
      "schemas/dated_covenant_reports.json",
      "schemas/altman_z_double_prime_input.json",
      "schemas/altman_z_prime_input.json",
      "schemas/altman_z_score_input.json",
      "schemas/arbitrage_report.json",
      "schemas/merton_model.json",
      "schemas/ohlson_o_score_input.json",
      "schemas/zmijewski_input.json",
      "schemas/bridge_chart.json",
      "schemas/credit_assessment.json",
      "schemas/dcf_options.json",
      "schemas/dcf_sensitivity_result.json",
      "schemas/forecast_metrics.json",
      "schemas/lbo_config.json",
      "schemas/lbo_result.json",
      "schemas/scenario_diff.json",
      "schemas/scenario_set.json",
      "schemas/sensitivity_result.json",
      "schemas/variance_config.json",
    ]);
  });
  it("accepts the captured native credit assessment under the generated CreditAssessment schema unchanged", async () => {
    const generated = resolve(repo, "finstack-quant-ui/src/generated");
    const schema = JSON.parse(
      await readFile(
        resolve(generated, "schemas/credit_assessment.json"),
        "utf8",
      ),
    );
    const captured = JSON.parse(
      await readFile(
        resolve(
          repo,
          "finstack-quant-ui/src/fixtures/statements/analyst-credit-assessment.json",
        ),
        "utf8",
      ),
    );
    const validator = z.fromJSONSchema(converterSchema(schema));
    expect(validator.parse(captured)).toEqual(captured);
  });
  it("accepts the captured native DCF sensitivity and the DCF options input under their generated schemas unchanged", async () => {
    const generated = resolve(repo, "finstack-quant-ui/src/generated");
    const fixture = async (name: string) =>
      JSON.parse(
        await readFile(
          resolve(repo, "finstack-quant-ui/src/fixtures/statements", name),
          "utf8",
        ),
      );
    const schema = async (name: string) =>
      z.fromJSONSchema(
        converterSchema(
          JSON.parse(await readFile(resolve(generated, name), "utf8")),
        ),
      );
    const sensitivity = await fixture("analyst-dcf-sensitivity.json");
    expect(
      (await schema("schemas/dcf_sensitivity_result.json")).parse(sensitivity),
    ).toEqual(sensitivity);
    const { options } = await fixture("analyst-dcf-assumptions.json");
    expect((await schema("schemas/dcf_options.json")).parse(options)).toEqual(
      options,
    );
  });
  it("accepts the LBO config input and the captured native LBO result under their generated schemas unchanged", async () => {
    const generated = resolve(repo, "finstack-quant-ui/src/generated");
    for (const [schemaName, fixtureName] of [
      ["schemas/lbo_config.json", "analyst-lbo-config.json"],
      ["schemas/lbo_result.json", "analyst-lbo.json"],
    ]) {
      const validator = z.fromJSONSchema(
        converterSchema(
          JSON.parse(await readFile(resolve(generated, schemaName), "utf8")),
        ),
      );
      const fixture = JSON.parse(
        await readFile(
          resolve(
            repo,
            "finstack-quant-ui/src/fixtures/statements",
            fixtureName,
          ),
          "utf8",
        ),
      );
      expect(validator.parse(fixture)).toEqual(fixture);
    }
  });
  it("accepts the captured native sensitivity result under the generated SensitivityResult schema unchanged", async () => {
    const generated = resolve(repo, "finstack-quant-ui/src/generated");
    const validator = z.fromJSONSchema(
      converterSchema(
        JSON.parse(
          await readFile(
            resolve(generated, "schemas/sensitivity_result.json"),
            "utf8",
          ),
        ),
      ),
    );
    const { result } = JSON.parse(
      await readFile(
        resolve(
          repo,
          "finstack-quant-ui/src/fixtures/statements/analyst-sensitivity.json",
        ),
        "utf8",
      ),
    );
    expect(result.scenarios).toHaveLength(4);
    expect(validator.parse(result)).toEqual(result);
  });
  it("accepts the captured native bridge and backtest under their generated schemas unchanged", async () => {
    const generated = resolve(repo, "finstack-quant-ui/src/generated");
    for (const [schemaName, fixtureName] of [
      ["schemas/bridge_chart.json", "analyst-bridge.json"],
      ["schemas/forecast_metrics.json", "analyst-backtest.json"],
    ]) {
      const validator = z.fromJSONSchema(
        converterSchema(
          JSON.parse(await readFile(resolve(generated, schemaName), "utf8")),
        ),
      );
      const fixture = JSON.parse(
        await readFile(
          resolve(
            repo,
            "finstack-quant-ui/src/fixtures/statements",
            fixtureName,
          ),
          "utf8",
        ),
      );
      expect(validator.parse(fixture)).toEqual(fixture);
    }
  });
  it("accepts the Monte Carlo config input and the captured native results under their generated schemas unchanged", async () => {
    const generated = resolve(repo, "finstack-quant-ui/src/generated");
    for (const [schemaName, fixtureName] of [
      ["schemas/monte_carlo_config.json", "analyst-monte-carlo-config.json"],
      ["schemas/monte_carlo_results.json", "analyst-monte-carlo.json"],
    ]) {
      const validator = z.fromJSONSchema(
        converterSchema(
          JSON.parse(await readFile(resolve(generated, schemaName), "utf8")),
        ),
      );
      const fixture = JSON.parse(
        await readFile(
          resolve(
            repo,
            "finstack-quant-ui/src/fixtures/statements",
            fixtureName,
          ),
          "utf8",
        ),
      );
      expect(validator.parse(fixture)).toEqual(fixture);
    }
  });
  it("accepts the captured native covenant series under the generated DatedCovenantReports and CovenantReport schemas unchanged", async () => {
    const generated = resolve(repo, "finstack-quant-ui/src/generated");
    const read = async (name: string) =>
      JSON.parse(await readFile(resolve(generated, name), "utf8"));
    const dated = z.fromJSONSchema(
      converterSchema(await read("schemas/dated_covenant_reports.json")),
    );
    const report = z.fromJSONSchema(
      converterSchema(await read("schemas/covenant_report.json")),
    );
    const series = JSON.parse(
      await readFile(
        resolve(
          repo,
          "finstack-quant-ui/src/fixtures/covenants/analyst-covenant-series.json",
        ),
        "utf8",
      ),
    );
    expect(series).toHaveLength(6);
    for (const row of series) {
      expect(dated.parse(row)).toEqual(row);
      for (const value of Object.values(row.reports))
        expect(report.parse(value)).toEqual(value);
    }
  });
  it("accepts the captured native scenario diffs under the generated ScenarioDiff schema unchanged", async () => {
    const generated = resolve(repo, "finstack-quant-ui/src/generated");
    const schema = JSON.parse(
      await readFile(resolve(generated, "schemas/scenario_diff.json"), "utf8"),
    );
    const diffs = JSON.parse(
      await readFile(
        resolve(
          repo,
          "finstack-quant-ui/src/fixtures/statements/analyst-scenario-diff.json",
        ),
        "utf8",
      ),
    );
    const validator = z.fromJSONSchema(converterSchema(schema));
    expect(diffs).toHaveLength(2);
    for (const diff of diffs) expect(validator.parse(diff)).toEqual(diff);
  });
  it("accepts the native scenario-table fixture grid under the generated ScenarioGrid schema unchanged", async () => {
    const generated = resolve(repo, "finstack-quant-ui/src/generated");
    const schema = JSON.parse(
      await readFile(resolve(generated, "schemas/scenario_grid.json"), "utf8"),
    );
    const cases = JSON.parse(
      await readFile(
        resolve(
          repo,
          "finstack-quant-ui/tests/valuations/scenario-table/cases.json",
        ),
        "utf8",
      ),
    );
    const validator = z.fromJSONSchema(converterSchema(schema));
    expect(validator.parse(cases.grid)).toEqual(cases.grid);
    expect(validator.safeParse({ ...cases.grid, cpr: [0.1] }).success).toBe(
      false,
    );
  });
  it("round-trips the analyst check suite through the generated CheckSuiteSpec codec", async () => {
    const text = await readFile(
      resolve(
        repo,
        "finstack-quant-ui/src/fixtures/statements/analyst-checks.json",
      ),
      "utf8",
    );
    const codec = createWireCodec(checkSuiteSchema);
    expect(JSON.parse(codec.stringify(codec.parse(text)))).toEqual(
      JSON.parse(text),
    );
  });
});

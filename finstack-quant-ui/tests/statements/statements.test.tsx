import { afterAll, beforeAll, expect, it } from "vitest";
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import { startWorker } from "../shared/worker/harness.mjs";
import { unwrap } from "@/workers/finstack-contract";
import { financialModelModule } from "@/components/finstack/statements/components/financial-model-editor/model";
import {
  adaptStatementResult,
  displayStatementCell,
  statementRows,
} from "@/components/finstack/statements/components/statement-grid/projection";
import type { FinancialModelSpecWire } from "@/lib/finstack/generated/types/financial_model_spec";

const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const fixture = readFileSync(
  new URL(
    "../../src/generated/examples/financial_model_spec.json",
    import.meta.url,
  ),
  "utf8",
);
const analystFixture = readFileSync(
  new URL("../../src/fixtures/statements/analyst-model.json", import.meta.url),
  "utf8",
);
const analystChecks = readFileSync(
  new URL("../../src/fixtures/statements/analyst-checks.json", import.meta.url),
  "utf8",
);
const market = readFileSync(
  new URL("../valuations/instruments/pricing-market.json", import.meta.url),
  "utf8",
);
let harness: Awaited<ReturnType<typeof startWorker>>;
beforeAll(async () => {
  harness = await startWorker();
}, 30000);
afterAll(async () => harness?.close());

it("validates and evaluates the canonical model through the real worker", async () => {
  const canonical = unwrap(await harness.proxy.validateStatementModel(fixture));
  expect(canonical).toBe(native.validateFinancialModelJson(fixture));
  expect(unwrap(await harness.proxy.statementNodeIds(canonical))).toEqual(
    native.modelNodeIds(canonical),
  );
  const result = unwrap(
    await harness.proxy.evaluateStatement({ modelJson: canonical }),
  );
  expect(result).toEqual(native.evaluateModel(canonical));
  const checked = adaptStatementResult(result);
  const model = financialModelModule.codec.parse(
    canonical,
  ) as FinancialModelSpecWire;
  const rows = statementRows(model, checked);
  expect(rows[0]?.cells["2025Q1"]?.text).toBe("100000");
  expect(checked.meta).toEqual(result.meta);
});

it("preserves money precision, absent cells, and nonfinite sentinels", async () => {
  const model = financialModelModule.codec.parse(
    fixture,
  ) as FinancialModelSpecWire;
  model.nodes.cash = {
    node_id: "cash",
    node_type: "value",
    value_type: { type: "monetary", currency: "USD" },
    values: { "2025Q1": { amount: "123.456789", currency: "USD" } },
  };
  model.nodes.lagged = {
    node_id: "lagged",
    node_type: "calculated",
    formula_text: "lag(revenue, 1)",
  };
  const modelJson = JSON.stringify(model);
  const result = unwrap(await harness.proxy.evaluateStatement({ modelJson }));
  expect(result).toEqual(native.evaluateModel(modelJson));
  const checked = adaptStatementResult(result);
  const rows = statementRows(model, checked);
  expect(rows.find((row) => row.nodeId === "cash")?.cells["2025Q1"]?.text).toBe(
    "USD 123.456789",
  );
  expect(
    displayStatementCell(
      rows.find((row) => row.nodeId === "cash")?.cells["2025Q1"],
      "millions",
    ),
  ).toBe("USD 0.0m");
  expect(
    rows.find((row) => row.nodeId === "lagged")?.cells["2025Q1"]?.value,
  ).toBe("nan");
  expect(
    rows.find((row) => row.nodeId === "revenue")?.cells["2025Q2"],
  ).toBeUndefined();
});

it("uses exact market and date inputs and rejects incomplete requests", async () => {
  const modelJson = unwrap(await harness.proxy.validateStatementModel(fixture));
  const request = { modelJson, marketJson: market, asOf: "2025-04-02" };
  const actual = unwrap(await harness.proxy.evaluateStatement(request));
  expect(actual).toEqual(
    native.evaluateModelWithMarket(modelJson, market, request.asOf),
  );
  const incomplete = await harness.proxy.evaluateStatement({
    modelJson,
    marketJson: market,
  });
  expect(incomplete.ok).toBe(false);
  if (!incomplete.ok)
    expect(incomplete.error.message).toMatch(/supplied together/);
});

it("routes formula, explanation and check semantics to WASM", async () => {
  const model = financialModelModule.codec.parse(
    fixture,
  ) as FinancialModelSpecWire;
  model.nodes.margin = {
    node_id: "margin",
    node_type: "calculated",
    formula_text: "revenue * 0.2",
  };
  const modelJson = JSON.stringify(model);
  const preview = unwrap(
    await harness.proxy.validateStatementFormula("revenue * 0.2"),
  );
  expect(preview).toBe(native.parseFormulaText("revenue * 0.2"));
  const invalid = await harness.proxy.validateStatementFormula("revenue +");
  expect(invalid.ok).toBe(false);
  const result = unwrap(await harness.proxy.evaluateStatement({ modelJson }));
  const resultsJson = JSON.stringify(result);
  const explanation = {
    modelJson,
    resultsJson,
    nodeId: "margin",
    period: "2025Q1",
  };
  expect(unwrap(await harness.proxy.explainStatement(explanation))).toEqual(
    native.explainFormula(modelJson, resultsJson, "margin", "2025Q1"),
  );
  expect(unwrap(await harness.proxy.traceStatement(modelJson, "margin"))).toBe(
    native.traceDependencies(modelJson, "margin"),
  );
  const configJson = JSON.stringify({
    name: "empty",
    builtin_checks: [],
    formula_checks: [],
  });
  const report = unwrap(
    await harness.proxy.runStatementChecks({
      modelJson,
      resultsJson,
      configJson,
      kind: "suite",
    }),
  );
  expect(report).toEqual(
    native.runChecks(
      modelJson,
      native.validateCheckSuiteSpecJson(configJson),
      resultsJson,
    ),
  );
  expect(report.summary.total_checks).toBe(0);
});

it("evaluates the complete analyst example, returned ratios, checks and supplied roll forwards", async () => {
  const canonical = unwrap(
    await harness.proxy.validateStatementModel(analystFixture),
  );
  const result = unwrap(
    await harness.proxy.evaluateStatement({ modelJson: canonical }),
  );
  expect(result).toEqual(native.evaluateModel(canonical));
  expect(result.meta?.num_nodes).toBe(24);
  expect(result.meta?.num_periods).toBe(8);
  expect(result.nodes.leverage?.["2025Q4"]).toBeDefined();
  const report = unwrap(
    await harness.proxy.runStatementChecks({
      modelJson: canonical,
      resultsJson: JSON.stringify(result),
      configJson: analystChecks,
      kind: "suite",
    }),
  );
  expect(report).toEqual(
    native.runChecks(
      canonical,
      native.validateCheckSuiteSpecJson(analystChecks),
      JSON.stringify(result),
    ),
  );
  for (const periodId of ["2025Q2", "2025Q3", "2025Q4"]) {
    const updated = readFileSync(
      new URL(
        `../../src/fixtures/statements/roll-forward-${periodId}.json`,
        import.meta.url,
      ),
      "utf8",
    );
    const model = JSON.parse(updated) as FinancialModelSpecWire;
    expect(
      model.periods.find((period) => period.id === periodId)?.is_actual,
    ).toBe(true);
    const evaluated = unwrap(
      await harness.proxy.evaluateStatement({ modelJson: updated }),
    );
    expect(evaluated).toEqual(native.evaluateModel(updated));
  }
});

// Captures the analyst model's native evaluation and check report for
// presentation-only gallery examples. Rerun with `mise run ui-fixtures` after
// rebuilding the WASM package.
import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { serializeHost } from "../../src/codec.mjs";
import { emit } from "../../scripts/fixture-output.mjs";

const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const fixtures = new URL("../../src/fixtures/statements/", import.meta.url);
const read = (name) => readFile(new URL(name, fixtures), "utf8");
const write = (name, json) => emit(new URL(name, fixtures), json);

const modelJson = native.validateFinancialModelJson(
  await read("analyst-model.json"),
);
const evaluator = new native.Evaluator();
const resultJson = serializeHost(evaluator.evaluate(modelJson));
evaluator.free();
const reportJson = serializeHost(
  native.runChecks(
    modelJson,
    native.validateCheckSuiteSpecJson(await read("analyst-checks.json")),
    resultJson,
  ),
);
await write("analyst-result.json", resultJson);
await write("analyst-check-report.json", reportJson);
// Native P&L summary TableEnvelope (long layout) over the canonical income
// lines for the four 2025 quarters, presented by the table-envelope item.
await write(
  "analyst-pl-summary.json",
  serializeHost(
    native.plSummaryReport(
      resultJson,
      ["revenue", "cogs", "gross_profit", "opex", "ebitda", "net_income"],
      ["2025Q1", "2025Q2", "2025Q3", "2025Q4"],
    ),
  ),
);
// Native creditAssessment of the analyst result at its last forecast quarter,
// presented by the credit-assessment-report item beside the P&L summary.
await write(
  "analyst-credit-assessment.json",
  serializeHost(native.creditAssessment(resultJson, "2025Q4")),
);
// Native evaluateDcf and dcfSensitivity of the analyst model under the
// illustrative assumptions in analyst-dcf-assumptions.json (the
// dcf-assumptions-form shape; unset optional arguments stay unset).
const dcf = JSON.parse(await read("analyst-dcf-assumptions.json"));
const dcfArguments = [
  modelJson,
  dcf.wacc,
  dcf.terminalValue,
  dcf.ufcfNode,
  dcf.netDebtOverride,
  dcf.options === undefined ? undefined : JSON.stringify(dcf.options),
];
await write(
  "analyst-dcf.json",
  serializeHost(native.evaluateDcf(...dcfArguments)),
);
await write(
  "analyst-dcf-sensitivity.json",
  serializeHost(native.dcfSensitivity(...dcfArguments)),
);
// Native evaluateLbo of the analyst model under the illustrative transaction
// in analyst-lbo-config.json (no check_mappings: the model has no balance
// sheet to map, so checks is null).
await write(
  "analyst-lbo.json",
  serializeHost(
    native.evaluateLbo(modelJson, await read("analyst-lbo-config.json")),
  ),
);
// Native scenarioDiff over the analyst scenario set (base → downside → severe):
// each scenario against its parent, over the P&L lines for the 2025 quarters.
const scenarioSetJson = await read("analyst-scenarios.json");
const scenarioResults = native.evaluateScenarioSet(modelJson, scenarioSetJson);
await write(
  "analyst-scenario-diff.json",
  serializeHost(
    [
      ["base", "downside"],
      ["downside", "severe"],
    ].map(([baseline, comparison]) =>
      native.scenarioDiff(
        scenarioSetJson,
        scenarioResults,
        baseline,
        comparison,
        ["revenue", "cogs", "gross_profit", "opex", "ebitda", "net_income"],
        ["2025Q1", "2025Q2", "2025Q3", "2025Q4"],
      ),
    ),
  ),
);

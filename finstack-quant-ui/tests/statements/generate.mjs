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
// Native sensitivity of the analyst model under analyst-sensitivity-run.json
// (the statement-sensitivity shape): each parameter's base value is the
// evaluated cell, passed with its percentage moves to
// parameterSpecWithPercentages, then runSensitivity and the tornado at the
// target metric and period, as the worker's runSensitivity does.
const evaluated = JSON.parse(resultJson);
const sensitivity = JSON.parse(await read("analyst-sensitivity-run.json"));
const sensitivityResult = native.runSensitivity(
  modelJson,
  JSON.stringify({
    mode: sensitivity.mode,
    parameters: sensitivity.parameters.map((parameter) =>
      native.parameterSpecWithPercentages(
        parameter.nodeId,
        parameter.periodId,
        evaluated.nodes[parameter.nodeId][parameter.periodId],
        parameter.pctRange,
      ),
    ),
    target_metrics: [sensitivity.targetMetric],
  }),
);
await write(
  "analyst-sensitivity.json",
  serializeHost({
    result: sensitivityResult,
    tornado: native.generateTornadoEntries(
      sensitivityResult,
      sensitivity.targetMetric,
      sensitivity.period,
    ),
  }),
);
// Native goalSeek under analyst-goal-seek-run.json, captured without the
// updated model (updateModel false) to keep the fixture small.
const seek = JSON.parse(await read("analyst-goal-seek-run.json"));
await write(
  "analyst-goal-seek.json",
  serializeHost(
    native.goalSeek(
      modelJson,
      seek.targetNode,
      seek.targetPeriod,
      seek.targetValue,
      seek.driverNode,
      seek.driverPeriod,
      false,
      seek.bounds,
    ),
  ),
);
// Since the last roll forward: the analyst model (actuals to 2025Q1) against
// analyst-reported-2025Q4.json, an illustrative host update reporting
// 2025Q2-2025Q4 with revenue, opex and D&A away from the forecast. The
// compared periods are the three it reported, as the workbench chooses them.
const reportedEvaluator = new native.Evaluator();
const reportedJson = serializeHost(
  reportedEvaluator.evaluate(
    native.validateFinancialModelJson(
      await read("analyst-reported-2025Q4.json"),
    ),
  ),
);
reportedEvaluator.free();
const rolled = ["2025Q2", "2025Q3", "2025Q4"];
const rollLabels = ["Before roll forward", "After roll forward"];
await write(
  "analyst-variance.json",
  serializeHost(
    native.runVariance(
      resultJson,
      reportedJson,
      JSON.stringify({
        baseline_label: rollLabels[0],
        comparison_label: rollLabels[1],
        metrics: ["revenue", "opex", "ebitda", "leverage"],
        periods: rolled,
      }),
    ),
  ),
);
await write(
  "analyst-bridge.json",
  serializeHost(
    native.varianceBridge(
      resultJson,
      reportedJson,
      "ebitda",
      "2025Q2",
      ["revenue", "cogs", "opex"],
      ...rollLabels,
    ),
  ),
);
const before = JSON.parse(resultJson).nodes.revenue;
const after = JSON.parse(reportedJson).nodes.revenue;
await write(
  "analyst-backtest.json",
  serializeHost(
    native.backtestForecast(
      rolled.map((period) => after[period]),
      rolled.map((period) => before[period]),
    ),
  ),
);
// Native Monte Carlo of analyst-model-stochastic.json (the analyst model with
// a log-normal revenue forecast) under analyst-monte-carlo-config.json,
// captured without the path table. The breach probability of leverage above
// 2.25x needs the paths, so it comes from the same config with
// include_path_data and only the returned fraction is kept (the 200-path
// table is about 750 KB).
const stochasticJson = native.validateFinancialModelJson(
  await read("analyst-model-stochastic.json"),
);
const monteCarloConfig = JSON.parse(
  await read("analyst-monte-carlo-config.json"),
);
const monteCarlo = new native.Evaluator();
await write(
  "analyst-monte-carlo.json",
  serializeHost(
    monteCarlo.evaluateMonteCarlo(
      stochasticJson,
      JSON.stringify(monteCarloConfig),
    ),
  ),
);
const withPaths = monteCarlo.evaluateMonteCarlo(
  stochasticJson,
  JSON.stringify({ ...monteCarloConfig, include_path_data: true }),
);
monteCarlo.free();
const breach = { metric: "leverage", threshold: 2.25 };
await write(
  "analyst-monte-carlo-breach.json",
  serializeHost({
    ...breach,
    probability:
      native.monteCarloBreachProbability(
        serializeHost(withPaths),
        breach.metric,
        breach.threshold,
      ) ?? null,
  }),
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

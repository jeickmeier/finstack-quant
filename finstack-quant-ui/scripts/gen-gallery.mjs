import { access, readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { loadRegistry } from "shadcn/registry";
import { checkGraph, visualTypes, workerRoles } from "./check-registry.mjs";
import { emit, emitSource, sha256 } from "./fixture-output.mjs";
import { sourceString } from "./source-string.mjs";
const root = fileURLToPath(new URL("../", import.meta.url)),
  repo = path.resolve(root, "..");
const output = path.join(repo, "docs-site/src/components/registry-gallery");
const registry = await loadRegistry({ cwd: root });
// The gallery wraps an item in FinstackQueryProvider exactly when its role is `host`.
const roles = await workerRoles(
  root,
  registry.items,
  checkGraph(registry.items),
);
const visual = registry.items
  .filter((item) => visualTypes.has(item.type))
  .map((item) => ({
    name: item.name,
    type: item.type,
    docs: item.docs,
    worker: roles.get(item.name),
  }));
const nonvisual = registry.items.filter((item) => !visualTypes.has(item.type));
const sources = {
  bond: "src/fixtures/results/bond.json",
  market: "tests/core/market-browser/cases.json",
  curveQuery: "tests/core/curve-query/cases.json",
  cubes: "tests/core/cubes/cases.json",
  calibration: "tests/calibration/cases.json",
  scenarios: "tests/valuations/scenario-table/cases.json",
  trancheAnalytics: "tests/valuations/scenario-table/tranche-analytics.json",
  poolReport: "tests/valuations/scenario-table/pool-report.json",
  waterfallReport: "tests/valuations/scenario-table/waterfall-report.json",
  dealCharts: "tests/valuations/scenario-table/deal-charts.json",
  equityTranche: "tests/valuations/scenario-table/equity-tranche.json",
  cashflows: "tests/valuations/cashflows/cases.json",
  loanYield: "tests/valuations/loan-yield/cases.json",
  bondCredit: "tests/valuations/bond-credit/cases.json",
  convertibleCard: "tests/valuations/convertible/cases.json",
  workoutLadder: "tests/valuations/workout-ladder/cases.json",
  quoteConverter: "tests/valuations/quote-converter/cases.json",
  marketScenario: "tests/scenarios/application/cases.json",
  horizonReturn: "tests/scenarios/horizon/cases.json",
  scenarioTemplates: "tests/scenarios/templates/cases.json",
  pnlAttribution: "tests/attribution/cases.json",
  performance: "tests/analytics/explicit.json",
  benchmark: "tests/analytics/benchmark.json",
  correlation: "tests/analytics/correlation/cases.json",
  credit: "tests/models/credit/migration.json",
  merton: "tests/models/credit/merton.json",
  scorecard: "tests/models/credit/scorecard.json",
  positionRisk: "tests/models/risk/cases.json",
  options: "tests/models/options/cases.json",
  liabilityManagement: "tests/models/liability-management/cases.json",
  recovery: "tests/models/recovery/cases.json",
  simm: "src/fixtures/margin/simm.json",
  vm: "src/fixtures/margin/vm.json",
  portfolioValuation: "tests/portfolio/gallery.json",
  scenarioPnl: "tests/portfolio/scenario-pnl.json",
  portfolioMargin: "tests/portfolio/margin/cases.json",
  portfolioWorkbench: "tests/portfolio/workbench/cases.json",
  portfolioExposures: "tests/portfolio/exposures/cases.json",
  portfolioAttribution: "tests/portfolio/attribution/cases.json",
  portfolioReturns: "tests/portfolio/returns/cases.json",
  portfolioBrinson: "tests/portfolio/brinson/cases.json",
  details: "tests/valuations/details/cases.json",
  irsInstrument:
    "../finstack-quant/valuations/tests/instruments/json_examples/interest_rate_swap.json",
  pricingMarket: "tests/valuations/instruments/pricing-market.json",
  statementModel: "src/fixtures/statements/analyst-model.json",
  statementChecks: "src/fixtures/statements/analyst-checks.json",
  statementResult: "src/fixtures/statements/analyst-result.json",
  statementCheckReport: "src/fixtures/statements/analyst-check-report.json",
  statementPlSummary: "src/fixtures/statements/analyst-pl-summary.json",
  statementCreditAssessment:
    "src/fixtures/statements/analyst-credit-assessment.json",
  statementDcfAssumptions:
    "src/fixtures/statements/analyst-dcf-assumptions.json",
  statementDcf: "src/fixtures/statements/analyst-dcf.json",
  statementDcfSensitivity:
    "src/fixtures/statements/analyst-dcf-sensitivity.json",
  statementLboConfig: "src/fixtures/statements/analyst-lbo-config.json",
  statementLbo: "src/fixtures/statements/analyst-lbo.json",
  statementSensitivityRun:
    "src/fixtures/statements/analyst-sensitivity-run.json",
  statementSensitivity: "src/fixtures/statements/analyst-sensitivity.json",
  statementGoalSeekRun: "src/fixtures/statements/analyst-goal-seek-run.json",
  statementGoalSeek: "src/fixtures/statements/analyst-goal-seek.json",
  statementBridge: "src/fixtures/statements/analyst-bridge.json",
  statementBacktest: "src/fixtures/statements/analyst-backtest.json",
  statementMonteCarloConfig:
    "src/fixtures/statements/analyst-monte-carlo-config.json",
  statementMonteCarlo: "src/fixtures/statements/analyst-monte-carlo.json",
  statementMonteCarloBreach:
    "src/fixtures/statements/analyst-monte-carlo-breach.json",
  statementScenarios: "src/fixtures/statements/analyst-scenarios.json",
  statementScenarioDiff: "src/fixtures/statements/analyst-scenario-diff.json",
  statementRollForward2025Q2:
    "src/fixtures/statements/roll-forward-2025Q2.json",
  statementRollForward2025Q3:
    "src/fixtures/statements/roll-forward-2025Q3.json",
  statementRollForward2025Q4:
    "src/fixtures/statements/roll-forward-2025Q4.json",
  waccCalculator: "tests/statements/wacc/cases.json",
  normalizationEditor: "tests/statements/normalization/cases.json",
  capitalStructureReport: "tests/statements/capital-structure/cases.json",
  relativeValueTable: "tests/statements/relative-value/cases.json",
  statementExport: "tests/statements/export/cases.json",
  covenantSeries: "src/fixtures/covenants/analyst-covenant-series.json",
  covenantEngine: "src/fixtures/covenants/analyst-covenants.json",
  covenantForecastConfig: "src/fixtures/covenants/analyst-forecast-config.json",
  covenantBreachForecast: "src/fixtures/covenants/analyst-breach-forecast.json",
};
const unwritten = [];
for (const { name } of visual)
  await access(path.join(output, "examples", `${name}.tsx`)).catch(() =>
    unwritten.push(name),
  );
if (unwritten.length)
  throw new Error(
    `Visual items without a gallery example (examples/<name>.tsx): ${unwritten}`,
  );
const data = {},
  provenance = [];
for (const [name, relative] of Object.entries(sources)) {
  const bytes = await readFile(path.join(root, relative));
  data[name] = JSON.parse(bytes);
  provenance.push({
    path: path.relative(repo, path.join(root, relative)),
    sha256: sha256(bytes),
  });
}
const pricingCases = JSON.parse(
  await readFile(
    path.join(root, "tests/valuations/instruments/pricing-cases.json"),
    "utf8",
  ),
).cases;
const catalogue = JSON.parse(
  await readFile(path.join(root, "src/generated/catalogue.json"), "utf8"),
);
const workbenchGroups = new Set(["credit", "rates", "fixed_income"]);
const workbenchTypes = new Set(
  catalogue
    .filter((entry) => workbenchGroups.has(entry.group))
    .map((entry) => entry.type),
);
async function readSource(source) {
  const bytes = await readFile(path.join(repo, source.path));
  return structuredClone(
    source.pointer.reduce((value, key) => value[key], JSON.parse(bytes)),
  );
}
function applyPatches(value, patches) {
  for (const patch of patches) {
    const parent = patch.path
      .slice(0, -1)
      .reduce((node, key) => node[key], value);
    parent[patch.path.at(-1)] = structuredClone(patch.value);
  }
}
data.pricingExamples = {};
for (const entry of pricingCases.filter((entry) =>
  workbenchTypes.has(entry.type),
)) {
  const instrument = await readSource(entry.instrumentSource);
  applyPatches(instrument, entry.instrumentPatches);
  const market = await readSource(entry.marketSource);
  applyPatches(market, entry.marketPatches);
  const sharedMarket =
    entry.marketSource.path ===
      "finstack-quant-ui/tests/valuations/instruments/pricing-market.json" &&
    entry.marketPatches.length === 0;
  data.pricingExamples[entry.type] = {
    instrument,
    ...(sharedMarket ? {} : { market }),
    request: entry.request,
  };
  workbenchTypes.delete(entry.type);
}
if (workbenchTypes.size)
  throw new Error(`Missing workbench pricing cases: ${[...workbenchTypes]}`);
const imports = nonvisual.map((item) => {
  const targets = (item.files ?? [])
    .map(({ target }) => (target.startsWith("~/") ? target.slice(2) : target))
    .filter(
      (target) =>
        /\.(?:[cm]?js|tsx?|json)$/.test(target) &&
        !target.endsWith(".d.ts") &&
        !target.endsWith(".worker.ts"),
    )
    .map((target) => {
      const extension = path.extname(target);
      return extension === ".ts" || extension === ".tsx"
        ? target.slice(0, -extension.length)
        : target;
    });
  return `${sourceString(item.name)}:()=>Promise.all([${targets.map((target) => `import(${sourceString("@/" + target)})`).join(",")}]).then(modules=>({name:${sourceString(item.name)},modules:modules.length}))`;
});
const exampleId = (name) =>
  name.replace(/(?:^|-)(\w)/g, (_, letter) => letter.toUpperCase());
const components = `"use client";
// Generated by finstack-quant-ui/scripts/gen-gallery.mjs: the example of every visual item, examples/<name>.tsx.
import type { ExampleProps } from "./examples/props";
${visual.map(({ name }) => `import { Example as ${exampleId(name)} } from "./examples/${name}";`).join("\n")}
const examples = {
${visual.map(({ name }) => `  ${JSON.stringify(name)}: ${exampleId(name)},`).join("\n")}
};
export function ComponentDemo({
  name,
  ...props
}: ExampleProps & { name: string }) {
  const Example = examples[name as keyof typeof examples];
  if (!Example) throw new Error(\`Missing gallery example: \${name}\`);
  return <Example {...props} />;
}
`;
await emit(
  path.join(output, "inventory.json"),
  JSON.stringify({
    visual,
    nonvisual: nonvisual.map(({ name, type }) => ({ name, type })),
    provenance,
  }),
);
await emit(path.join(output, "data.json"), JSON.stringify(data));
await emitSource(
  path.join(output, "imports.ts"),
  `// Generated consumer import harness. Worker entry executes only through FinstackProvider.\nexport const importHarnesses={${imports.join(",\n")}};\n`,
);
await emitSource(path.join(output, "components.tsx"), components);
console.log(
  `Gallery: ${visual.length} visual and ${nonvisual.length} nonvisual items`,
);

import { checking, emit } from "../../../scripts/fixture-output.mjs";
import { native, readRepoSource } from "../../../scripts/fixture-inputs.mjs";
// The priced bond of the results fixture: a USD-OIS discount curve and an HZ hazard curve.
const bondPath = "finstack-quant-ui/src/fixtures/results/bond.json";
const bondInput = await readRepoSource(bondPath);
const base = bondInput.value.request;
const templateId = "rate_shock_2022";
const ratesId = "rate_shock_2022_rates";
const equityId = "rate_shock_2022_equity";
// The worker's ScenariosApi calls, as the builder makes them.
const metadata = native.listBuiltinTemplateMetadata();
const template = native.buildFromTemplate(templateId);
const rates = native.buildTemplateComponent(templateId, ratesId);
const equity = native.buildTemplateComponent(templateId, equityId);
// The user maps the placeholder rates curve to the market's discount curve.
const remapped = {
  ...rates,
  operations: rates.operations.map((operation) => ({
    ...operation,
    curve_id: "USD-OIS",
  })),
};
const composed = native.composeScenarios([remapped, equity]);
function apply(name, spec) {
  const scenarioJson = JSON.stringify(
    native.parseScenarioSpec(JSON.stringify(spec)),
  );
  try {
    const applied = native.applyScenario(
      scenarioJson,
      base.marketJson,
      base.asOf,
      undefined,
      native.scenarioSpecRequiresInstruments(scenarioJson)
        ? `[${base.instrumentJson}]`
        : undefined,
    );
    return { name, scenarioJson, report: applied.report };
  } catch (error) {
    return {
      name,
      scenarioJson,
      error: { name: error.name, kind: error.kind, message: error.message },
    };
  }
}
const applications = [
  // Template ids are placeholders: USD-SOFR is not in the bond market.
  apply("rates-as-built", rates),
  apply("whole-template", template),
  // Remapped rates plus equity: the curve shift applies, the missing equities warn.
  apply("rates-remapped-with-equity", composed),
];
await emit(
  new URL("cases.json", import.meta.url),
  JSON.stringify({
    description:
      "Native built-in scenario templates: the template metadata list, the rate_shock_2022 template and its rates and equity components, the rates component remapped from USD-SOFR to USD-OIS composed with the equity component, and each applied to the results-fixture bond market at its valuation date (the native error, or the report with its warnings).",
    sources: [bondInput.source],
    asOf: base.asOf,
    curveIds: ["USD-OIS", "HZ"],
    templateId,
    componentIds: { rates: ratesId, equity: equityId },
    metadata,
    template,
    rates,
    equity,
    remapped,
    composed,
    applications,
  }),
);
console.log(
  `${checking ? "Checked" : "Generated"} ${metadata.length} scenario templates and ${applications.length} applications.`,
);

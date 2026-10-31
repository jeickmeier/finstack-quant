import type { attribution, scenarios } from "finstack-quant-wasm";
import type { ScenariosApi } from "../finstack-contract";
import type { ServiceContext } from "./context";
/** scenarios facade members this slice reads. */
export type ScenariosNative = {
  scenarios: Pick<
    typeof scenarios,
    | "parallelBpMany"
    | "operationSpecTimeRollForward"
    | "parseScenarioSpec"
    | "scenarioSpecRequiresInstruments"
    | "scenarioSpecMutatesInstruments"
    | "applyScenario"
    | "applicationReportEffectiveAsOf"
    | "computeHorizonReturn"
    | "horizonResultExplainText"
    | "listBuiltinTemplateMetadata"
    | "buildFromTemplate"
    | "buildTemplateComponent"
    | "composeScenarios"
  >;
  attribution: Pick<typeof attribution, "pnlAttributionPctOfTotalByBucket">;
};
/** ScenariosApi over the scenarios facade namespace and the shared market cache. */
export function createScenariosService(
  native: ScenariosNative,
  context: ServiceContext,
): ScenariosApi {
  const api = native.scenarios;
  return {
    buildMarketScenario(request) {
      return context.result(() => {
        // Omitted options stay unset so Rust applies its own defaults.
        const operations = [
          ...(request.roll
            ? [
                api.operationSpecTimeRollForward(
                  request.roll.period,
                  undefined,
                  request.roll.rollMode ?? undefined,
                ),
              ]
            : []),
          ...request.shifts.flatMap((shift) =>
            api.parallelBpMany(shift.curveKind, [...shift.curveIds], shift.bp),
          ),
        ];
        const spec = api.parseScenarioSpec(
          JSON.stringify({
            id: request.id,
            operations,
            ...(request.hazardBumpMode
              ? { hazard_bump_mode: request.hazardBumpMode }
              : {}),
          }),
        );
        return JSON.stringify(spec);
      });
    },
    applyMarketScenario(request) {
      return context.result(() => {
        const spec = request.scenarioJson;
        // finstack-compose: Rust's predicate decides; only the request's instrument is routed
        const inventory = api.scenarioSpecRequiresInstruments(spec)
          ? `[${request.instrumentJson}]`
          : undefined;
        const applied = api.applyScenario(
          spec,
          request.marketJson,
          request.asOf,
          undefined,
          inventory,
        );
        let instrumentJson = request.instrumentJson;
        // finstack-compose: Rust's predicate decides; a mutating scenario requires instruments, so Rust returned the copy
        if (api.scenarioSpecMutatesInstruments(spec))
          instrumentJson = JSON.stringify(applied.instruments![0]);
        return {
          report: applied.report,
          marketJson: context.market(JSON.stringify(applied.market)).json,
          asOf: api.applicationReportEffectiveAsOf(
            applied.report,
            request.asOf,
          ),
          instrumentJson,
        };
      });
    },
    horizonReturn(request) {
      return context.result(() => {
        // Omitted method, config and calendar stay unset so Rust applies its defaults.
        const report = api.computeHorizonReturn(
          request.instrumentJson,
          request.marketJson,
          request.asOf,
          request.scenarioJson,
          request.method ?? undefined,
          undefined,
          request.calendarId ?? undefined,
        );
        return {
          report,
          explain: api.horizonResultExplainText(report),
          attributionBuckets:
            native.attribution.pnlAttributionPctOfTotalByBucket(
              report.attribution,
            ),
        };
      });
    },
    validateScenarioSpec(json) {
      return context.result(() => JSON.stringify(api.parseScenarioSpec(json)));
    },
    listBuiltinTemplateMetadata() {
      return context.result(() => api.listBuiltinTemplateMetadata());
    },
    buildFromTemplate(templateId) {
      return context.result(() => api.buildFromTemplate(templateId));
    },
    buildTemplateComponent(request) {
      return context.result(() =>
        api.buildTemplateComponent(request.templateId, request.componentId),
      );
    },
    composeScenarios(specJsons) {
      // Each text is parsed for transport only; Rust validates and composes.
      return context.result(() =>
        api.composeScenarios(specJsons.map((json) => JSON.parse(json))),
      );
    },
  };
}

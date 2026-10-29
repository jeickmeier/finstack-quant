"use client";
import type { ExampleProps } from "./props";
import { MarketScenarioBuilder } from "@/components/finstack/scenarios/components/market-scenario-builder/market-scenario-builder";
import { useScenarioSpecValidator } from "@/hooks/scenarios/use-scenario-spec-validator/use-scenario-spec-validator";
import { useListBuiltinTemplateMetadata } from "@/hooks/scenarios/use-list-builtin-template-metadata/use-list-builtin-template-metadata";
import { useBuildFromTemplate } from "@/hooks/scenarios/use-build-from-template/use-build-from-template";
import { useBuildTemplateComponent } from "@/hooks/scenarios/use-build-template-component/use-build-template-component";
import { useComposeScenarios } from "@/hooks/scenarios/use-compose-scenarios/use-compose-scenarios";
import { useBuildMarketScenario } from "@/hooks/scenarios/use-market-scenario/use-market-scenario";
import { WorkerPreview } from "../worker-preview";
import data from "../data.json";
// The rate_shock_2022 rates component remapped to USD-OIS and composed with its
// equity component, as Rust returned it.
const cases = data.scenarioTemplates;
const composed = cases.applications.find(
  (entry) => entry.name === "rates-remapped-with-equity",
)!.scenarioJson;
// The hooks sit outside the item's install closure, so the example mounts its
// own provider; the install matrix runs the item with caller stubs.
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <WorkerPreview>
      <Builder density={density} />
    </WorkerPreview>
  );
}
function Builder({ density }: ExampleProps) {
  const templates = useListBuiltinTemplateMetadata();
  return (
    <MarketScenarioBuilder
      defaultJson={composed}
      validate={useScenarioSpecValidator()}
      curveIds={cases.curveIds}
      templates={templates.data}
      templatesError={templates.error?.message}
      buildFromTemplate={useBuildFromTemplate()}
      buildTemplateComponent={useBuildTemplateComponent()}
      composeScenarios={useComposeScenarios()}
      buildMarketScenario={useBuildMarketScenario()}
      density={density}
    />
  );
}

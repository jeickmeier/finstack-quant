"use client";
import { useState } from "react";
import type { ScenarioSpec, TemplateMetadata } from "finstack-quant-wasm";
import { MarketScenarioBuilder } from "@/components/finstack/scenarios/components/market-scenario-builder/market-scenario-builder";

import data from "./data.json";
// The caller's validation returns the text unchanged and the template loaders
// return the supplied native rate_shock_2022 outputs: the item installs
// without a worker.
const cases = data.scenarioTemplates;
const validate = async (json: string) => json;
const specs: Record<string, unknown> = {
  [cases.templateId]: cases.template,
  [cases.componentIds.rates]: cases.rates,
  [cases.componentIds.equity]: cases.equity,
};
export function InstalledItem() {
  const [spec, setSpec] = useState<string | null>(null);
  return (
    <>
      <p>
        Supplied valid fixture; standalone scenario editor with caller
        validation and template callbacks.
      </p>
      <MarketScenarioBuilder
        defaultJson={JSON.stringify(cases.composed)}
        validate={validate}
        onSpecChange={setSpec}
        curveIds={cases.curveIds}
        templates={cases.metadata as TemplateMetadata[]}
        buildFromTemplate={async (id) => specs[id] as ScenarioSpec}
        buildTemplateComponent={async ({ componentId }) =>
          specs[componentId] as ScenarioSpec
        }
      />
      <output aria-label="Canonical spec">
        {spec === null ? "none" : "ready"}
      </output>
    </>
  );
}

"use client";
import { useState } from "react";
import type { ExampleProps } from "./props";
import { CapitalStructureReport } from "@/components/finstack/statements/components/capital-structure-report/capital-structure-report";
import { useCapitalStructureSummary } from "@/hooks/statements/use-capital-structure-summary/use-capital-structure-summary";
import data from "../data.json";
import { WorkerPreview } from "../worker-preview";
// A quarterly model with two first-lien bonds and a second-lien bond, evaluated natively.
const fixture = data.capitalStructureReport;
const resultJson = JSON.stringify(fixture.result);
const modelJson = JSON.stringify(fixture.model);
const nodeIds = Object.keys(fixture.model.nodes);
const periodIds = fixture.model.periods.map((period) => period.id);
const start = fixture.cases.full.config;
function Live({ density }: ExampleProps) {
  const [configJson, setConfigJson] = useState(() => JSON.stringify(start));
  const summary = useCapitalStructureSummary({
    resultJson,
    modelJson,
    configJson,
  });
  return (
    <CapitalStructureReport
      defaultConfig={start}
      nodeIds={nodeIds}
      periodIds={periodIds}
      onSubmit={setConfigJson}
      summary={summary.data}
      loading={summary.isFetching}
      error={summary.error?.message}
      density={density}
    />
  );
}
// The hook sits outside the item's install closure, so the example mounts its
// own provider; the install matrix runs the item with a supplied summary.
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <WorkerPreview>
      <Live density={density} />
    </WorkerPreview>
  );
}

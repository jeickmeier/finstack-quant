"use client";
import { CapitalStructureReport } from "@/components/finstack/statements/components/capital-structure-report/capital-structure-report";
import type { CapitalStructureSummary } from "@/workers/finstack-contract";

import data from "./data.json";
// The summary is the supplied native fixture: the item installs without a
// worker, so nothing is summarized here.
const { model, cases } = data.capitalStructureReport;
export function InstalledItem() {
  return (
    <>
      <p>Supplied valid fixture; standalone capital structure report.</p>
      <CapitalStructureReport
        defaultConfig={cases.full.config}
        nodeIds={Object.keys(model.nodes)}
        periodIds={model.periods.map((period) => period.id)}
        onSubmit={() => {}}
        summary={cases.full.summary as unknown as CapitalStructureSummary}
      />
    </>
  );
}

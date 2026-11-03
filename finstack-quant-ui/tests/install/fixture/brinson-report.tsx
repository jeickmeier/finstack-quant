"use client";
import {
  BrinsonReport,
  type CarinoLinkedAttribution,
} from "@/components/finstack/portfolio/components/brinson-report/brinson-report";

import data from "./data.json";
// The attribution is the supplied native fixture: the item installs without a worker.
const { inputs, result } = data.portfolioBrinson;
export function InstalledItem() {
  return (
    <>
      <p>Supplied valid fixture; standalone Brinson report.</p>
      <BrinsonReport
        report={result as CarinoLinkedAttribution}
        periodLabels={inputs.periodLabels}
      />
    </>
  );
}

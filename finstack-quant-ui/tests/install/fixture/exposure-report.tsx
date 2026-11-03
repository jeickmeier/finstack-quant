"use client";
import {
  ExposureReport,
  type PortfolioPrimitiveExposureReport,
  type SensitivityMatrixJson,
} from "@/components/finstack/portfolio/components/exposure-report/exposure-report";

import data from "./data.json";
// The report and matrix are the supplied native fixture: the item installs without a worker.
const { report, sensitivities } = data.portfolioExposures;
export function InstalledItem() {
  return (
    <>
      <p>Supplied valid fixture; standalone exposure report.</p>
      <ExposureReport
        report={report as PortfolioPrimitiveExposureReport}
        sensitivities={sensitivities as SensitivityMatrixJson}
      />
    </>
  );
}

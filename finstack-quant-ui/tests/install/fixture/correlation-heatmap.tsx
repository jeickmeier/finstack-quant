"use client";
import {
  CorrelationHeatmap,
  type CorrelationMatrixData,
} from "@/components/finstack/analytics/components/correlation-heatmap/correlation-heatmap";

import data from "./data.json";
// The matrix is the supplied native fixture: the item installs without a worker.
const { result } = data.correlation;
export function InstalledItem() {
  return (
    <>
      <p>Supplied valid fixture; standalone correlation heatmap.</p>
      <CorrelationHeatmap report={result as CorrelationMatrixData} />
    </>
  );
}

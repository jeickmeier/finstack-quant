"use client";
import type { ExampleProps } from "./props";
import { CorrelationHeatmap } from "@/components/finstack/analytics/components/correlation-heatmap/correlation-heatmap";
import {
  useCorrelationMatrix,
  type CorrelationMatrixRequest,
} from "@/hooks/analytics/use-correlation-matrix/use-correlation-matrix";
import data from "../data.json";
import { WorkerPreview } from "../worker-preview";
// Host data: 60 business days of a rates, a credit, an equity and a gold
// sleeve's daily decimal returns.
const panel = data.correlation.request as CorrelationMatrixRequest;
function Live({ density }: ExampleProps) {
  const correlation = useCorrelationMatrix(panel);
  return (
    <CorrelationHeatmap
      report={correlation.data ?? null}
      loading={correlation.isFetching}
      error={correlation.error?.message}
      density={density}
    />
  );
}
// The hook sits outside the item's install closure, so the example mounts its
// own provider; the install matrix runs the item with a supplied result.
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <WorkerPreview>
      <Live density={density} />
    </WorkerPreview>
  );
}

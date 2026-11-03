"use client";
import type { ExampleProps } from "./props";
import { ExposureReport } from "@/components/finstack/portfolio/components/exposure-report/exposure-report";
import {
  useFactorSensitivities,
  usePrimitiveExposures,
} from "@/hooks/portfolio/use-exposure-report/use-exposure-report";
import data from "../data.json";
import { WorkerPreview } from "../worker-preview";
// Host data: the portfolio workbench's book (two swaps and a deposit) over the
// pricing market, with additive dv01 and theta per primitive and the
// host-parity usd_rates factor (a parallel USD-OIS shift in bp).
const { specJson, request } = data.portfolioWorkbench;
const { inputs } = data.portfolioExposures;
const marketJson = JSON.stringify(data.pricingMarket);
function Live({ density }: ExampleProps) {
  const exposures = usePrimitiveExposures({
    specJson,
    marketJson,
    metrics: inputs.metrics,
  });
  const sensitivities = useFactorSensitivities({
    specJson,
    marketJson,
    factorsJson: inputs.factorsJson,
    asOf: request.asOf,
  });
  return (
    <ExposureReport
      report={exposures.data ?? null}
      loading={exposures.isFetching}
      error={exposures.error?.message}
      sensitivities={sensitivities.data ?? null}
      sensitivitiesLoading={sensitivities.isFetching}
      sensitivitiesError={sensitivities.error?.message}
      density={density}
    />
  );
}
// The hooks sit outside the item's install closure, so the example mounts its
// own provider; the install matrix runs the item with a supplied result.
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <WorkerPreview>
      <Live density={density} />
    </WorkerPreview>
  );
}

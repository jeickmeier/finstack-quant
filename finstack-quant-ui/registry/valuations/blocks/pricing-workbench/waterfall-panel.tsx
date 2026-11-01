"use client";
import {
  useDealSimulation,
  type DealSimulationRequest,
} from "@/hooks/valuations/use-deal-report/use-deal-report";
import { WaterfallReport } from "@/components/finstack/valuations/components/waterfall-report/waterfall-report";
/** Native period-by-period waterfall of the completed request's deal. */
export function WaterfallPanel({
  request,
  density,
}: {
  request: DealSimulationRequest;
  density?: "compact" | "comfortable";
}) {
  const simulation = useDealSimulation(request);
  return (
    <WaterfallReport
      run={simulation.data ?? null}
      loading={simulation.isFetching}
      error={simulation.error?.message}
      density={density}
    />
  );
}

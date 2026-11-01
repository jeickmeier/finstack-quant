"use client";
import {
  useDealSimulation,
  useSimulationFieldUnits,
  type DealSimulationRequest,
} from "@/hooks/valuations/use-deal-report/use-deal-report";
import { CoverageTestChart } from "@/components/finstack/valuations/components/coverage-test-chart/coverage-test-chart";
/** Native coverage tests of the completed request's deal, in the units Rust states for its currency. */
export function CoveragePanel({
  request,
  density,
}: {
  request: DealSimulationRequest;
  density?: "compact" | "comfortable";
}) {
  const simulation = useDealSimulation(request);
  const units = useSimulationFieldUnits(
    simulation.data?.diagnostics.periods?.[0]?.pool_balance.currency ?? null,
  );
  return (
    <CoverageTestChart
      run={simulation.data ?? null}
      units={units.data ?? null}
      loading={simulation.isFetching || units.isFetching}
      error={simulation.error?.message ?? units.error?.message}
      density={density}
    />
  );
}

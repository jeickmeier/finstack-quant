"use client";
import {
  useScenarioTable,
  type ScenarioRequest,
} from "@/hooks/use-scenario-table/use-scenario-table";
import { ScenarioHeatmapExample } from "@/components/finstack/components/scenario-heatmap/example";
/** A detail panel delegates all calculation and rendering to their component owners. */
export function ScenarioPanel({
  request,
  priceDomain,
}: {
  request: ScenarioRequest;
  priceDomain: readonly [number, number];
}) {
  const query = useScenarioTable(request);
  return query.error ? (
    <p role="alert">{query.error.message}</p>
  ) : query.data ? (
    <ScenarioHeatmapExample
      key={JSON.stringify(request)}
      table={query.data}
      priceDomain={priceDomain}
    />
  ) : (
    <p role="status">Calculating scenarios…</p>
  );
}

"use client";
import {
  useScenarioTable,
  type ScenarioRequest,
} from "@/hooks/valuations/use-scenario-table/use-scenario-table";
import { ScenarioHeatmapPanel } from "@/components/finstack/valuations/components/scenario-heatmap/scenario-heatmap";
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
    <ScenarioHeatmapPanel
      key={JSON.stringify(request)}
      table={query.data}
      priceDomain={priceDomain}
    />
  ) : (
    <p role="status">Calculating scenarios…</p>
  );
}

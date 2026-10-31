"use client";
import { useState } from "react";
import {
  useScenarioTable,
  type ScenarioRequest,
} from "@/hooks/valuations/use-scenario-table/use-scenario-table";
import { ScenarioHeatmapPanel } from "@/components/finstack/valuations/components/scenario-heatmap/scenario-heatmap";
import { SchemaForm } from "@/components/finstack/core/components/schema-form/schema-form";
import { scenarioGridModule } from "./scenario-grid";
/** Parse the host grid for the editor; a grid the contract cannot read is reported, never replaced. */
function gridValues(
  gridJson: string,
): { value: Record<string, unknown> } | { error: string } {
  try {
    const value = scenarioGridModule.codec.parse(gridJson);
    if (value === null || typeof value !== "object" || Array.isArray(value))
      throw new TypeError("Scenario grid must be a JSON object");
    return { value: value as Record<string, unknown> };
  } catch (error) {
    return { error: error instanceof Error ? error.message : String(error) };
  }
}
/**
 * Edits the native ScenarioGrid seeded from the host's grid and delegates the
 * table and its rendering to their owners. The seeded grid runs as supplied;
 * an edited grid runs only when submitted, and Rust validates its values.
 */
export function ScenarioPanel({
  request,
  priceDomain,
}: {
  request: ScenarioRequest;
  priceDomain?: readonly [number, number];
}) {
  const [initial] = useState(() => gridValues(request.gridJson));
  const [gridJson, setGridJson] = useState(request.gridJson);
  const current = { ...request, gridJson };
  const query = useScenarioTable(current);
  return (
    <div className="flex flex-col gap-3">
      {"error" in initial ? (
        <p role="alert">{initial.error}</p>
      ) : (
        <SchemaForm
          module={scenarioGridModule}
          defaultValues={initial.value}
          layout="full"
          submitLabel="Run scenarios"
          validate={async (json) => json}
          onSubmit={setGridJson}
        />
      )}
      {query.error ? (
        <p role="alert">{query.error.message}</p>
      ) : query.data ? (
        <ScenarioHeatmapPanel
          key={JSON.stringify(current)}
          table={query.data}
          priceDomain={priceDomain}
        />
      ) : (
        <p role="status">Calculating scenarios…</p>
      )}
    </div>
  );
}

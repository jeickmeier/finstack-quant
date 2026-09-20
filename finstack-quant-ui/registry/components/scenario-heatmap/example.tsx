"use client";
import { useState } from "react";
import type { ScenarioTable } from "finstack-quant-wasm";
import { useLinkedSelection } from "@/hooks/use-linked-selection/use-linked-selection";
import { ScenarioHeatmap } from "./scenario-heatmap";
/** Standalone supplied-result example. Remount when loading a different table. */
export function ScenarioHeatmapExample({
  table,
  priceDomain,
}: {
  table: ScenarioTable;
  priceDomain: readonly [number, number];
}) {
  const [severity, setSeverity] = useState(table.cells[0]?.severity),
    link = useLinkedSelection();
  if (severity === undefined) return <p>No returned scenario cells</p>;
  return (
    <ScenarioHeatmap
      table={table}
      severity={severity}
      onSeverityChange={setSeverity}
      priceDomain={priceDomain}
      link={link}
      caption="Original native scenario prices"
      sources={[{ label: "structuredCreditTrancheScenarioTable" }]}
    />
  );
}

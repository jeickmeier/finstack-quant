"use client";
import type { ExampleProps } from "./props";
import { useState } from "react";
import { ScenarioHeatmap } from "@/components/finstack/valuations/components/scenario-heatmap/scenario-heatmap";
import { useLinkedSelection } from "@/hooks/shared/use-linked-selection/use-linked-selection";
import type { ScenarioTable } from "finstack-quant-wasm";
import data from "../data.json";
const scenario = data.scenarios.cases[0]!.table as ScenarioTable;
export function Example({
  width = 880,
  presentation,
}: ExampleProps & {
  presentation?: Pick<
    import("react").ComponentProps<typeof ScenarioHeatmap>,
    "title" | "subtitle" | "onSelect" | "renderTooltipBody" | "figureRef"
  >;
}) {
  const [severity, setSeverity] = useState(scenario.cells[0]!.severity);
  const link = useLinkedSelection();
  return (
    <ScenarioHeatmap
      {...presentation}
      table={scenario}
      severity={severity}
      onSeverityChange={setSeverity}
      priceDomain={[80, 140]}
      width={width}
      height={460}
      link={link}
    />
  );
}

"use client";
import type { ExampleProps } from "./props";
import { ScenarioPnlTable } from "@/components/finstack/portfolio/components/scenario-pnl-table/scenario-pnl-table";
import data from "../data.json";
import { scenarioPnlItems } from "../fixtures";
// The host-parity deposit book under a 10bp rise and a 15bp fall of USD-OIS.
const batch = data.scenarioPnl.batch;
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <ScenarioPnlTable
      report={{
        positionIds: batch.positionIds,
        items: scenarioPnlItems,
      }}
      density={density}
    />
  );
}

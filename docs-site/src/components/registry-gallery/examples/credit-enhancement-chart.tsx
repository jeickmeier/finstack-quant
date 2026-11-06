"use client";
import type { SimulationFieldUnits } from "finstack-quant-wasm";
import type { ExampleProps } from "./props";
import { CreditEnhancementChart } from "@/components/finstack/valuations/components/credit-enhancement-chart/credit-enhancement-chart";
import data from "../data.json";
import { dealCharts } from "../fixtures";

// Captured native runSimulation and simulationFieldUnits of a three-class deal whose
// junior classes lose enhancement over its life; this item needs no worker.
const covered = dealCharts.cases.find((entry) => entry.name === "covered")!;

export function Example({ density = "compact" }: ExampleProps) {
  return (
    <CreditEnhancementChart
      run={covered.run}
      units={data.dealCharts.units as SimulationFieldUnits}
      density={density}
    />
  );
}

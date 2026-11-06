"use client";
import type { SimulationFieldUnits } from "finstack-quant-wasm";
import type { ExampleProps } from "./props";
import { CoverageTestChart } from "@/components/finstack/valuations/components/coverage-test-chart/coverage-test-chart";
import data from "../data.json";
import { dealCharts } from "../fixtures";

// Captured native runSimulation and simulationFieldUnits of a deal with a diverting
// OC test it breaches and a tranche trigger that diverts no cash; this item needs no
// worker.
const covered = dealCharts.cases.find((entry) => entry.name === "covered")!;

export function Example({ density = "compact" }: ExampleProps) {
  return (
    <CoverageTestChart
      run={covered.run}
      units={data.dealCharts.units as SimulationFieldUnits}
      initialTest="OC_SENIOR"
      density={density}
    />
  );
}

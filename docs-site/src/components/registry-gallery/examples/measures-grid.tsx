"use client";
import type { ExampleProps } from "./props";
import { MeasuresGrid } from "@/components/finstack/valuations/components/measures-grid/measures-grid";
import type { MetricMetadata } from "finstack-quant-wasm";
import data from "../data.json";

export function Example({ density = "compact" }: ExampleProps) {
  return (
    <MeasuresGrid
      result={data.bond.result}
      metadata={data.bond.metadata as MetricMetadata[]}
      density={density}
    />
  );
}

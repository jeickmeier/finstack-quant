"use client";
import type { ExampleProps } from "./props";
import { MeasuresGrid } from "@/components/finstack/valuations/components/measures-grid/measures-grid";
import type { MetricMetadata } from "finstack-quant-wasm";
import data from "../data.json";
import { bondResult } from "../fixtures";

const result = bondResult;

export function Example({ density = "compact" }: ExampleProps) {
  return (
    <MeasuresGrid
      result={result}
      metadata={data.bond.metadata as MetricMetadata[]}
      density={density}
    />
  );
}

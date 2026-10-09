"use client";
import type { ExampleProps } from "./props";
import { DimensionalMeasures } from "@/components/finstack/valuations/components/dimensional-measures/dimensional-measures";
import type { MetricMetadata, ValuationResult } from "finstack-quant-wasm";
import data from "../data.json";

const result = data.bond.result as unknown as ValuationResult;

export function Example(_props: ExampleProps) {
  return (
    <DimensionalMeasures
      result={result}
      metadata={data.bond.metadata as MetricMetadata[]}
    />
  );
}

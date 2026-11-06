"use client";
import type { ExampleProps } from "./props";
import { DimensionalMeasures } from "@/components/finstack/valuations/components/dimensional-measures/dimensional-measures";
import type { MetricMetadata } from "finstack-quant-wasm";
import data from "../data.json";
import { bondResult } from "../fixtures";

const result = bondResult;

export function Example(_props: ExampleProps) {
  return (
    <DimensionalMeasures
      result={result}
      metadata={data.bond.metadata as MetricMetadata[]}
    />
  );
}

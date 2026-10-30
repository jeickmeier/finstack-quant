"use client";
import type { MetricMetadata } from "finstack-quant-wasm";
import type { ExampleProps } from "./props";
import {
  PositionsTable,
  type PositionsTableData,
} from "@/components/finstack/portfolio/components/positions-table/positions-table";
import data from "../data.json";
// The native valuation of the bond and swap book with Rust metric metadata
// for the measure labels; this item needs no worker.
const report = data.portfolioValuation.report as unknown as PositionsTableData;
const metadata = data.portfolioValuation
  .metadata as unknown as MetricMetadata[];
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <PositionsTable report={report} metadata={metadata} density={density} />
  );
}

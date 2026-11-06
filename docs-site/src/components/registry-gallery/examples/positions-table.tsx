"use client";
import type { ExampleProps } from "./props";
import { PositionsTable } from "@/components/finstack/portfolio/components/positions-table/positions-table";
import { portfolioValuation } from "../fixtures";
// The native valuation of the bond and swap book with Rust metric metadata
// for the measure labels; this item needs no worker.
const report = portfolioValuation.report;
const metadata = portfolioValuation.metadata;
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <PositionsTable report={report} metadata={metadata} density={density} />
  );
}

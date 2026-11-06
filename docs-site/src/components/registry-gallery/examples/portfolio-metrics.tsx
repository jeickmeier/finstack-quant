"use client";
import type { ExampleProps } from "./props";
import { PortfolioMetrics } from "@/components/finstack/portfolio/components/portfolio-metrics/portfolio-metrics";
import { portfolioValuation } from "../fixtures";
// Aggregated metrics of the bond and swap book and its bucketed_dv01 series
// in Rust order; this item needs no worker.
const report = portfolioValuation.report;
const series = portfolioValuation.series;
const metadata = portfolioValuation.metadata;
// Host display choice: four decimal places of the returned values.
const fourPlaces = (value: number) => value.toFixed(4);
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <PortfolioMetrics
      report={report}
      series={series}
      metadata={metadata}
      formatValue={fourPlaces}
      density={density}
    />
  );
}

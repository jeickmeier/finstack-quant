"use client";
import type { MetricMetadata } from "finstack-quant-wasm";
import type { ExampleProps } from "./props";
import {
  PortfolioMetrics,
  type PortfolioMetricSeriesData,
  type PortfolioMetricsData,
} from "@/components/finstack/portfolio/components/portfolio-metrics/portfolio-metrics";
import data from "../data.json";
// Aggregated metrics of the bond and swap book and its bucketed_dv01 series
// in Rust order; this item needs no worker.
const report = data.portfolioValuation
  .report as unknown as PortfolioMetricsData;
const series = data.portfolioValuation
  .series as unknown as PortfolioMetricSeriesData;
const metadata = data.portfolioValuation
  .metadata as unknown as MetricMetadata[];
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

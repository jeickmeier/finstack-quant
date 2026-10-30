"use client";
import type { ExampleProps } from "./props";
import {
  PortfolioSummary,
  type PortfolioSummaryData,
} from "@/components/finstack/portfolio/components/portfolio-summary/portfolio-summary";
import data from "../data.json";
// The native valuation of the bond and swap book over the pricing market;
// this item needs no worker.
const report = data.portfolioValuation
  .report as unknown as PortfolioSummaryData;
export function Example({ density = "compact" }: ExampleProps) {
  return <PortfolioSummary report={report} density={density} />;
}

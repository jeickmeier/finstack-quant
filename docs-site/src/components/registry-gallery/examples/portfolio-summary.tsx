"use client";
import type { ExampleProps } from "./props";
import { PortfolioSummary } from "@/components/finstack/portfolio/components/portfolio-summary/portfolio-summary";
import { portfolioValuation } from "../fixtures";
// The native valuation of the bond and swap book over the pricing market;
// this item needs no worker.
const report = portfolioValuation.report;
export function Example({ density = "compact" }: ExampleProps) {
  return <PortfolioSummary report={report} density={density} />;
}

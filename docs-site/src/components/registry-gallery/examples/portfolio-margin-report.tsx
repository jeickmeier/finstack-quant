"use client";
import type { ExampleProps } from "./props";
import { PortfolioMarginReport } from "@/components/finstack/portfolio/components/portfolio-margin-report/portfolio-margin-report";
import { portfolioMargin } from "../fixtures";
// Two swaps under the Rust USD regulatory CSA and a regulatory CSA-B, and an
// unmargined deposit, with 1,000,000 USD of IM already held under the first.
const run = portfolioMargin.cases.find(
  (entry) => entry.name === "usd-collateral-held",
)!;
export function Example({ density = "compact" }: ExampleProps) {
  return <PortfolioMarginReport result={run.result} density={density} />;
}

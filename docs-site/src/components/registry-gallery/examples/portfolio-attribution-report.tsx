"use client";
import type { ExampleProps } from "./props";
import {
  PortfolioAttributionReport,
  type PortfolioAttributionData,
} from "@/components/finstack/portfolio/components/portfolio-attribution-report/portfolio-attribution-report";
import data from "../data.json";
// The margined rates book (two bilateral swaps and a deposit) attributed by
// the Rust waterfall from the pricing market to the market after a 50bp
// USD-OIS shock, reconciled at a one-cent tolerance.
const run = data.portfolioAttribution.cases.find(
  (entry) => entry.scenario === "usd-ois-50bp" && entry.name === "waterfall",
)!;
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <PortfolioAttributionReport
      report={run.report as unknown as PortfolioAttributionData}
      density={density}
    />
  );
}

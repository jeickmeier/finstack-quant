"use client";
import type { ExampleProps } from "./props";
import { MarketScenarioReport } from "@/components/finstack/scenarios/components/market-scenario-report/market-scenario-report";
import { marketScenario } from "../fixtures";
// The native report of a one-month roll plus a 25bp USD-OIS shift on the example bond.
const report = marketScenario.cases.find(
  (entry) => entry.name === "roll-1m-25bp",
)!.report;
export function Example({ density = "compact" }: ExampleProps) {
  return <MarketScenarioReport report={report} density={density} />;
}

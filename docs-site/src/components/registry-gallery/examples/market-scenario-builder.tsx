"use client";
import { useState } from "react";
import type { ExampleProps } from "./props";
import {
  MarketScenarioBuilder,
  type MarketScenarioDraft,
} from "@/components/finstack/scenarios/components/market-scenario-builder/market-scenario-builder";
import data from "../data.json";
// A one-month roll plus a 25bp USD-OIS shift, with the spec Rust built for it.
const entry = data.marketScenario.cases.find(
  (entry) => entry.name === "roll-1m-25bp",
)!;
export function Example({ density = "compact" }: ExampleProps) {
  const [draft, setDraft] = useState(entry.draft as MarketScenarioDraft);
  return (
    <MarketScenarioBuilder
      value={draft}
      onValueChange={setDraft}
      curveIds={["USD-OIS", "HZ"]}
      specJson={entry.scenarioJson}
      density={density}
    />
  );
}

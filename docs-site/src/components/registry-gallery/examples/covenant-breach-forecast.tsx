"use client";
import type { ExampleProps } from "./props";
import { CovenantBreachForecast } from "@/components/finstack/covenants/components/covenant-breach-forecast/covenant-breach-forecast";
import type { FutureBreachWire } from "@/lib/finstack/generated/types/future_breach";
import data from "../data.json";

// Captured native statements_analytics.forecastBreaches of a tightened analyst
// engine (interest coverage 5.5x, leverage 2.1x) under the stochastic config
// shown (tests/covenants/generate.mjs); this item needs no worker.
const configJson = JSON.stringify(data.covenantForecastConfig);
const breaches = data.covenantBreachForecast
  .stochastic as unknown as FutureBreachWire[];

export function Example({ density = "compact" }: ExampleProps) {
  return (
    <CovenantBreachForecast
      defaultConfigJson={configJson}
      onSubmit={() => {}}
      breaches={breaches}
      density={density}
    />
  );
}

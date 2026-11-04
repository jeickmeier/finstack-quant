"use client";
import type { ExampleProps } from "./props";
import { CovenantBreachForecast } from "@/components/finstack/covenants/components/covenant-breach-forecast/covenant-breach-forecast";
import data from "../data.json";
import { covenantBreachForecast } from "../fixtures";

// Captured native statements_analytics.forecastBreaches of a tightened analyst
// engine (interest coverage 5.5x, leverage 2.1x) under the stochastic config
// shown (tests/covenants/generate.mjs); this item needs no worker.
const configJson = JSON.stringify(data.covenantForecastConfig);
const breaches = covenantBreachForecast.stochastic;

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

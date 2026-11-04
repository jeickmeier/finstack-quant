"use client";
import type { ExampleProps } from "./props";
import { ForecastBacktest } from "@/components/finstack/statements/components/forecast-backtest/forecast-backtest";
import { statementBacktest } from "../fixtures";

// Captured native statements_analytics.backtestForecast of the analyst
// model's revenue forecast against illustrative reported quarters; this
// item needs no worker.
const metrics = statementBacktest;

export function Example({ density = "compact" }: ExampleProps) {
  return (
    <ForecastBacktest
      metrics={metrics}
      caption="Revenue: forecast before the roll forward against reported values, 2025Q2, 2025Q3, 2025Q4"
      density={density}
    />
  );
}

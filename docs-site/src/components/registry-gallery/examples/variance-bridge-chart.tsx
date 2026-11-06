"use client";
import type { ExampleProps } from "./props";
import { VarianceBridgeChart } from "@/components/finstack/statements/components/variance-bridge-chart/variance-bridge-chart";
import { statementBridge } from "../fixtures";

// Captured native statements_analytics.varianceBridge of 2025Q2 EBITDA across
// revenue, cost of revenue and opex, before and after an illustrative roll
// forward; this item needs no worker.
const bridge = statementBridge;

export function Example({ density = "compact" }: ExampleProps) {
  return <VarianceBridgeChart bridge={bridge} density={density} />;
}

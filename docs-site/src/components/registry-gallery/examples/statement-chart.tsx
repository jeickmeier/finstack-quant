"use client";
import type { ExampleProps } from "./props";
import { StatementChart } from "@/components/finstack/statements/components/statement-chart/statement-chart";
import type { FinancialModelSpecWire } from "@/lib/finstack/generated/types/financial_model_spec";
import data from "../data.json";
import { statementResult } from "../fixtures";

// Captured native evaluation of the analyst model; this item needs no worker.
const model = data.statementModel as FinancialModelSpecWire;
const result = statementResult;

export function Example(_props: ExampleProps) {
  return <StatementChart model={model} result={result} nodeId="revenue" />;
}

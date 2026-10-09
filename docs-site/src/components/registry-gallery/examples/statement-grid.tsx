"use client";
import type { ExampleProps } from "./props";
import { StatementGrid } from "@/components/finstack/statements/components/statement-grid/statement-grid";
import type { FinancialModelSpecWire } from "@/lib/finstack/generated/types/financial_model_spec";
import type { StatementResult } from "finstack-quant-wasm";
import data from "../data.json";

// Captured native evaluation of the analyst model; this item needs no worker.
const model = data.statementModel as FinancialModelSpecWire;
const result = data.statementResult as unknown as StatementResult;

export function Example(_props: ExampleProps) {
  return <StatementGrid model={model} result={result} />;
}

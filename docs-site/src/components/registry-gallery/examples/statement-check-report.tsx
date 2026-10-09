"use client";
import type { ExampleProps } from "./props";
import { StatementCheckReport } from "@/components/finstack/statements/components/statement-diagnostics/statement-check-report";
import type { CheckReport } from "finstack-quant-wasm";
import data from "../data.json";

// Captured native check run over the analyst model; this item needs no worker.
const report = data.statementCheckReport as unknown as CheckReport;

export function Example(_props: ExampleProps) {
  return <StatementCheckReport report={report} />;
}

"use client";
import type { ExampleProps } from "./props";
import { StatementCheckReport } from "@/components/finstack/statements/components/statement-diagnostics/statement-check-report";
import { statementCheckReport } from "../fixtures";

// Captured native check run over the analyst model; this item needs no worker.
const report = statementCheckReport;

export function Example({ density = "compact" }: ExampleProps) {
  return <StatementCheckReport report={report} density={density} />;
}

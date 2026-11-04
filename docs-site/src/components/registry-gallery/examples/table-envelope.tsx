"use client";
import type { ExampleProps } from "./props";
import { TableEnvelope } from "@/components/finstack/shared/table/table-envelope/table-envelope";
import { statementPlSummary } from "../fixtures";

// Captured native statements_analytics.plSummaryReport over the analyst model; this item needs no worker.
const envelope = statementPlSummary;

export function Example({ density = "compact" }: ExampleProps) {
  return (
    <TableEnvelope
      envelope={envelope}
      caption="P&L summary, 2025 quarters"
      density={density}
    />
  );
}

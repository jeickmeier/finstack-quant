"use client";
import type { ExampleProps } from "./props";
import type { TableEnvelopeWire } from "@/lib/finstack/generated/types/table_envelope";
import { TableEnvelope } from "@/components/finstack/shared/table/table-envelope/table-envelope";
import data from "../data.json";

// Captured native statements_analytics.plSummaryReport over the analyst model; this item needs no worker.
const envelope = data.statementPlSummary as unknown as TableEnvelopeWire;

export function Example({ density = "compact" }: ExampleProps) {
  return (
    <TableEnvelope
      envelope={envelope}
      caption="P&L summary, 2025 quarters"
      density={density}
    />
  );
}

"use client";
import type { ExampleProps } from "./props";
import type { InstrumentCashflowWire } from "@/lib/finstack/generated/types/instrument_cashflow";
import type { CashflowLadder } from "@/components/finstack/valuations/components/cashflow-viewer/cashflow-data";
import { CashflowViewer } from "@/components/finstack/valuations/components/cashflow-viewer/cashflow-viewer";
import data from "../data.json";
const entry = data.cashflows.cases.find((entry) => entry.type === "xccy_swap")!;
const cashflows = entry.cashflows as unknown as InstrumentCashflowWire;
// Native calendar-year ladder per row currency (USD and EUR) of the same envelope.
const ladder = entry.ladder as unknown as CashflowLadder[];
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <CashflowViewer cashflows={cashflows} ladder={ladder} density={density} />
  );
}

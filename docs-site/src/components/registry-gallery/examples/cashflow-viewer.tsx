"use client";
import type { ExampleProps } from "./props";
import type { InstrumentCashflowWire } from "@/lib/finstack/generated/types/instrument_cashflow";
import { CashflowViewer } from "@/components/finstack/valuations/components/cashflow-viewer/cashflow-viewer";
import data from "../data.json";
const cashflows = data.cashflows.cases.find(
  (entry) => entry.type === "xccy_swap",
)!.cashflows as unknown as InstrumentCashflowWire;
export function Example({ density = "compact" }: ExampleProps) {
  return <CashflowViewer cashflows={cashflows} density={density} />;
}

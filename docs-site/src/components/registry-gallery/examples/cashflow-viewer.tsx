"use client";
import type { ExampleProps } from "./props";
import { CashflowViewer } from "@/components/finstack/components/cashflow-viewer/cashflow-viewer";
import data from "../data.json";
const cashflows = data.cashflows.cases.find(
  (entry) => entry.type === "xccy_swap",
)!;
export function Example({ density = "compact" }: ExampleProps) {
  return <CashflowViewer text={cashflows.cashflows} density={density} />;
}

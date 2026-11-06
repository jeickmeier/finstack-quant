"use client";
import type { ExampleProps } from "./props";
import { CashflowViewer } from "@/components/finstack/valuations/components/cashflow-viewer/cashflow-viewer";
import { instrumentCashflows } from "../fixtures";
// A PIK term loan: the balance columns show the coupons accreting to principal.
const entry = instrumentCashflows.cases.find(
  (entry) => entry.type === "term_loan_pik",
)!;
const cashflows = entry.cashflows;
// Native calendar-year ladder per row currency of the same envelope.
const ladder = entry.ladder;
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <CashflowViewer cashflows={cashflows} ladder={ladder} density={density} />
  );
}

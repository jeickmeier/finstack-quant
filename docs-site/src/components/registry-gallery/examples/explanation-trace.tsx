"use client";
import type { ExampleProps } from "./props";
import { ExplanationTrace } from "@/components/finstack/valuations/components/explanation-trace/explanation-trace";

export function Example({ density = "compact" }: ExampleProps) {
  return (
    <>
      <p>
        Supplied canonical Rust trace example; request one from pricing with
        the <code>explain</code> argument.
      </p>
      <ExplanationTrace
        value={{
          type: "pricing",
          entries: [
            {
              kind: "cashflow_pv",
              date: "2025-01-15",
              cashflow_amount: 50000,
              cashflow_currency: "USD",
              discount_factor: 0.95,
              pv_amount: 47500,
              pv_currency: "USD",
              curve_id: "USD_GOVT",
            },
          ],
        }}
        density={density}
      />
    </>
  );
}

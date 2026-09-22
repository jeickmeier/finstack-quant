"use client";
import type { ExampleProps } from "./props";
import { ExplanationTrace } from "@/components/finstack/valuations/components/explanation-trace/explanation-trace";

export function Example({ density = "compact" }: ExampleProps) {
  return (
    <>
      <p>
        Supplied canonical Rust trace example; pricing has no trace request
        input.
      </p>
      <ExplanationTrace
        value={{
          type: "calibration",
          entries: [
            {
              kind: "calibration_iteration",
              iteration: 0,
              residual: 0.005,
              knots_updated: ["2025-01-15"],
              converged: false,
            },
          ],
        }}
        density={density}
      />
    </>
  );
}

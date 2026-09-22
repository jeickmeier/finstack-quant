"use client";
import { lazy, Suspense } from "react";
import type { ValuationResult } from "finstack-quant-wasm";
import { serializeHost } from "@/lib/finstack/codec.mjs";
import { JsonViewer } from "@/components/finstack/shared/primitives/json-viewer/json-viewer";
const MonteCarlo = lazy(() =>
  import("@/components/finstack/models/components/monte-carlo-diagnostics/monte-carlo-diagnostics").then(
    (module) => ({ default: module.MonteCarloDiagnostics }),
  ),
);
const Explanation = lazy(() =>
  import("../explanation-trace/explanation-trace").then((module) => ({
    default: module.ExplanationTrace,
  })),
);
const Covenants = lazy(() =>
  import("@/components/finstack/covenants/components/covenant-report/covenant-report").then(
    (module) => ({
      default: module.CovenantReport,
    }),
  ),
);
const rawLabels: Record<string, string> = {
  composite: "Composite details JSON",
  credit_derivative: "Credit derivative details JSON",
  fx: "FX details JSON",
  structured_credit_stochastic: "Structured credit details JSON",
};
/** Lazy typed/raw routing over returned details; retains the complete result and all stamps. */
export function ValuationDetails({
  result,
  density = "compact",
  includeExplanation = true,
}: {
  result: ValuationResult;
  density?: "compact" | "comfortable";
  /** Hide the trace when the host provides a dedicated result tab. */
  includeExplanation?: boolean;
}) {
  const details = result.details;
  const unknown =
    details &&
    details.type !== "monte_carlo" &&
    !Object.hasOwn(rawLabels, details.type);
  return (
    <section
      aria-label="Valuation details"
      data-density={density}
      className="space-y-3 font-sans text-foreground"
    >
      {unknown && (
        <p role="alert" className="text-sm text-warning">
          Unrecognized details type: {details.type}. Original data is shown
          below.
        </p>
      )}
      <Suspense fallback={<p role="status">Loading result details…</p>}>
        {details?.type === "monte_carlo" ? (
          <MonteCarlo
            data={details.data}
            currency={result.value.currency}
            density={density}
          />
        ) : details ? (
          <JsonViewer
            text={serializeHost(details.data)}
            label={
              Object.hasOwn(rawLabels, details.type)
                ? rawLabels[details.type]
                : "Unrecognized details JSON"
            }
            density={density}
          />
        ) : (
          <p role="status">No valuation details returned</p>
        )}
        {includeExplanation && result.explanation != null && (
          <Explanation value={result.explanation} density={density} />
        )}
        {result.covenants != null && (
          <Covenants value={result.covenants} density={density} />
        )}
      </Suspense>
      <details>
        <summary className="text-sm">Complete result JSON</summary>
        <JsonViewer
          label="Complete valuation result JSON"
          text={serializeHost(result)}
          density={density}
        />
      </details>
    </section>
  );
}

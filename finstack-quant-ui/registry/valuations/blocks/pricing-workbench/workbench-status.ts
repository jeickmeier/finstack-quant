import type { PriceRequest } from "@/workers/finstack-contract";

export type WorkbenchStateKind = "priced" | "failed" | "idle" | "pending";

/** Same strings and stale rules the workbench header already shows. */
export function workbenchStatus({
  workerStatus,
  candidate,
  request,
  completed,
  fetching,
  errorMessage,
}: {
  workerStatus: "starting" | "ready" | "error";
  candidate: PriceRequest | null;
  request: PriceRequest | null;
  completed: { request: PriceRequest } | null;
  fetching: boolean;
  errorMessage: string | undefined;
}): {
  stale: boolean;
  status: string;
  stateKind: WorkbenchStateKind;
  currentError: string | undefined;
} {
  const currentError = request === candidate ? errorMessage : undefined;
  // Results belong to `completed.request`; any newer candidate (or invalid edit) makes them stale.
  const stale =
    completed !== null &&
    (candidate === null ||
      candidate.instrumentJson !== completed.request.instrumentJson ||
      candidate.marketJson !== completed.request.marketJson ||
      candidate.asOf !== completed.request.asOf ||
      candidate.model !== completed.request.model ||
      JSON.stringify(candidate.metrics) !==
        JSON.stringify(completed.request.metrics) ||
      candidate.pricingOptions !== completed.request.pricingOptions ||
      candidate.marketHistory !== completed.request.marketHistory);
  const status =
    workerStatus !== "ready"
      ? `Worker ${workerStatus}`
      : !candidate
        ? completed
          ? "Edits pending validation"
          : "Waiting for valid inputs"
        : request !== candidate
          ? "Pricing queued"
          : fetching
            ? "Pricing…"
            : currentError
              ? "Pricing failed"
              : completed
                ? "Priced"
                : "Waiting for valuation";
  const stateKind: WorkbenchStateKind =
    status === "Priced"
      ? "priced"
      : status === "Pricing failed"
        ? "failed"
        : workerStatus !== "ready" || (!candidate && !completed)
          ? "idle"
          : "pending";
  return { stale, status, stateKind, currentError };
}

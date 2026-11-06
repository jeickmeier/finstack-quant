"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { EvaluateCovenantsRequest } from "@/workers/finstack-contract";
export type {
  DatedCovenantReports,
  EvaluateCovenantsRequest,
} from "@/workers/finstack-contract";
/** Snapshot the engine, model and result JSON. Null disables the evaluation. */
export function evaluateCovenantsOptions(
  client: FinstackClient | null,
  session: number,
  request: EvaluateCovenantsRequest | null,
) {
  const snapshot = request === null ? null : { ...request };
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "evaluateCovenants", snapshot] as const,
    ready: snapshot !== null,
    missing:
      "Covenant evaluation requires an explicit request and ready worker",
    queryFn: (ready) => ready.call("evaluateCovenants", snapshot!),
  });
}
/** Native statements_analytics.evaluateCovenants DatedCovenantReports rows or structured failure. */
export function useEvaluateCovenants(request: EvaluateCovenantsRequest | null) {
  return useWorkerQuery((worker) =>
    evaluateCovenantsOptions(worker.client, worker.session, request),
  );
}

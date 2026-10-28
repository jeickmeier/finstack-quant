"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { StatementRequest } from "@/workers/finstack-contract";
export type { StatementRequest } from "@/workers/finstack-contract";
/** Snapshot model, market and as-of. Null disables evaluation. */
export function evaluateStatementOptions(
  client: FinstackClient | null,
  session: number,
  request: StatementRequest | null,
) {
  const snapshot = request === null ? null : { ...request };
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "evaluateStatement", snapshot] as const,
    ready: snapshot !== null,
    missing:
      "Statement evaluation requires an explicit request and ready worker",
    queryFn: (ready) => ready.call("evaluateStatement", snapshot!),
  });
}
/** Native StatementResult or structured failure. */
export function useEvaluateStatement(request: StatementRequest | null) {
  return useWorkerQuery((worker) => ({
    ...evaluateStatementOptions(worker.client, worker.session, request),
    enabled: worker.status === "ready" && request !== null,
  }));
}

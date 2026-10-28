"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { StatementExplanationRequest } from "@/workers/finstack-contract";
export type { StatementExplanationRequest } from "@/workers/finstack-contract";
/** Snapshot model, matching result, node and period. Null disables the explanation. */
export function explainStatementOptions(
  client: FinstackClient | null,
  session: number,
  request: StatementExplanationRequest | null,
) {
  const snapshot = request === null ? null : { ...request };
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "explainStatement", snapshot] as const,
    ready: snapshot !== null,
    missing:
      "Statement explanation requires an explicit request and ready worker",
    queryFn: (ready) => ready.call("explainStatement", snapshot!),
  });
}
/** Native structured Explanation or structured failure. */
export function useExplainStatement(
  request: StatementExplanationRequest | null,
) {
  return useWorkerQuery((worker) => ({
    ...explainStatementOptions(worker.client, worker.session, request),
    enabled: worker.status === "ready" && request !== null,
  }));
}

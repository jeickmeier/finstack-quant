"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { StatementRequest } from "@/workers/finstack-contract";

/** One immutable model/market/date request in the current worker session. */
export function useEvaluateStatement(request: StatementRequest | null) {
  const snapshot = request === null ? null : Object.freeze({ ...request });
  return useWorkerQuery((worker) => ({
    ...workerQueryOptions({
      client: worker.client,
      queryKey: ["finstack", worker.session, "statement", snapshot] as const,
      ready: snapshot !== null,
      missing: "Statement worker is not ready",
      queryFn: (client) => client.call("evaluateStatement", snapshot!),
    }),
    enabled: worker.status === "ready" && snapshot !== null,
  }));
}

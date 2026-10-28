"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { StatementTraceRequest } from "@/workers/finstack-contract";
export type { StatementTraceRequest } from "@/workers/finstack-contract";
/** Snapshot model and root node. Null disables the trace. */
export function traceStatementOptions(
  client: FinstackClient | null,
  session: number,
  request: StatementTraceRequest | null,
) {
  const snapshot = request === null ? null : { ...request };
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "traceStatement", snapshot] as const,
    ready: snapshot !== null,
    missing: "Statement trace requires an explicit request and ready worker",
    queryFn: (ready) => ready.call("traceStatement", snapshot!),
  });
}
/** Native DependencyTree or structured failure. */
export function useTraceStatement(request: StatementTraceRequest | null) {
  return useWorkerQuery((worker) => ({
    ...traceStatementOptions(worker.client, worker.session, request),
    enabled: worker.status === "ready" && request !== null,
  }));
}

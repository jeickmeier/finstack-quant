"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { StatementRollupRequest } from "@/workers/finstack-contract";
export type { StatementRollupRequest } from "@/workers/finstack-contract";
/** Snapshot model, matching result and RollupSpec. Null disables the rollup. */
export function rollupStatementOptions(
  client: FinstackClient | null,
  session: number,
  request: StatementRollupRequest | null,
) {
  const snapshot = request === null ? null : { ...request };
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "rollupStatement", snapshot] as const,
    ready: snapshot !== null,
    missing: "Statement rollup requires an explicit request and ready worker",
    queryFn: (ready) => ready.call("rollupStatement", snapshot!),
  });
}
/** Native rolled-up StatementResult or structured failure. */
export function useRollupStatement(request: StatementRollupRequest | null) {
  return useWorkerQuery((worker) => ({
    ...rollupStatementOptions(worker.client, worker.session, request),
    enabled: worker.status === "ready" && request !== null,
  }));
}

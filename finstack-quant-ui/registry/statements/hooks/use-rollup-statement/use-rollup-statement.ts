"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type {
  StatementDefaultRollupRequest,
  StatementRollupRequest,
} from "@/workers/finstack-contract";
export type {
  StatementDefaultRollupRequest,
  StatementRollupRequest,
} from "@/workers/finstack-contract";
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
  return useWorkerQuery((worker) =>
    rollupStatementOptions(worker.client, worker.session, request),
  );
}
/** Snapshot model and frequency. Null disables the default. */
export function defaultRollupSpecOptions(
  client: FinstackClient | null,
  session: number,
  request: StatementDefaultRollupRequest | null,
) {
  const snapshot = request === null ? null : { ...request };
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "defaultRollupSpec", snapshot] as const,
    ready: snapshot !== null,
    missing:
      "Default rollup spec requires an explicit request and ready worker",
    queryFn: (ready) => ready.call("defaultRollupSpec", snapshot!),
  });
}
/** Native RollupSpec listing the methods a model declares, or structured failure. */
export function useDefaultRollupSpec(
  request: StatementDefaultRollupRequest | null,
) {
  return useWorkerQuery((worker) =>
    defaultRollupSpecOptions(worker.client, worker.session, request),
  );
}

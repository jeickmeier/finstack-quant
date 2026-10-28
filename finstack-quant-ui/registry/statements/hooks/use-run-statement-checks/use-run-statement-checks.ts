"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { StatementChecksRequest } from "@/workers/finstack-contract";
export type { StatementChecksRequest } from "@/workers/finstack-contract";
/** Snapshot model, matching result, runner kind and configuration. Null disables the run. */
export function runStatementChecksOptions(
  client: FinstackClient | null,
  session: number,
  request: StatementChecksRequest | null,
) {
  const snapshot = request === null ? null : { ...request };
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "runStatementChecks", snapshot] as const,
    ready: snapshot !== null,
    missing: "Statement checks require an explicit request and ready worker",
    queryFn: (ready) => ready.call("runStatementChecks", snapshot!),
  });
}
/** Native CheckReport or structured failure. */
export function useRunStatementChecks(request: StatementChecksRequest | null) {
  return useWorkerQuery((worker) => ({
    ...runStatementChecksOptions(worker.client, worker.session, request),
    enabled: worker.status === "ready" && request !== null,
  }));
}

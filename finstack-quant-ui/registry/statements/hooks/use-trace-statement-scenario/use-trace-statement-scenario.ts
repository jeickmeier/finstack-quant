"use client";
import { useQueries } from "@tanstack/react-query";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import { useFinstack } from "@/hooks/shared/use-finstack/use-finstack";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { StatementScenarioTraceRequest } from "@/workers/finstack-contract";
export type { StatementScenarioTraceRequest } from "@/workers/finstack-contract";
/** Snapshot the scenario set and scenario name. Null disables the trace. */
export function traceStatementScenarioOptions(
  client: FinstackClient | null,
  session: number,
  request: StatementScenarioTraceRequest | null,
) {
  const snapshot = request === null ? null : { ...request };
  return workerQueryOptions({
    client,
    queryKey: [
      "finstack",
      session,
      "traceStatementScenario",
      snapshot,
    ] as const,
    ready: snapshot !== null,
    missing: "Scenario lineage requires an explicit request and ready worker",
    queryFn: (ready) => ready.call("traceStatementScenario", snapshot!),
  });
}
/** Native lineage, root ancestor first and the scenario last, or a structured failure. */
export function useTraceStatementScenario(
  request: StatementScenarioTraceRequest | null,
) {
  return useWorkerQuery((worker) => ({
    ...traceStatementScenarioOptions(worker.client, worker.session, request),
    enabled: worker.status === "ready" && request !== null,
  }));
}
/** One native lineage query per request, in request order; each entry keeps its own status and error. */
export function useTraceStatementScenarios(
  requests: readonly StatementScenarioTraceRequest[],
) {
  const worker = useFinstack();
  return useQueries({
    queries: requests.map((request) => ({
      ...traceStatementScenarioOptions(worker.client, worker.session, request),
      enabled: worker.status === "ready",
    })),
  });
}

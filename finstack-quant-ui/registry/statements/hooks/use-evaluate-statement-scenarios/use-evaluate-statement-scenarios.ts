"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { StatementScenarioRequest } from "@/workers/finstack-contract";
export type {
  StatementScenarioRequest,
  StatementScenarioResults,
} from "@/workers/finstack-contract";
/** Snapshot model, scenario set, market and as-of. Null disables evaluation. */
export function evaluateStatementScenariosOptions(
  client: FinstackClient | null,
  session: number,
  request: StatementScenarioRequest | null,
) {
  const snapshot = request === null ? null : { ...request };
  return workerQueryOptions({
    client,
    queryKey: [
      "finstack",
      session,
      "evaluateStatementScenarios",
      snapshot,
    ] as const,
    ready: snapshot !== null,
    missing: "Statement scenarios require an explicit request and ready worker",
    queryFn: (ready) => ready.call("evaluateStatementScenarios", snapshot!),
  });
}
/** Native scenario name to StatementResult map or structured failure. */
export function useEvaluateStatementScenarios(
  request: StatementScenarioRequest | null,
) {
  return useWorkerQuery((worker) => ({
    ...evaluateStatementScenariosOptions(
      worker.client,
      worker.session,
      request,
    ),
    enabled: worker.status === "ready" && request !== null,
  }));
}

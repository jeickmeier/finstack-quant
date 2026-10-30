"use client";
import { requestSnapshot } from "@/hooks/shared/use-finstack/snapshot";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { StatementScenarioDiffRequest } from "@/workers/finstack-contract";
export type {
  StatementScenarioDiff,
  StatementScenarioDiffRequest,
  StatementScenarioPair,
} from "@/workers/finstack-contract";
/** Snapshot the scenario set, results, pairs, metrics and periods. Null disables the diff. */
export function diffStatementScenariosOptions(
  client: FinstackClient | null,
  session: number,
  request: StatementScenarioDiffRequest | null,
) {
  const { snapshot, key } = requestSnapshot(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "diffStatementScenarios", key] as const,
    ready: snapshot !== null,
    missing: "Scenario diffs require an explicit request and ready worker",
    queryFn: (ready) => ready.call("diffStatementScenarios", snapshot!),
  });
}
/** One native ScenarioDiff per requested pair, in pair order, or a structured failure. */
export function useDiffStatementScenarios(
  request: StatementScenarioDiffRequest | null,
) {
  return useWorkerQuery((worker) => ({
    ...diffStatementScenariosOptions(worker.client, worker.session, request),
    enabled: worker.status === "ready" && request !== null,
  }));
}

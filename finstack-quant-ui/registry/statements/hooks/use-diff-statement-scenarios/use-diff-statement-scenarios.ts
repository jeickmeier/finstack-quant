"use client";
import {
  snapshotQueryOptions,
  useWorkerQuery,
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
  return snapshotQueryOptions(
    client,
    session,
    "diffStatementScenarios",
    request,
    "Scenario diffs require an explicit request and ready worker",
  );
}
/** One native ScenarioDiff per requested pair, in pair order, or a structured failure. */
export function useDiffStatementScenarios(
  request: StatementScenarioDiffRequest | null,
) {
  return useWorkerQuery((worker) =>
    diffStatementScenariosOptions(worker.client, worker.session, request),
  );
}

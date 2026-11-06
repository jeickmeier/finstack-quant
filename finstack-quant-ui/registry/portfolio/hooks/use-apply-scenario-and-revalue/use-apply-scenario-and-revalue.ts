"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { ScenarioRevalueRequest } from "@/workers/finstack-contract";
export type {
  ScenarioRevalue,
  ScenarioRevalueRequest,
} from "@/workers/finstack-contract";
/** Key the revaluation on the complete snapshotted spec, market and scenario text; nothing is parsed. */
export function applyScenarioAndRevalueOptions(
  client: FinstackClient | null,
  session: number,
  request: ScenarioRevalueRequest | null,
) {
  const snapshot = request === null ? null : structuredClone(request);
  return workerQueryOptions({
    client,
    queryKey: [
      "finstack",
      session,
      "applyScenarioAndRevalue",
      snapshot,
    ] as const,
    ready: snapshot !== null,
    missing: "No portfolio scenario supplied",
    queryFn: (ready) => ready.call("applyScenarioAndRevalue", snapshot!),
  });
}
/** Native portfolio.applyScenarioAndRevalue stressed valuation and report with the book's id orders, or the actual native error; supply a Finstack/Query provider. */
export function useApplyScenarioAndRevalue(
  request: ScenarioRevalueRequest | null,
) {
  return useWorkerQuery((worker) =>
    applyScenarioAndRevalueOptions(worker.client, worker.session, request),
  );
}

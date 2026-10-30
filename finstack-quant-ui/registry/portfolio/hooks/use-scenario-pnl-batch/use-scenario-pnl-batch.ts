"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { ScenarioPnlBatchRequest } from "@/workers/finstack-contract";
export type {
  ScenarioPnlBatch,
  ScenarioPnlBatchRequest,
} from "@/workers/finstack-contract";
/** Key the batch on the complete snapshotted spec, market and scenario texts; nothing is parsed or reordered. */
export function scenarioPnlBatchOptions(
  client: FinstackClient | null,
  session: number,
  request: ScenarioPnlBatchRequest | null,
) {
  const snapshot = request === null ? null : structuredClone(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "scenarioPnlBatch", snapshot] as const,
    ready: snapshot !== null,
    missing: "No portfolio scenario batch supplied",
    queryFn: (ready) => ready.call("scenarioPnlBatch", snapshot!),
  });
}
/** Native portfolio.scenarioPnlBatch items in scenario order with the book's position order, or the earliest failing scenario's native error; supply a Finstack/Query provider. */
export function useScenarioPnlBatch(request: ScenarioPnlBatchRequest | null) {
  return useWorkerQuery((worker) => ({
    ...scenarioPnlBatchOptions(worker.client, worker.session, request),
    enabled: worker.status === "ready" && request !== null,
  }));
}

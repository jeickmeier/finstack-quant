"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { ScenarioPnlRequest } from "@/workers/finstack-contract";
export type {
  ScenarioPnlBatch,
  ScenarioPnlRequest,
} from "@/workers/finstack-contract";
/** Key the batch on the complete snapshotted spec, market and scenario texts; nothing is parsed or reordered. */
export function scenarioPnlOptions(
  client: FinstackClient | null,
  session: number,
  request: ScenarioPnlRequest | null,
) {
  const snapshot = request === null ? null : structuredClone(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "scenarioPnl", snapshot] as const,
    ready: snapshot !== null,
    missing: "No portfolio scenario batch supplied",
    queryFn: (ready) => ready.call("scenarioPnl", snapshot!),
  });
}
/** Native portfolio.scenarioPnl items in scenario order with the book's position order, or the earliest failing scenario's native error; supply a Finstack/Query provider. */
export function useScenarioPnl(request: ScenarioPnlRequest | null) {
  return useWorkerQuery((worker) =>
    scenarioPnlOptions(worker.client, worker.session, request),
  );
}

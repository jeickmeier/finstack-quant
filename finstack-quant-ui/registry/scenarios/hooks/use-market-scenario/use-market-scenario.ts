"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { MarketScenarioRequest } from "@/workers/finstack-contract";
export type {
  MarketScenarioRequest,
  MarketScenarioRoll,
  MarketScenarioShift,
} from "@/workers/finstack-contract";
/** Key the canonical spec on the complete snapshotted request; nothing is defaulted or reordered. */
export function marketScenarioOptions(
  client: FinstackClient | null,
  session: number,
  request: MarketScenarioRequest | null,
) {
  const snapshot = request === null ? null : structuredClone(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "buildMarketScenario", snapshot] as const,
    ready: snapshot !== null,
    missing: "No market scenario supplied",
    queryFn: (ready) => ready.call("buildMarketScenario", snapshot!),
  });
}
/** Canonical ScenarioSpec text built and validated by Rust from the supplied operations, or the actual native error; supply a Finstack/Query provider. */
export function useMarketScenario(request: MarketScenarioRequest | null) {
  return useWorkerQuery((worker) => ({
    ...marketScenarioOptions(worker.client, worker.session, request),
    enabled: worker.status === "ready" && request !== null,
  }));
}

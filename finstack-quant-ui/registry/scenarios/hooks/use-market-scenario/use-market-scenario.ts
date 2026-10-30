"use client";
import { useCallback } from "react";
import { useFinstack } from "@/hooks/shared/use-finstack/use-finstack";
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
/** A builder of canonical ScenarioSpec text from native operations, called on demand; rejects with the native error. */
export function useBuildMarketScenario() {
  const worker = useFinstack();
  return useCallback(
    async (request: MarketScenarioRequest) => {
      if (worker.error) throw worker.error;
      if (worker.status !== "ready" || !worker.client)
        throw new Error("Market scenario is waiting for the worker");
      return worker.client.call(
        "buildMarketScenario",
        structuredClone(request),
      );
    },
    [worker.client, worker.status, worker.error],
  );
}

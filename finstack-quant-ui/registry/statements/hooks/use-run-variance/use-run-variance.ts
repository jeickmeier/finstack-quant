"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { RunVarianceRequest } from "@/workers/finstack-contract";
export type { RunVarianceRequest } from "@/workers/finstack-contract";
export type { VarianceReport } from "finstack-quant-wasm";
/** Snapshot both results and the VarianceConfig JSON. Null disables the comparison. */
export function runVarianceOptions(
  client: FinstackClient | null,
  session: number,
  request: RunVarianceRequest | null,
) {
  const snapshot = request === null ? null : { ...request };
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "runVariance", snapshot] as const,
    ready: snapshot !== null,
    missing: "The variance requires an explicit request and ready worker",
    queryFn: (ready) => ready.call("runVariance", snapshot!),
  });
}
/** Native statements_analytics.runVariance VarianceReport or structured failure. */
export function useRunVariance(request: RunVarianceRequest | null) {
  return useWorkerQuery((worker) =>
    runVarianceOptions(worker.client, worker.session, request),
  );
}

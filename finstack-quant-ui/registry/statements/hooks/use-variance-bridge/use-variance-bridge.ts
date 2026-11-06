"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { VarianceBridgeRequest } from "@/workers/finstack-contract";
export type {
  BridgeChart,
  VarianceBridgeRequest,
} from "@/workers/finstack-contract";
/** Snapshot both results, the target, period, drivers and labels. Null disables the bridge. */
export function varianceBridgeOptions(
  client: FinstackClient | null,
  session: number,
  request: VarianceBridgeRequest | null,
) {
  const snapshot =
    request === null ? null : { ...request, drivers: [...request.drivers] };
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "varianceBridge", snapshot] as const,
    ready: snapshot !== null,
    missing:
      "The variance bridge requires an explicit request and ready worker",
    queryFn: (ready) => ready.call("varianceBridge", snapshot!),
  });
}
/** Native statements_analytics.varianceBridge BridgeChart or structured failure. */
export function useVarianceBridge(request: VarianceBridgeRequest | null) {
  return useWorkerQuery((worker) =>
    varianceBridgeOptions(worker.client, worker.session, request),
  );
}

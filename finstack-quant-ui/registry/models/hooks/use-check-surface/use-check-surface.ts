"use client";
import { requestSnapshot } from "@/hooks/shared/use-finstack/snapshot";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { CheckSurfaceRequest } from "@/workers/finstack-contract";
export type {
  CheckSurfaceRequest,
  ArbitrageReport,
} from "@/workers/finstack-contract";
/** Snapshot the stored surface, forwards and tolerance; null means no check request. */
export function checkSurfaceOptions(
  client: FinstackClient | null,
  session: number,
  request: CheckSurfaceRequest | null,
) {
  const { snapshot, key } = requestSnapshot(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "checkSurface", key] as const,
    ready: snapshot !== null,
    missing:
      "The surface arbitrage check requires a ready worker, a stored surface and forwards",
    queryFn: (ready) => ready.call("checkSurface", snapshot!),
  });
}
/** Native models.volatility.checkSurface report; errors (such as a rejected quote convention) are unchanged. */
export function useCheckSurface(request: CheckSurfaceRequest | null) {
  return useWorkerQuery((worker) => ({
    ...checkSurfaceOptions(worker.client, worker.session, request),
    enabled: worker.status === "ready" && request !== null,
  }));
}

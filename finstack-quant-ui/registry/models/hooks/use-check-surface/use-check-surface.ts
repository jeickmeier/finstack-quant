"use client";
import {
  snapshotQueryOptions,
  useWorkerQuery,
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
  return snapshotQueryOptions(
    client,
    session,
    "checkSurface",
    request,
    "The surface arbitrage check requires a ready worker, a stored surface and forwards",
  );
}
/** Native models.volatility.checkSurface report; errors (such as a rejected quote convention) are unchanged. */
export function useCheckSurface(request: CheckSurfaceRequest | null) {
  return useWorkerQuery((worker) =>
    checkSurfaceOptions(worker.client, worker.session, request),
  );
}

"use client";
import { requestSnapshot } from "@/hooks/shared/use-finstack/snapshot";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { FxStrikeSurfaceRequest } from "@/workers/finstack-contract";
export type {
  FxStrikeSurfaceRequest,
  FxStrikeSurface,
} from "@/workers/finstack-contract";
/** Snapshot every quote array, the spot and both rates; null means no materialization request. */
export function fxStrikeSurfaceOptions(
  client: FinstackClient | null,
  session: number,
  request: FxStrikeSurfaceRequest | null,
) {
  const { snapshot, key } = requestSnapshot(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "materializeFxSurface", key] as const,
    ready: snapshot !== null,
    missing:
      "FX strike-grid materialization requires a ready worker, a spot and both rates",
    queryFn: (ready) => ready.call("materializeFxSurface", snapshot!),
  });
}
/** Native strike-space surface materialized from FX delta quotes; errors are unchanged. */
export function useFxStrikeSurface(request: FxStrikeSurfaceRequest | null) {
  return useWorkerQuery((worker) =>
    fxStrikeSurfaceOptions(worker.client, worker.session, request),
  );
}

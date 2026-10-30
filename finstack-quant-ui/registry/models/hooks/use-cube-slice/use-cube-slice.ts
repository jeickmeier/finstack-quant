"use client";
import { requestSnapshot } from "@/hooks/shared/use-finstack/snapshot";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { CubeSliceRequest } from "@/workers/finstack-contract";
export type { CubeSliceRequest, CubeSlice } from "@/workers/finstack-contract";
/** Snapshot every SABR node, forward, the axis, fixed value, strikes and convention; null means no slice request. */
export function cubeSliceOptions(
  client: FinstackClient | null,
  session: number,
  request: CubeSliceRequest | null,
) {
  const { snapshot, key } = requestSnapshot(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "cubeSlice", key] as const,
    ready: snapshot !== null,
    missing:
      "Cube slicing requires a ready worker, an axis value, strikes and a convention",
    queryFn: (ready) => ready.call("cubeSlice", snapshot!),
  });
}
/** Native materialized cube slice JSON; errors are unchanged. */
export function useCubeSlice(request: CubeSliceRequest | null) {
  return useWorkerQuery((worker) =>
    cubeSliceOptions(worker.client, worker.session, request),
  );
}

"use client";
import {
  snapshotQueryOptions,
  useWorkerQuery,
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
  return snapshotQueryOptions(
    client,
    session,
    "cubeSlice",
    request,
    "Cube slicing requires a ready worker, an axis value, strikes and a convention",
  );
}
/** Native materialized cube slice JSON; errors are unchanged. */
export function useCubeSlice(request: CubeSliceRequest | null) {
  return useWorkerQuery((worker) =>
    cubeSliceOptions(worker.client, worker.session, request),
  );
}

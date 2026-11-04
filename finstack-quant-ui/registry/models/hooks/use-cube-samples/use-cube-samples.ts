"use client";
import {
  snapshotQueryOptions,
  useWorkerQuery,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { CubeSampleRequest } from "@/workers/finstack-contract";
export type { CubeSampleRequest } from "@/workers/finstack-contract";
/** Snapshot every SABR node, forward, coordinate and output convention; null means no sampling request. */
export function cubeOptions(
  client: FinstackClient | null,
  session: number,
  request: CubeSampleRequest | null,
) {
  return snapshotQueryOptions(
    client,
    session,
    "sampleCube",
    request,
    "Cube sampling requires a ready worker and explicit coordinates/convention",
  );
}
/** Native volatilities in the selected convention and coordinate order; errors are unchanged. */
export function useCubeSamples(request: CubeSampleRequest | null) {
  return useWorkerQuery((worker) =>
    cubeOptions(worker.client, worker.session, request),
  );
}

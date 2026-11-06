"use client";
import {
  snapshotQueryOptions,
  useWorkerQuery,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { CubeSmileRequest } from "@/workers/finstack-contract";
export type { CubeSmileRequest, CubeSmile } from "@/workers/finstack-contract";
/** Snapshot every SABR node, forward, the node indices, strikes, rate and repair passes; null means no smile request. */
export function cubeSmileOptions(
  client: FinstackClient | null,
  session: number,
  request: CubeSmileRequest | null,
) {
  return snapshotQueryOptions(
    client,
    session,
    "cubeSmile",
    request,
    "A cube smile requires a ready worker, a node, strikes and an explicit rate",
  );
}
/** Native SABR smile, arbitrage check and optional repair of one cube node; errors are unchanged. */
export function useCubeSmile(request: CubeSmileRequest | null) {
  return useWorkerQuery((worker) =>
    cubeSmileOptions(worker.client, worker.session, request),
  );
}

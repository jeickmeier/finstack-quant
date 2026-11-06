"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { CurveQueryRequest } from "@/workers/finstack-contract";
export type {
  CurveQueryRequest,
  CurveQueryResult,
} from "@/workers/finstack-contract";
/** Key the lookup on the complete market JSON, target and points; nothing is defaulted or reordered. */
export function curveQueryOptions(
  client: FinstackClient | null,
  session: number,
  request: CurveQueryRequest | null,
) {
  const snapshot = request === null ? null : structuredClone(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "curveQuery", snapshot] as const,
    ready: snapshot !== null,
    missing: "No curve lookup supplied",
    queryFn: (ready) => ready.call("curveQuery", snapshot!),
  });
}
/** Native values of one stored curve or surface at the supplied points, or the actual native error; supply a Finstack/Query provider. */
export function useCurveQuery(request: CurveQueryRequest | null) {
  return useWorkerQuery((worker) =>
    curveQueryOptions(worker.client, worker.session, request),
  );
}

"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { MarginCallsRequest } from "@/workers/finstack-contract";
export type { MarginCallsRequest } from "@/workers/finstack-contract";
/** Key the call on the complete snapshotted request. */
export function marginCallsOptions(
  client: FinstackClient | null,
  session: number,
  request: MarginCallsRequest | null,
) {
  const snapshot = request === null ? null : structuredClone(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "marginCalls", snapshot] as const,
    ready: snapshot !== null,
    missing: "No CSA supplied",
    queryFn: (ready) => ready.call("marginCalls", snapshot!),
  });
}
/** Native margin-call schedule of an exposure series under a CSA, or the actual native error; supply a Finstack/Query provider. */
export function useMarginCalls(request: MarginCallsRequest | null) {
  return useWorkerQuery((worker) =>
    marginCallsOptions(worker.client, worker.session, request),
  );
}

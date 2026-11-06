"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { AttributePnlManyRequest } from "@/workers/finstack-contract";
export type {
  AttributedPnl,
  AttributePnlManyRequest,
} from "@/workers/finstack-contract";
/** Key the attribution on the complete snapshotted request; the JSON texts are never parsed and nothing is defaulted. */
export function attributePnlManyOptions(
  client: FinstackClient | null,
  session: number,
  request: AttributePnlManyRequest | null,
) {
  const snapshot = request === null ? null : structuredClone(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "attributePnlMany", snapshot] as const,
    ready: snapshot !== null,
    missing: "No attribution request supplied",
    queryFn: (ready) => ready.call("attributePnlMany", snapshot!),
  });
}
/** Native attribution.attributePnlMany results (one per instrument, in order) with Rust's explain text, residual check and share of total, or the actual native error; supply a Finstack/Query provider. */
export function useAttributePnlMany(request: AttributePnlManyRequest | null) {
  return useWorkerQuery((worker) =>
    attributePnlManyOptions(worker.client, worker.session, request),
  );
}

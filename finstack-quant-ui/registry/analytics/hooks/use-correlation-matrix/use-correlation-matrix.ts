"use client";
import { requestSnapshot } from "@/hooks/shared/use-finstack/snapshot";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { CorrelationMatrixRequest } from "@/workers/finstack-contract";
export type {
  CorrelationMatrixReport,
  CorrelationMatrixRequest,
} from "@/workers/finstack-contract";
/** Key the matrix on the complete snapshotted panel; nothing is defaulted or reordered. */
export function correlationMatrixOptions(
  client: FinstackClient | null,
  session: number,
  request: CorrelationMatrixRequest | null,
) {
  const { snapshot, key } = requestSnapshot(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "correlationMatrix", key] as const,
    ready: snapshot !== null,
    missing: "No return panel supplied",
    queryFn: (ready) => ready.call("correlationMatrix", snapshot!),
  });
}
/** Native analytics.Performance#correlationMatrix of a host-supplied panel with Rust's repaired flag, or the native error; null disables the call. Supply a Finstack/Query provider. */
export function useCorrelationMatrix(request: CorrelationMatrixRequest | null) {
  return useWorkerQuery((worker) =>
    correlationMatrixOptions(worker.client, worker.session, request),
  );
}

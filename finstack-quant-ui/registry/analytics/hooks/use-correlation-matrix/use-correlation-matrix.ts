"use client";
import {
  snapshotQueryOptions,
  useWorkerQuery,
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
  return snapshotQueryOptions(
    client,
    session,
    "correlationMatrix",
    request,
    "No return panel supplied",
  );
}
/** Native analytics.Performance#correlationMatrix of a host-supplied panel with its common-sample count and date bounds, or the native error; null disables the call. Supply a Finstack/Query provider. */
export function useCorrelationMatrix(request: CorrelationMatrixRequest | null) {
  return useWorkerQuery((worker) =>
    correlationMatrixOptions(worker.client, worker.session, request),
  );
}

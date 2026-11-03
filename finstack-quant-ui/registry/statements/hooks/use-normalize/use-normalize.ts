"use client";
import { requestSnapshot } from "@/hooks/shared/use-finstack/snapshot";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { NormalizeRequest } from "@/workers/finstack-contract";
export type {
  NormalizationResult,
  NormalizeRequest,
} from "@/workers/finstack-contract";
/** Snapshot the result and config JSON. Null disables the query. */
export function normalizeOptions(
  client: FinstackClient | null,
  session: number,
  request: NormalizeRequest | null,
) {
  const { snapshot, key } = requestSnapshot(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "normalize", key] as const,
    ready: snapshot !== null,
    missing: "Normalization requires an explicit request and ready worker",
    queryFn: (ready) => ready.call("normalize", snapshot!),
  });
}
/** Native statements.normalize: one NormalizationResult per period of the target node, or the native error. */
export function useNormalize(request: NormalizeRequest | null) {
  return useWorkerQuery((worker) =>
    normalizeOptions(worker.client, worker.session, request),
  );
}

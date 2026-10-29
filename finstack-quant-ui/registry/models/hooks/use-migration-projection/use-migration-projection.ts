"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { MigrationProjectionRequest } from "@/workers/finstack-contract";
export type {
  MigrationProjection,
  MigrationProjectionRequest,
} from "@/workers/finstack-contract";
/** Key the projection on the complete snapshotted matrix, horizons and tolerance; an omitted tolerance stays omitted. */
export function migrationProjectionOptions(
  client: FinstackClient | null,
  session: number,
  request: MigrationProjectionRequest | null,
) {
  const snapshot = request === null ? null : structuredClone(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "migrationProjection", snapshot] as const,
    ready: snapshot !== null,
    missing: "No transition matrix and horizons supplied",
    queryFn: (ready) => ready.call("migrationProjection", snapshot!),
  });
}
/** Native generator and default probabilities projected to each horizon, or the actual native error; supply a Finstack/Query provider. */
export function useMigrationProjection(
  request: MigrationProjectionRequest | null,
) {
  return useWorkerQuery((worker) => ({
    ...migrationProjectionOptions(worker.client, worker.session, request),
    enabled: worker.status === "ready" && request !== null,
  }));
}

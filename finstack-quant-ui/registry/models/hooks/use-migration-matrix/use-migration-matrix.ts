"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { MigrationMatrixRequest } from "@/workers/finstack-contract";
export type {
  MigrationMatrix,
  MigrationMatrixRequest,
} from "@/workers/finstack-contract";
/** Key the matrix on its complete snapshotted labels, rows and horizon; nothing is defaulted or reordered. */
export function migrationMatrixOptions(
  client: FinstackClient | null,
  session: number,
  request: MigrationMatrixRequest | null,
) {
  const snapshot = request === null ? null : structuredClone(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "migrationMatrix", snapshot] as const,
    ready: snapshot !== null,
    missing: "No transition matrix supplied",
    queryFn: (ready) => ready.call("migrationMatrix", snapshot!),
  });
}
/** Native models.credit.migration.TransitionMatrix of a labelled host matrix, or the actual native error; supply a Finstack/Query provider. */
export function useMigrationMatrix(request: MigrationMatrixRequest | null) {
  return useWorkerQuery((worker) => ({
    ...migrationMatrixOptions(worker.client, worker.session, request),
    enabled: worker.status === "ready" && request !== null,
  }));
}

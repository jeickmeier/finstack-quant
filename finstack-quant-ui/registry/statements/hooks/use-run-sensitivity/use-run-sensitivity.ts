"use client";
import { requestSnapshot } from "@/hooks/shared/use-finstack/snapshot";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { RunSensitivityRequest } from "@/workers/finstack-contract";
export type {
  RunSensitivityRequest,
  SensitivityParameterRequest,
  StatementSensitivity,
} from "@/workers/finstack-contract";
/** Snapshot the model, mode, parameters, target metric and period. Null disables the run. A non-finite input stays distinct in the key and reaches Rust as entered. */
export function runSensitivityOptions(
  client: FinstackClient | null,
  session: number,
  request: RunSensitivityRequest | null,
) {
  const { snapshot, key } = requestSnapshot(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "runSensitivity", key] as const,
    ready: snapshot !== null,
    missing:
      "The sensitivity run requires an explicit request and ready worker",
    queryFn: (ready) => ready.call("runSensitivity", snapshot!),
  });
}
/** Native statements_analytics.runSensitivity result with its generateTornadoEntries tornado, or structured failure. */
export function useRunSensitivity(request: RunSensitivityRequest | null) {
  return useWorkerQuery((worker) =>
    runSensitivityOptions(worker.client, worker.session, request),
  );
}

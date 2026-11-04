"use client";
import {
  snapshotQueryOptions,
  useWorkerQuery,
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
  return snapshotQueryOptions(
    client,
    session,
    "runSensitivity",
    request,
    "The sensitivity run requires an explicit request and ready worker",
  );
}
/** Native statements_analytics.runSensitivity result with its generateTornadoEntries tornado, or structured failure. */
export function useRunSensitivity(request: RunSensitivityRequest | null) {
  return useWorkerQuery((worker) =>
    runSensitivityOptions(worker.client, worker.session, request),
  );
}

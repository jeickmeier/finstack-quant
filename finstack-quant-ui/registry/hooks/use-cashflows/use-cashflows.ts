"use client";
import { useWorkerQuery, workerQueryOptions } from "../use-finstack/query";
import type { FinstackClient } from "../use-finstack/client";
import type { CashflowRequest } from "../../workers/finstack-contract";
export type { CashflowRequest } from "../../workers/finstack-contract";
/** Snapshot the complete export request, preserving original JSON and model selection. */
export function cashflowOptions(
  client: FinstackClient | null,
  session: number,
  request: CashflowRequest,
) {
  const snapshot = Object.freeze({ ...request });
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "cashflows", snapshot] as const,
    ready: true,
    missing: "Worker is not ready",
    queryFn: (ready) => ready.call("cashflows", snapshot),
  });
}
/** Original native JSON text or actual native error; supply a Finstack/Query provider. */
export function useCashflows(request: CashflowRequest, enabled = true) {
  return useWorkerQuery((worker) => ({
    ...cashflowOptions(worker.client, worker.session, request),
    enabled: worker.status === "ready" && enabled,
  }));
}

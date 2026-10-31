"use client";
import { requestSnapshot } from "@/hooks/shared/use-finstack/snapshot";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { PoolReportRequest } from "@/workers/finstack-contract";
export type {
  PoolReport,
  PoolReportRequest,
} from "@/workers/finstack-contract";
/** Native statistics, WARF and concentration of the deal's pool, or a structured failure. Null disables the call. */
export function usePoolReport(request: PoolReportRequest | null) {
  return useWorkerQuery((worker) => {
    const { snapshot, key } = requestSnapshot(request);
    return workerQueryOptions({
      client: worker.client,
      queryKey: ["finstack", worker.session, "poolReport", key] as const,
      ready: snapshot !== null,
      missing: "Pool report requires an explicit request and ready worker",
      queryFn: (ready) => ready.call("poolReport", snapshot!),
    });
  });
}
/** Native units of a pool report's fields for its currency. Null disables the call. */
export function usePoolReportFieldUnits(currency: string | null) {
  return useWorkerQuery((worker) =>
    workerQueryOptions({
      client: worker.client,
      queryKey: [
        "finstack",
        worker.session,
        "poolReportFieldUnits",
        currency,
      ] as const,
      ready: currency !== null,
      missing: "Pool report units require a currency and ready worker",
      queryFn: (ready) => ready.call("poolReportFieldUnits", currency!),
    }),
  );
}

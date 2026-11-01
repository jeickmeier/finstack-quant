"use client";
import { requestSnapshot } from "@/hooks/shared/use-finstack/snapshot";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type {
  DealSimulationRequest,
  PoolReportRequest,
} from "@/workers/finstack-contract";
export type {
  DealSimulationRequest,
  PoolReport,
  PoolReportRequest,
  SimulationRun,
} from "@/workers/finstack-contract";
/** Native tranche flows and period accounting of the deal's simulation, or a structured failure. Null disables the call. */
export function useDealSimulation(request: DealSimulationRequest | null) {
  return useWorkerQuery((worker) => {
    const { snapshot, key } = requestSnapshot(request);
    return workerQueryOptions({
      client: worker.client,
      queryKey: ["finstack", worker.session, "runSimulation", key] as const,
      ready: snapshot !== null,
      missing: "Deal simulation requires an explicit request and ready worker",
      queryFn: (ready) => ready.call("runSimulation", snapshot!),
    });
  });
}
/** Native units of a simulation's charted series for the deal currency. Null disables the call. */
export function useSimulationFieldUnits(currency: string | null) {
  return useWorkerQuery((worker) =>
    workerQueryOptions({
      client: worker.client,
      queryKey: [
        "finstack",
        worker.session,
        "simulationFieldUnits",
        currency,
      ] as const,
      ready: currency !== null,
      missing: "Simulation units require a currency and ready worker",
      queryFn: (ready) => ready.call("simulationFieldUnits", currency!),
    }),
  );
}
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

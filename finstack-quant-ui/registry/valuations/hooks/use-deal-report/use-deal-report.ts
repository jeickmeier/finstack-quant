"use client";
import {
  snapshotQueryOptions,
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type {
  DealSimulationRequest,
  EquityMetricsRequest,
  PoolReportRequest,
} from "@/workers/finstack-contract";
export type {
  DealSimulationRequest,
  EquityMetrics,
  EquityMetricsRequest,
  PoolReport,
  PoolReportRequest,
  SimulationRun,
} from "@/workers/finstack-contract";
/** Native tranche flows and period accounting of the deal's simulation, or a structured failure. Null disables the call. */
export function useDealSimulation(request: DealSimulationRequest | null) {
  return useWorkerQuery((worker) =>
    snapshotQueryOptions(
      worker.client,
      worker.session,
      "runSimulation",
      request,
      "Deal simulation requires an explicit request and ready worker",
    ),
  );
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
/** Native equity metrics of the deal at a purchase price, or a structured failure, as for a deal with no equity tranche. Null disables the call. */
export function useEquityMetrics(request: EquityMetricsRequest | null) {
  return useWorkerQuery((worker) =>
    snapshotQueryOptions(
      worker.client,
      worker.session,
      "equityMetrics",
      request,
      "Equity metrics require an explicit request and ready worker",
    ),
  );
}
/** Native units of an equity tranche's metrics for their currency. Null disables the call. */
export function useEquityMetricsFieldUnits(currency: string | null) {
  return useWorkerQuery((worker) =>
    workerQueryOptions({
      client: worker.client,
      queryKey: [
        "finstack",
        worker.session,
        "equityMetricsFieldUnits",
        currency,
      ] as const,
      ready: currency !== null,
      missing: "Equity metric units require a currency and ready worker",
      queryFn: (ready) => ready.call("equityMetricsFieldUnits", currency!),
    }),
  );
}
/** Native statistics, WARF and concentration of the deal's pool, or a structured failure. Null disables the call. */
export function usePoolReport(request: PoolReportRequest | null) {
  return useWorkerQuery((worker) =>
    snapshotQueryOptions(
      worker.client,
      worker.session,
      "poolReport",
      request,
      "Pool report requires an explicit request and ready worker",
    ),
  );
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

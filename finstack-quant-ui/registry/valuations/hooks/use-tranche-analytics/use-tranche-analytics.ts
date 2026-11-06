"use client";
import { useCallback } from "react";
import { useFinstack } from "@/hooks/shared/use-finstack/use-finstack";
import {
  snapshotQueryOptions,
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type {
  StandardDealRequest,
  TrancheMetricsRequest,
  TrancheOasRequest,
  TranchePriceRequest,
  TrancheRequest,
} from "@/workers/finstack-contract";
export type {
  DealTranches,
  StandardDealRequest,
  StandardDealType,
  TrancheMetricsRequest,
  TrancheOasRequest,
  TranchePriceRequest,
  TrancheRequest,
} from "@/workers/finstack-contract";
/** The deal families Rust's standard constructor accepts, each with its registry profile id and example inputs. */
export function useStandardDealFamilies() {
  return useWorkerQuery((worker) =>
    workerQueryOptions({
      client: worker.client,
      queryKey: ["finstack", worker.session, "standardDealFamilies"] as const,
      ready: true,
      missing: "Deal families require a ready worker",
      queryFn: (ready) => ready.call("standardDealFamilies"),
    }),
  );
}
/** Build a deal on request: native canonical instrument JSON, or the native error thrown. */
export function useNewStandardDeal() {
  const worker = useFinstack();
  return useCallback(
    async (request: StandardDealRequest) => {
      if (worker.error) throw worker.error;
      if (worker.status !== "ready" || !worker.client)
        throw new Error("Building a deal is waiting for the worker");
      return worker.client.call("newStandardDeal", request);
    },
    [worker.client, worker.status, worker.error],
  );
}
/** Native tranches of a deal's instrument JSON. Null disables the call. */
export function useDealTranches(instrumentJson: string | null) {
  return useWorkerQuery((worker) =>
    workerQueryOptions({
      client: worker.client,
      queryKey: [
        "finstack",
        worker.session,
        "dealTranches",
        instrumentJson,
      ] as const,
      ready: instrumentJson !== null,
      missing: "Deal tranches require instrument JSON and a ready worker",
      queryFn: (ready) => ready.call("dealTranches", instrumentJson!),
    }),
  );
}
/** Snapshot the deal, tranche, market, date, optional price and metric ids. Null disables the call. */
export function trancheMetricsOptions(
  client: FinstackClient | null,
  session: number,
  request: TrancheMetricsRequest | null,
) {
  return snapshotQueryOptions(
    client,
    session,
    "trancheMetrics",
    request,
    "Tranche metrics require an explicit request and ready worker",
  );
}
/** Snapshot the deal, tranche, market, date, explicit price and optional OasConfig. Null disables the solve. */
export function trancheOasOptions(
  client: FinstackClient | null,
  session: number,
  request: TrancheOasRequest | null,
) {
  return snapshotQueryOptions(
    client,
    session,
    "trancheOas",
    request,
    "Tranche OAS requires an explicit price and ready worker",
  );
}
/** Native units of the TrancheMetrics fields for a result's currency. Null disables the call. */
export function useTrancheMetricFieldUnits(currency: string | null) {
  return useWorkerQuery((worker) =>
    workerQueryOptions({
      client: worker.client,
      queryKey: [
        "finstack",
        worker.session,
        "trancheMetricFieldUnits",
        currency,
      ] as const,
      ready: currency !== null,
      missing: "Tranche field units require a currency and ready worker",
      queryFn: (ready) => ready.call("trancheMetricFieldUnits", currency!),
    }),
  );
}
/** Native valuations.instruments.trancheSolveFieldUnits for the break-even CDR, discount margin and OAS tiles. */
export function useTrancheSolveFieldUnits() {
  return useWorkerQuery((worker) =>
    workerQueryOptions({
      client: worker.client,
      queryKey: ["finstack", worker.session, "trancheSolveFieldUnits"] as const,
      ready: true,
      missing: "Tranche solve units require a ready worker",
      queryFn: (ready) => ready.call("trancheSolveFieldUnits"),
    }),
  );
}
/** Snapshot the deal, tranche, market, date and explicit price. Null disables the solve. */
export function trancheDiscountMarginOptions(
  client: FinstackClient | null,
  session: number,
  request: TranchePriceRequest | null,
) {
  return snapshotQueryOptions(
    client,
    session,
    "trancheDiscountMargin",
    request,
    "Tranche discount margin requires an explicit price and ready worker",
  );
}
/** Snapshot the deal, tranche, market and date. Null disables the solve. */
export function trancheBreakevenCdrOptions(
  client: FinstackClient | null,
  session: number,
  request: TrancheRequest | null,
) {
  return snapshotQueryOptions(
    client,
    session,
    "trancheBreakevenCdr",
    request,
    "Tranche break-even CDR requires an explicit request and ready worker",
  );
}
/** Native TrancheMetrics or a structured failure. */
export function useTrancheMetrics(request: TrancheMetricsRequest | null) {
  return useWorkerQuery((worker) =>
    trancheMetricsOptions(worker.client, worker.session, request),
  );
}
/** Native flows and date table of one tranche, or a structured failure. Null disables the call. */
export function useTrancheSchedule(request: TrancheRequest | null) {
  return useWorkerQuery((worker) =>
    snapshotQueryOptions(
      worker.client,
      worker.session,
      "trancheSchedule",
      request,
      "Tranche schedule requires an explicit request and ready worker",
    ),
  );
}
/** Native OasResult under the supplied OasConfig, else Rust's default; pass null until the user asks for the solve. */
export function useTrancheOas(request: TrancheOasRequest | null) {
  return useWorkerQuery((worker) =>
    trancheOasOptions(worker.client, worker.session, request),
  );
}
/** Native decimal discount margin or a structured failure (fixed-rate tranches fail natively). */
export function useTrancheDiscountMargin(request: TranchePriceRequest | null) {
  return useWorkerQuery((worker) =>
    trancheDiscountMarginOptions(worker.client, worker.session, request),
  );
}
/** Native decimal break-even constant default rate or a structured failure. */
export function useTrancheBreakevenCdr(request: TrancheRequest | null) {
  return useWorkerQuery((worker) =>
    trancheBreakevenCdrOptions(worker.client, worker.session, request),
  );
}
